//! coding_agent_runtime — drive one prompt turn through a coding-agent
//! [`AgentSession`] and map its events onto HandBox's existing three Tauri
//! channels (`agent_stream_event` / `agent_stream_closed` / `agent_stream_error`).
//!
//! This is the platform-shift proof feature: a prompt now runs end-to-end on
//! `hand_coding_agent::AgentSession` instead of the legacy
//! [`agent_runtime::AgentRuntime`] loop, while the *frontend* contract stays
//! byte-for-byte the same. The event payloads, channel names, the
//! closed-exactly-once invariant, and the sanitized error envelope all mirror
//! `agent_runtime.rs` so the UI never has to know which engine drove the turn.
//!
//! Drive strategy (A — `subscribe` direct-drive):
//! - The session is `subscribe`d to a callback that unwraps
//!   [`AgentSessionEvent::Agent`] into the inner `AgentEvent` and emits it on
//!   `agent_stream_event` as `{ sessionId, event }`. The `AgentEvent` here is
//!   the very type the HandBox frontend already consumes (same git+tag dedup as
//!   `hand-agent`), so `serde_json::to_value(event)` produces the identical
//!   shape the legacy sink produced.
//! - `CompactionStart` / `CompactionEnd` / `SessionInfoChanged` are
//!   out-of-band lifecycle signals the current frontend contract does not yet
//!   model; they are logged rather than synthesized into the agent-event stream
//!   (introducing new event shapes is out of this feature's scope).
//! - `AgentSessionEvent::Error(_)` is rare — the error main-path is
//!   `send_message` returning `Err` — so it is logged; the run still closes
//!   exactly once below.
//! - After subscribing, the background task awaits `session.send_message`.
//!   On `Err` it emits a sanitized `{ sessionId, error }` envelope BEFORE the
//!   terminal closed signal; on either outcome it emits `agent_stream_closed`
//!   EXACTLY ONCE.
//!
//! Concurrency: `send_message` takes `&mut self`, so the background task OWNS
//! the session for the turn. The abort / steer handles
//! ([`AgentSession::cancel_handle`] / [`AgentSession::steering_queue_handle`])
//! are cloned out BEFORE the session is moved into the task and returned to the
//! caller — once the session moves, it is unreachable. `drive_agent_run`
//! registers those handles in a process-level [`run_controls`] registry keyed
//! by `session_id` and removes the entry at the single closed emit site (so the
//! registration lifetime exactly brackets the run). [`abort_run`] flips the
//! cancel token through that registry; [`steer_run`] pushes a user [`Message`]
//! onto the steering queue, which the agent loop drains at the next mid-turn
//! boundary (so a mid-run send joins the CURRENT turn, never a follow-up).
//!
//! Session persistence stays in-memory for this feature (the session is built
//! with `no_session = true`; JSONL persistence is M3).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use hand_ai_model::{Message, UserMessage};
use hand_coding_agent::{AgentSession, AgentSessionEvent, CodingAgentError};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::models::AppError;

/// What a single [`AgentSessionEvent`] maps to on HandBox's event surface.
///
/// Pulling the mapping out of the `subscribe` closure makes the policy
/// directly unit-testable without spawning a task or touching the network:
/// feed a synthetic `AgentSessionEvent` in, assert which side of the contract
/// it lands on.
#[derive(Debug, Clone, PartialEq)]
pub enum MappedEvent {
    /// Forward the inner `AgentEvent` JSON on `agent_stream_event` as the
    /// `event` field of `{ sessionId, event }`.
    Forward(Value),
    /// An out-of-band lifecycle signal (compaction / session-info / session
    /// error) the current frontend contract does not model; log and drop.
    Logged,
}

