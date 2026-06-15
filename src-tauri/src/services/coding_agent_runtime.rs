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
//! caller — once the session moves, it is unreachable. The full steer/abort
//! wiring is the next feature (`m1-steer-abort`); this feature only hands those
//! handles back so that feature has its attachment point.
//!
//! Session persistence stays in-memory for this feature (the session is built
//! with `no_session = true`; JSONL persistence is M3).

use std::sync::Arc;

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
/// it is unreachable. The next feature (`m1-steer-abort`) flips the cancel
/// token (`abort`) and pushes onto the steering queue (`steer`) through these
/// handles while the turn is in flight. This feature only hands them back; it
/// does not yet wire abort / steer commands to them.
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
    // itself unreachable. Handed back for the next feature (m1-steer-abort).
    let cancel = session.cancel_handle();
    let steering = session.steering_queue_handle();

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
}