/// Map an [`AgentSessionEvent`] to its HandBox event-surface action.
///
/// - `Agent(e)` → `Forward(serde_json::to_value(e))`: the inner `AgentEvent`
///   serializes to exactly the shape the legacy sink produced (same
///   `#[serde(tag = "type", rename_all = "snake_case", rename_all_fields =
///   "camelCase")]` type), so the frontend contract is unchanged.
/// - everything else → `Logged`: compaction / session-info / session-error are
///   not part of the current `agent_stream_event` contract; emitting new shapes
///   is out of scope, so they are observed for diagnostics and dropped.
fn map_session_event(event: &AgentSessionEvent) -> MappedEvent {
    match event {
        AgentSessionEvent::Agent(agent_event) => {
            // The inner `AgentEvent` is the same type the frontend already
            // consumes; a serialize failure is structural and must never break
            // the stream — fall back to a diagnostic object (mirrors the legacy
            // sink's serializeError fallback).
            let value = serde_json::to_value(agent_event.as_ref())
                .unwrap_or_else(|e| json!({ "type": "serializeError", "message": e.to_string() }));
            MappedEvent::Forward(value)
        }
        // Out-of-band lifecycle signals not modeled by the current frontend
        // contract. Logged, not synthesized into the agent-event stream.
        AgentSessionEvent::CompactionStart
        | AgentSessionEvent::CompactionEnd { .. }
        | AgentSessionEvent::SessionInfoChanged { .. }
        | AgentSessionEvent::Error(_) => MappedEvent::Logged,
    }
}

/// Map a run-level [`CodingAgentError`] to a **sanitized** [`AppError`]
/// `{ code, message, hint }` for the `agent_stream_error` envelope.
///
/// SECURITY: same discipline as `agent_runtime::sanitize_agent_error` — never
/// echo raw provider / transport error text (it can carry an API key or a
/// credentialed URL). Each variant maps to a stable AppError code plus a
/// generic-but-useful hint. The `CodingAgentError::Agent` arm delegates to the
/// exact same `AgentError` classification the legacy path uses, so the error
/// codes the frontend sees are identical across engines.
fn sanitize_coding_agent_error(err: &CodingAgentError) -> AppError {
    use hand_agent::AgentError;
    use hand_ai_model::ClientError;

    match err {
        // The model loop failed at run level. Reuse the exact AgentError
        // classification the legacy runtime applies so the frontend sees the
        // same codes regardless of which engine drove the turn.
        CodingAgentError::Agent(agent_err) => match agent_err {
            AgentError::Client(client_err) => match client_err {
                ClientError::ProviderNotFound { model_id, .. } => AppError::with_hint(
                    "AUTH_ERROR",
                    &format!("no provider is configured for model \"{}\"", model_id),
                    "请在设置中为该模型配置可用的供应商与 API Key",
                ),
                ClientError::OAuthRequired { .. } => {
                    AppError::auth_error("the selected provider requires sign-in credentials")
                }
                ClientError::StreamEndedWithoutResult => {
                    AppError::network_error("the model stream ended without producing a response")
                }
            },
            AgentError::Proxy { status, .. } => match status {
                401 | 403 => AppError::auth_error(
                    "the provider rejected the request (authentication failed)",
                ),
                429 => AppError::rate_limit_error(),
                _ => AppError::network_error("the provider request failed"),
            },
            AgentError::Aborted => {
                AppError::with_hint("INTERNAL_ERROR", "the run was aborted", "请重试该回合")
            }
            _ => AppError::internal_error("the agent run failed to complete"),
        },
        // Session/settings/tool/serialization/io/other lifecycle failures from
        // the coding-agent layer. These originate from our own assembly, not
        // from raw provider text, but still take a generic internal code.
        _ => AppError::internal_error("the agent run failed to complete"),
    }
}

/// Event sink for a coding-agent run — the single choke point through which a
/// driven turn reaches HandBox's three Tauri channels.
///
/// Shape mirrors `agent_runtime::RunSink` deliberately: `on_event` receives
/// `{ sessionId, event }`, `on_closed` receives the terminal `{ sessionId }`
/// EXACTLY ONCE, and the optional `on_error` receives the sanitized
/// `{ sessionId, error }` envelope BEFORE `on_closed`. When `on_error` is
/// absent the envelope falls back to `on_event` so the error still reaches the
/// UI without introducing a second closed emit site.
#[derive(Clone)]
pub struct CodingRunSink {
    on_event: Arc<dyn Fn(Value) + Send + Sync>,
    on_closed: Arc<dyn Fn(Value) + Send + Sync>,
    on_error: Option<Arc<dyn Fn(Value) + Send + Sync>>,
}

impl CodingRunSink {
    /// Construct a sink. `on_event` receives `{ sessionId, event }`; `on_closed`
    /// receives the terminal `{ sessionId }`. The error envelope falls back to
    /// `on_event` until [`CodingRunSink::with_error`] injects a dedicated
    /// channel.
    pub fn new(
        on_event: Arc<dyn Fn(Value) + Send + Sync>,
        on_closed: Arc<dyn Fn(Value) + Send + Sync>,
    ) -> Self {
        Self {
            on_event,
            on_closed,
            on_error: None,
        }
    }

    /// Inject a dedicated channel for the run-level `Err` envelope, yielding a
    /// sink that routes `{ sessionId, error }` to `on_error` instead of
    /// `on_event`.
    pub fn with_error(mut self, on_error: Arc<dyn Fn(Value) + Send + Sync>) -> Self {
        self.on_error = Some(on_error);
        self
    }
}

/// Abort / steer handles for a driven run, cloned out before the session was
/// moved into the background task.
///
/// `send_message` borrows the session `&mut`, so once the task owns the session
/// it is unreachable. [`abort_run`] flips the cancel token and [`steer_run`]
/// pushes onto the steering queue through these handles while the turn is in
/// flight, reaching the live run via the process-level [`run_controls`]
/// registry (`drive_agent_run` registers them on entry and removes the entry at
/// the closed emit site).
pub struct RunDriveHandles {
    /// Shared cancellation token — `cancel()` it to abort the in-flight turn
    /// (identical semantics to `AgentSession::abort`).
    pub cancel: Arc<std::sync::Mutex<hand_agent::CancellationToken>>,
    /// Shared steering queue — push a user `Message` to inject it at the next
    /// mid-turn boundary (drained by the session's `get_steering_messages`).
    pub steering: Arc<std::sync::Mutex<Vec<hand_ai_model::Message>>>,
    /// The spawned driver task. Awaiting it joins the run; dropping it detaches.
    pub task: JoinHandle<()>,
}

/// Live steer / abort controls for one driven run.
///
/// Holds the two shared handles cloned out of the `AgentSession` before it
/// moved into the driver task (see [`RunDriveHandles`]). Both are
/// `Arc<std::sync::Mutex<..>>` — the SAME `Arc`s the in-flight `send_message`
/// wired into its cancel token and `get_steering_messages` closure — so flipping
/// `cancel` or pushing onto `steering` here reaches the running turn directly.
struct RunControl {
    cancel: Arc<Mutex<hand_agent::CancellationToken>>,
    steering: Arc<Mutex<Vec<Message>>>,
}

/// Process-level `session_id → RunControl` registry for coding-agent driven
/// runs.
///
/// Companion to `commands::agent_run::active_coding_runs` (the one-run-per-
/// session set): that set gates concurrency, this map carries the live steer /
/// abort handles. A process-level `OnceLock<Mutex<HashMap<..>>>` is used for the
/// same reason — the `AgentSession` is owned by the background driver task for
/// the turn, so there is no instance-level place to hang per-run state that the
/// stateless command handlers can reach. `drive_agent_run` inserts on entry and
/// removes the entry at the single closed emit site, so an entry exists for
/// exactly the run's lifetime. [`steer_run`] / [`abort_run`] look the run up
/// here; an absent entry (no active run) is a clean no-op.
fn run_controls() -> &'static Mutex<HashMap<String, RunControl>> {
    static CONTROLS: OnceLock<Mutex<HashMap<String, RunControl>>> = OnceLock::new();
    CONTROLS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Inject `text` as a user [`Message`] into the session's IN-FLIGHT turn.
///
/// Mirrors the legacy `AgentRuntime::steer` contract:
/// - empty / whitespace-only `text` is a no-op — nothing is enqueued;
/// - a session with no active run in the registry is a CLEAN no-op (the front
///   end may race a steer against a run that just ended naturally; returning an
///   error would turn that benign race into noise);
/// - otherwise the message is pushed onto the run's steering queue, which the
///   agent loop drains at the next mid-turn boundary via `get_steering_messages`
///   — so it joins the CURRENT turn as a user message and never spawns a second
///   concurrent run. The follow-up queue is deliberately NOT touched, so a
///   mid-run send is never auto-continued after the turn ends (VAL-CARUN-021).
pub fn steer_run(session_id: &str, text: String) {
    if text.trim().is_empty() {
        return;
    }
    let controls = run_controls().lock().unwrap();
    if let Some(control) = controls.get(session_id) {
        let message = Message::User(UserMessage::new_text(text));
        control.steering.lock().unwrap().push(message);
    }
}

/// Abort the session's in-flight turn by flipping its cancellation token.
///
/// Mirrors the legacy `AgentRuntime::abort` contract: an unknown / already-
/// finished session is a CLEAN no-op (the front end may race an abort against a
/// run that just ended). The token reached here is the SAME one the in-flight
/// `send_message` is driving on, so cancelling it makes the agent loop unwind at
/// its next await point and synthesize a `stopReason=aborted` terminal turn; the
/// driver task's `send_message` then resolves and the sink emits
/// `agent_stream_closed` EXACTLY ONCE (the closed-once invariant holds on the
/// abort path too). The registry entry is NOT removed here — removal stays owned
/// by the driver task's closed emit site, so a stale abort can't drop a live
/// entry out from under a still-running turn.
pub fn abort_run(session_id: &str) {
    let controls = run_controls().lock().unwrap();
    if let Some(control) = controls.get(session_id) {
        control.cancel.lock().unwrap().cancel();
    }
}

/// Register a run's steer / abort controls under `session_id`.
///
/// Called by `drive_agent_run` immediately after the handles are cloned out of
/// the session and before the driver task is spawned, so a steer / abort issued
/// the instant the command returns already reaches the run.
fn register_run(
    session_id: &str,
    cancel: Arc<Mutex<hand_agent::CancellationToken>>,
    steering: Arc<Mutex<Vec<Message>>>,
) {
    run_controls()
        .lock()
        .unwrap()
        .insert(session_id.to_string(), RunControl { cancel, steering });
}

/// Remove a run's steer / abort controls. Called from the driver task at the
/// single closed emit site, so the registration lifetime exactly brackets the
/// run and a subsequent run for the same session can register cleanly.
fn deregister_run(session_id: &str) {
    run_controls().lock().unwrap().remove(session_id);
}

/// Drive one prompt turn through `session`, mapping its events onto `sink`.
///
/// Non-blocking: subscribes the session, captures its abort / steer handles,
/// spawns the driver task, and returns immediately with [`RunDriveHandles`].
/// The turn runs in the background; events arrive asynchronously via `sink`.
///
/// Lifecycle guarantees (the sacred closed-once invariant):
/// - every `AgentSessionEvent::Agent(e)` is forwarded on `on_event` as
///   `{ sessionId, event }` in emission order;
/// - on `send_message` → `Err`, a sanitized `{ sessionId, error }` envelope is
///   emitted (via `on_error`, or `on_event` as fallback) BEFORE closing;
/// - `on_closed` fires EXACTLY ONCE with `{ sessionId }`, regardless of Ok/Err.
pub fn drive_agent_run(
    mut session: AgentSession,
    session_id: String,
    input: String,
    sink: CodingRunSink,
) -> RunDriveHandles {
    // Capture abort / steer handles BEFORE the session moves into the task —
    // afterwards the `&mut self` borrow inside `send_message` makes the session
    // itself unreachable. Register them so `steer_run` / `abort_run` reach this
    // run; the entry is removed at the closed emit site below.
    let cancel = session.cancel_handle();
    let steering = session.steering_queue_handle();
    register_run(&session_id, Arc::clone(&cancel), Arc::clone(&steering));

    // Subscribe the event forwarder. The callback is `Fn + Send + Sync +
    // 'static`; it captures the sink's event channel and the session id, and is
    // invoked synchronously by the session for each emitted event during
    // `send_message`.
    let event_sink = Arc::clone(&sink.on_event);
    let event_session = session_id.clone();
    session.subscribe(
        move |event: AgentSessionEvent| match map_session_event(&event) {
            MappedEvent::Forward(event_json) => {
                event_sink(json!({
                    "sessionId": event_session,
                    "event": event_json,
                }));
            }
            MappedEvent::Logged => {
                tracing::debug!(
                    session_id = %event_session,
                    "coding-agent out-of-band session event (not forwarded): {:?}",
                    event
                );
            }
        },
    );

    let on_error = sink.on_error.clone();
    let on_event_for_err = Arc::clone(&sink.on_event);
    let on_closed = Arc::clone(&sink.on_closed);
    let error_session = session_id.clone();
    // One clone drives the closed payload, the other deregisters the run's
    // steer / abort controls at that same terminal site.
    let deregister_session = session_id.clone();
    let closed_session = session_id;

    let task = tokio::spawn(async move {
        // Drive exactly one turn. Streaming events have already been forwarded
        // through the subscribe callback by the time this resolves.
        let result = session.send_message(&input).await;

        // Run-level Err: emit the sanitized envelope BEFORE closing (no
        // assistant content on this path). An in-band stop_reason=error turn is
        // an `Ok` and never reaches here, so we never double-report it.
        if let Err(err) = &result {
            let app_error = sanitize_coding_agent_error(err);
            let envelope = json!({
                "sessionId": error_session,
                "error": app_error,
            });
            match &on_error {
                Some(emit_error) => emit_error(envelope),
                // No dedicated channel: fall back to on_event so the error
                // still reaches the UI without adding a second closed site.
                None => on_event_for_err(envelope),
            }
        }

        // Terminal close — the single closed emit site. Fires exactly once for
        // both Ok and Err.
        on_closed(json!({ "sessionId": closed_session }));

        // Run is over: drop its steer / abort controls so the registration
        // lifetime exactly brackets the run and the next run for this session
        // can register cleanly. Sequenced after the closed emit so a steer /
        // abort observing the run as still-registered always targets a live
        // turn.
        deregister_run(&deregister_session);
    });

    RunDriveHandles {
        cancel,
        steering,
        task,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hand_agent::{AgentError, AgentEvent};
    use hand_ai_model::{
        Api, AssistantContentBlock, AssistantMessage, Message, StopReason, TextContent, Usage,
    };
    use std::sync::Mutex as StdMutex;

    /// Build a finished assistant `Message` carrying `text` (mirrors the legacy
    /// runtime's test helper) so synthetic `AgentEvent`s look realistic.
    fn assistant_message(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![AssistantContentBlock::Text(TextContent::new(
                text.to_string(),
            ))],
            api: Api::OpenAICompletions,
            provider: hand_ai_model::types::Provider::OpenAI,
            model: "gpt-4o".to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
            response_model: None,
            response_id: None,
            diagnostics: None,
        })
    }

    /// A capturing sink that records every event / closed / error payload, so a
    /// test can assert the mapped shapes and the closed-once invariant.
    #[derive(Clone, Default)]
    struct CapturingSink {
        events: Arc<StdMutex<Vec<Value>>>,
        closed: Arc<StdMutex<Vec<Value>>>,
        errors: Arc<StdMutex<Vec<Value>>>,
    }

    impl CapturingSink {
        fn into_run_sink(self) -> CodingRunSink {
            let events = Arc::clone(&self.events);
            let closed = Arc::clone(&self.closed);
            let errors = Arc::clone(&self.errors);
            CodingRunSink::new(
                Arc::new(move |v| events.lock().unwrap().push(v)),
                Arc::new(move |v| closed.lock().unwrap().push(v)),
            )
            .with_error(Arc::new(move |v| errors.lock().unwrap().push(v)))
        }
    }

    /// Replays a scripted `AgentSessionEvent` sequence through the SAME
    /// subscribe-callback + terminal-close logic `drive_agent_run` uses, but
    /// without an `AgentSession` or the network. This lets us assert the event
    /// mapping → `{ sessionId, event }` shape and closed-exactly-once on a
    /// fake/mock event stream (the feature's required mapping unit test).
    ///
    /// `error` mirrors a `send_message` outcome: `Some(err)` exercises the
    /// envelope-before-closed path; `None` is the happy path.
    fn replay_through_sink(
        session_id: &str,
        sink: &CodingRunSink,
        events: Vec<AgentSessionEvent>,
        error: Option<CodingAgentError>,
    ) {
        // 1) Each subscribed event maps and forwards exactly as the real
        //    callback does.
        for event in &events {
            match map_session_event(event) {
                MappedEvent::Forward(event_json) => (sink.on_event)(json!({
                    "sessionId": session_id,
                    "event": event_json,
                })),
                MappedEvent::Logged => {}
            }
        }
        // 2) Terminal sequencing: error envelope (if any) BEFORE the single
        //    closed emit — the same ordering as the spawned task.
        if let Some(err) = &error {
            let envelope = json!({
                "sessionId": session_id,
                "error": sanitize_coding_agent_error(err),
            });
            match &sink.on_error {
                Some(emit_error) => emit_error(envelope),
                None => (sink.on_event)(envelope),
            }
        }
        (sink.on_closed)(json!({ "sessionId": session_id }));
    }

    /// The required mapping unit test: a fake `AgentSessionEvent::Agent` stream
    /// maps to `{ sessionId, event }` shapes and produces EXACTLY ONE closed.
    #[test]
    fn agent_events_map_to_session_event_shape_and_close_once() {
        let session_id = "sess-abc";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        let events = vec![
            AgentSessionEvent::Agent(Box::new(AgentEvent::AgentStart)),
            AgentSessionEvent::Agent(Box::new(AgentEvent::MessageEnd {
                message: assistant_message("hi there"),
            })),
            AgentSessionEvent::Agent(Box::new(AgentEvent::AgentEnd {
                messages: vec![assistant_message("hi there")],
            })),
        ];

        replay_through_sink(session_id, &run_sink, events, None);

        let captured = sink.events.lock().unwrap();
        assert_eq!(captured.len(), 3, "every Agent event is forwarded");

        // Each forwarded payload is exactly `{ sessionId, event }` and the
        // event carries the snake_case `type` tag the frontend already reads.
        let first = &captured[0];
        assert_eq!(first.get("sessionId").unwrap(), session_id);
        assert_eq!(
            first.get("event").unwrap().get("type").unwrap(),
            "agent_start",
            "inner AgentEvent keeps its snake_case type tag (frontend contract)"
        );
        assert_eq!(
            captured[2].get("event").unwrap().get("type").unwrap(),
            "agent_end"
        );

        // EXACTLY ONE closed; no error envelope on the happy path.
        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
        assert_eq!(sink.errors.lock().unwrap().len(), 0, "no error on Ok path");
        // The closed payload is `{ sessionId }`.
        assert_eq!(
            sink.closed.lock().unwrap()[0].get("sessionId").unwrap(),
            session_id
        );
    }

    /// Out-of-band lifecycle events (compaction / session-info / session-error)
    /// are NOT forwarded onto `agent_stream_event`, but the run still closes
    /// exactly once.
    #[test]
    fn out_of_band_events_are_not_forwarded_but_run_still_closes_once() {
        let session_id = "sess-oob";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        let events = vec![
            AgentSessionEvent::CompactionStart,
            AgentSessionEvent::Agent(Box::new(AgentEvent::AgentStart)),
            AgentSessionEvent::CompactionEnd {
                summary: "compacted".into(),
            },
            AgentSessionEvent::SessionInfoChanged {
                name: Some("renamed".into()),
            },
            AgentSessionEvent::Error("transient".into()),
        ];

        replay_through_sink(session_id, &run_sink, events, None);

        // Only the single Agent event was forwarded.
        assert_eq!(
            sink.events.lock().unwrap().len(),
            1,
            "only Agent events reach agent_stream_event"
        );
        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
    }

    /// A run-level `Err` emits a sanitized `{ sessionId, error }` envelope on
    /// the dedicated error channel BEFORE the single closed signal, and the
    /// envelope never echoes raw provider text.
    #[test]
    fn run_level_error_emits_sanitized_envelope_before_close() {
        let session_id = "sess-err";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        // A ProviderNotFound under the model loop — the canonical run-level
        // error. The raw text would carry the model id; the sanitized code is
        // AUTH_ERROR and the message references only the (non-secret) model id.
        let err = CodingAgentError::Agent(AgentError::Client(
            hand_ai_model::ClientError::ProviderNotFound {
                api: Api::OpenAICompletions,
                model_id: "gpt-4o".to_string(),
            },
        ));

        replay_through_sink(session_id, &run_sink, vec![], Some(err));

        // Envelope landed on the dedicated error channel, not on on_event.
        let errors = sink.errors.lock().unwrap();
        assert_eq!(errors.len(), 1, "one sanitized error envelope");
        assert_eq!(errors[0].get("sessionId").unwrap(), session_id);
        let error_obj = errors[0].get("error").unwrap();
        assert_eq!(error_obj.get("code").unwrap(), "AUTH_ERROR");
        // No assistant events on this path; closed still fires exactly once.
        assert_eq!(sink.events.lock().unwrap().len(), 0);
        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
    }

    /// `map_session_event` classification is exhaustive and stable: Agent
    /// forwards, everything else logs.
    #[test]
    fn map_session_event_classifies_each_variant() {
        assert!(matches!(
            map_session_event(&AgentSessionEvent::Agent(Box::new(AgentEvent::AgentStart))),
            MappedEvent::Forward(_)
        ));
        assert_eq!(
            map_session_event(&AgentSessionEvent::CompactionStart),
            MappedEvent::Logged
        );
        assert_eq!(
            map_session_event(&AgentSessionEvent::CompactionEnd {
                summary: String::new()
            }),
            MappedEvent::Logged
        );
        assert_eq!(
            map_session_event(&AgentSessionEvent::SessionInfoChanged { name: None }),
            MappedEvent::Logged
        );
        assert_eq!(
            map_session_event(&AgentSessionEvent::Error("x".into())),
            MappedEvent::Logged
        );
    }

    /// The sanitizer maps every error family to a stable code and never echoes
    /// raw transport text (security: no API key / credentialed URL leakage).
    #[test]
    fn sanitizer_maps_codes_without_leaking_raw_text() {
        // Proxy 401 → AUTH_ERROR; the raw `message` would carry transport
        // details, but the sanitized message must be the generic constant.
        let proxy = CodingAgentError::Agent(AgentError::Proxy {
            status: 401,
            message: "https://api.example.com/v1?key=sk-secret rejected".to_string(),
        });
        let app_err = sanitize_coding_agent_error(&proxy);
        assert_eq!(app_err.code, "AUTH_ERROR");
        assert!(
            !app_err.message.contains("sk-secret"),
            "sanitized message must not echo raw provider text"
        );

        // 429 → RATE_LIMIT.
        let rate = CodingAgentError::Agent(AgentError::Proxy {
            status: 429,
            message: "too many".to_string(),
        });
        assert_eq!(sanitize_coding_agent_error(&rate).code, "RATE_LIMIT");

        // A non-Agent lifecycle error → INTERNAL_ERROR.
        let session_err = CodingAgentError::Session("no session found".to_string());
        assert_eq!(
            sanitize_coding_agent_error(&session_err).code,
            "INTERNAL_ERROR"
        );
    }

    // --- run-control registry (m1-steer-abort) ---
    //
    // The registry is process-global, so each test uses a FRESH random
    // `session_id` (uuid) to stay isolated from every other test in the binary.

    /// A fresh, registry-unique session id.
    fn fresh_session_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Register a run with empty (but shared) cancel + steering handles, mirroring
    /// what `drive_agent_run` registers. Returns clones of both handles so the
    /// test can inspect them the way the live `send_message` turn would observe
    /// them (same `Arc`).
    fn register_test_run(
        session_id: &str,
    ) -> (
        Arc<Mutex<hand_agent::CancellationToken>>,
        Arc<Mutex<Vec<Message>>>,
    ) {
        let cancel = Arc::new(Mutex::new(hand_agent::CancellationToken::new()));
        let steering = Arc::new(Mutex::new(Vec::new()));
        register_run(session_id, Arc::clone(&cancel), Arc::clone(&steering));
        (cancel, steering)
    }

    /// True if the registry currently holds an entry for `session_id`.
    fn is_registered(session_id: &str) -> bool {
        run_controls().lock().unwrap().contains_key(session_id)
    }

    /// `register_run` makes the run reachable; `deregister_run` removes exactly
    /// that entry. This is the lifetime contract `drive_agent_run` relies on:
    /// register on entry, deregister at the closed emit site.
    #[test]
    fn register_then_deregister_brackets_the_run() {
        let session_id = fresh_session_id();
        assert!(!is_registered(&session_id), "absent before register");

        let _handles = register_test_run(&session_id);
        assert!(is_registered(&session_id), "present after register");

        deregister_run(&session_id);
        assert!(!is_registered(&session_id), "absent after deregister");
    }

    /// `steer_run` pushes a user `Message` onto the SAME steering queue the live
    /// turn drains — so a mid-run steer joins the current turn as a user message.
    #[test]
    fn steer_enqueues_user_message_onto_active_runs_queue() {
        let session_id = fresh_session_id();
        let (_cancel, steering) = register_test_run(&session_id);
        assert_eq!(steering.lock().unwrap().len(), 0);

        steer_run(&session_id, "look at foo.rs".to_string());

        let queue = steering.lock().unwrap();
        assert_eq!(queue.len(), 1, "steer enqueues exactly one message");
        assert!(
            matches!(&queue[0], Message::User(_)),
            "steered message is a user message"
        );
        drop(queue);
        deregister_run(&session_id);
    }

    /// Empty / whitespace-only steer text is a clean no-op: nothing is enqueued,
    /// the active run is left undisturbed (VAL-CARUN-017).
    #[test]
    fn steer_with_blank_text_is_noop() {
        let session_id = fresh_session_id();
        let (_cancel, steering) = register_test_run(&session_id);

        steer_run(&session_id, String::new());
        steer_run(&session_id, "   \n\t ".to_string());

        assert_eq!(
            steering.lock().unwrap().len(),
            0,
            "blank steer text enqueues nothing"
        );
        deregister_run(&session_id);
    }

    /// Steering a session with no active run is a clean no-op — no panic, no
    /// error, nothing enqueued anywhere (VAL-CARUN-017). The front end may race
    /// a steer against a run that just ended.
    #[test]
    fn steer_with_no_active_run_is_noop() {
        let session_id = fresh_session_id();
        assert!(!is_registered(&session_id));
        // Must not panic and must not create a registry entry.
        steer_run(&session_id, "hello".to_string());
        assert!(
            !is_registered(&session_id),
            "steer never registers a run on its own"
        );
    }

    /// `abort_run` flips the SAME cancellation token the live turn drives on, so
    /// the agent loop unwinds at its next await point and the run finishes
    /// "aborted" (VAL-CARUN-005's mechanism). The registry entry is NOT removed
    /// by abort — removal stays owned by the closed emit site.
    #[test]
    fn abort_cancels_the_runs_token_without_deregistering() {
        let session_id = fresh_session_id();
        let (cancel, _steering) = register_test_run(&session_id);
        assert!(
            !cancel.lock().unwrap().is_cancelled(),
            "token starts uncancelled"
        );

        abort_run(&session_id);

        assert!(
            cancel.lock().unwrap().is_cancelled(),
            "abort flips the run's cancel token"
        );
        assert!(
            is_registered(&session_id),
            "abort does not deregister — the closed emit site owns removal"
        );
        deregister_run(&session_id);
    }

    /// Aborting a session with no active run is a clean no-op (no panic, no
    /// error) — same benign-race tolerance as the blank-steer path.
    #[test]
    fn abort_with_no_active_run_is_noop() {
        let session_id = fresh_session_id();
        assert!(!is_registered(&session_id));
        abort_run(&session_id);
        assert!(!is_registered(&session_id));
    }

    /// After a run deregisters (its closed emit site fired), a steer / abort for
    /// that session is again a clean no-op — no residue leaks into the next run.
    /// This underpins VAL-CARUN-006 (abort-then-resend starts a clean new turn).
    #[test]
    fn steer_after_deregister_does_not_resurrect_the_run() {
        let session_id = fresh_session_id();
        let (_cancel, steering) = register_test_run(&session_id);
        deregister_run(&session_id);

        steer_run(&session_id, "late".to_string());
        abort_run(&session_id);

        assert_eq!(
            steering.lock().unwrap().len(),
            0,
            "the deregistered queue receives nothing"
        );
        assert!(!is_registered(&session_id), "no entry resurrected");
    }
}
