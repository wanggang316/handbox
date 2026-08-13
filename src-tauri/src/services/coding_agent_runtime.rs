//! coding_agent_runtime — drive one prompt turn through a coding-agent
//! [`AgentSession`] and map its events onto HandBox's existing three Tauri
//! channels (`agent_stream_event` / `agent_stream_closed` / `agent_stream_error`).
//!
//! Contract the frontend depends on: run events forward verbatim, a run-level
//! error emits a sanitized envelope BEFORE the terminal signal, and
//! `agent_stream_closed` fires EXACTLY ONCE. Session-lifecycle signals
//! (compaction / session-info) ride a separate `agent_session_lifecycle`
//! channel so they can never enter the run-event reducer or disturb that
//! invariant.
//!
//! `send_message` borrows the session `&mut`, so the driver task owns it for the
//! turn; the cancel and steering handles are cloned out first and kept in the
//! process-level [`run_controls`] registry, registered on entry and removed at
//! the closed emit site, so [`abort_run`] / [`steer_run`] reach a live run only.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use hand_ai_model::{ImageContent, Message, UserMessage};
use hand_coding_agent::{AgentSession, AgentSessionEvent, CodingAgentError};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::models::AppError;
use crate::services::agent_run_types::AgentRunAttachment;

/// What a single [`AgentSessionEvent`] maps to on HandBox's event surface.
/// Kept out of the `subscribe` closure so the policy is unit-testable without
/// spawning a task or touching the network.
#[derive(Debug, Clone, PartialEq)]
pub enum MappedEvent {
    /// Forward the inner `AgentEvent` JSON on `agent_stream_event` as the
    /// `event` field of `{ sessionId, event }`.
    Forward(Value),
    /// A session-lifecycle signal (compaction / session-info) emitted on the
    /// SEPARATE `agent_session_lifecycle` channel as `{ sessionId, kind, .. }`.
    /// Never enters the `agent_stream_event` reducer.
    Lifecycle(Value),
    /// An out-of-band signal with no frontend surface (a session `Error`):
    /// logged for diagnostics and dropped.
    Logged,
}

/// Per-image byte cap enforced at the IPC boundary. The frontend already limits
/// attachments, but the backend never trusts it: an oversize image is dropped so
/// unbounded bytes never get base64'd into the model context.
const ATTACHMENT_BYTE_CAP: usize = 10 * 1024 * 1024;
/// Per-turn attachment count cap, so a pathological request cannot blow up the
/// assembled message.
const ATTACHMENT_MAX_COUNT: usize = 16;

/// Validate `attachments` at the IPC boundary and convert the surviving images
/// into `ImageContent` blocks for `send_message_with_images`.
///
/// Non-`image/*` mimes, images over [`ATTACHMENT_BYTE_CAP`], and everything past
/// [`ATTACHMENT_MAX_COUNT`] are dropped SILENTLY — the turn still runs, and an
/// all-dropped batch yields an empty `Vec` that falls back to plain text.
pub fn images_from_attachments(attachments: &[AgentRunAttachment]) -> Vec<ImageContent> {
    let mut images: Vec<ImageContent> =
        Vec::with_capacity(attachments.len().min(ATTACHMENT_MAX_COUNT));
    for att in attachments.iter().take(ATTACHMENT_MAX_COUNT) {
        if !att.mime_type.starts_with("image/") {
            // Non-image attachment: defensively dropped (frontend pre-filters).
            continue;
        }
        if att.data.len() > ATTACHMENT_BYTE_CAP {
            // Oversize image: dropped so unbounded bytes never enter context.
            continue;
        }
        let data_b64 = BASE64_STANDARD.encode(&att.data);
        images.push(ImageContent::new(data_b64, att.mime_type.clone()));
    }
    images
}

/// Generic, non-leaking replacement for an assistant message's in-band
/// `errorMessage` (`stopReason == "error"`). The upstream transport puts the
/// raw provider response body's `error` string here (proxy.rs phase 2), which
/// can echo a key fragment (e.g. an OpenAI 401 body repeats the offending
/// `sk-...`) — so it MUST NOT reach the UI / timeline / logs verbatim.
const INBAND_ERROR_REDACTION: &str = "the model returned an error";

/// Scrub the in-band `errorMessage` from any assistant message inside a
/// serialized `AgentEvent` value, in place.
///
/// SECURITY (in-band leg of the never-echo-raw-provider-text contract): an
/// error-stopped `AssistantMessage` carries an `errorMessage` taken from the raw
/// provider body, and it rides an `Ok` stream, so it never passes through
/// [`sanitize_coding_agent_error`]. Only that field is replaced; every other
/// field — crucially the text `content` — is left untouched, so already-streamed
/// assistant text survives while the upstream body never leaks.
///
/// The scrub walks every `message` / `messages` an `AgentEvent` can carry and
/// touches only objects whose `stopReason == "error"`; a normal finished turn is
/// left byte-for-byte unchanged.
fn redact_inband_error_messages(event_json: &mut Value) {
    /// Redact one message object if it is an error-stopped assistant message.
    fn redact_message(message: &mut Value) {
        if message.get("stopReason").and_then(Value::as_str) == Some("error")
            && message.get("errorMessage").is_some()
        {
            message["errorMessage"] = Value::String(INBAND_ERROR_REDACTION.to_string());
        }
    }

    let Some(obj) = event_json.as_object_mut() else {
        return;
    };
    // Single-message variants: MessageStart / MessageUpdate / MessageEnd / TurnEnd.
    if let Some(message) = obj.get_mut("message") {
        redact_message(message);
    }
    // AgentEnd carries the whole turn's messages.
    if let Some(messages) = obj.get_mut("messages").and_then(Value::as_array_mut) {
        for message in messages.iter_mut() {
            redact_message(message);
        }
    }
}

/// Map an [`AgentSessionEvent`] to its HandBox event-surface action.
///
/// - `Agent(e)` → `Forward`: the inner `AgentEvent` serializes to exactly the
///   shape the frontend consumes, except that an in-band `errorMessage` is
///   scrubbed first (see [`redact_inband_error_messages`]).
/// - `CompactionStart` / `CompactionEnd` / `SessionInfoChanged` → `Lifecycle`:
///   a tagged `{ kind, .. }` value for the separate lifecycle channel (compaction
///   indicator + sidebar rename), never the run-event reducer.
/// - `Error(_)` → `Logged`: a bare session error has no frontend surface, since
///   the run-level error path is `send_message` returning `Err`.
fn map_session_event(event: &AgentSessionEvent) -> MappedEvent {
    match event {
        AgentSessionEvent::Agent(agent_event) => {
            // A serialize failure is structural and must never break the
            // stream — fall back to a diagnostic object.
            let mut value = serde_json::to_value(agent_event.as_ref())
                .unwrap_or_else(|e| json!({ "type": "serializeError", "message": e.to_string() }));
            // SECURITY: scrub any in-band raw-provider `errorMessage` before it
            // reaches the frontend (the in-band leg never passes through the
            // run-level sanitizer). Text content is preserved.
            redact_inband_error_messages(&mut value);
            MappedEvent::Forward(value)
        }
        // Lifecycle signals → the dedicated channel; `kind` is the discriminator
        // the frontend narrows on. `CompactionEnd`'s `summary` rides the wire
        // but is deliberately not rendered into the timeline.
        AgentSessionEvent::CompactionStart => {
            MappedEvent::Lifecycle(json!({ "kind": "compaction_start" }))
        }
        AgentSessionEvent::CompactionEnd { summary } => {
            MappedEvent::Lifecycle(json!({ "kind": "compaction_end", "summary": summary }))
        }
        AgentSessionEvent::SessionInfoChanged { name } => {
            MappedEvent::Lifecycle(json!({ "kind": "session_info_changed", "name": name }))
        }
        // A bare session error has no frontend surface. Logged, dropped.
        AgentSessionEvent::Error(_) => MappedEvent::Logged,
    }
}

/// Map a run-level [`CodingAgentError`] to a **sanitized** [`AppError`]
/// `{ code, message, hint }` for the `agent_stream_error` envelope.
///
/// SECURITY: never echo raw provider / transport error text — it can carry an
/// API key or a credentialed URL. Each variant maps to a stable AppError code
/// plus a generic-but-useful hint.
fn sanitize_coding_agent_error(err: &CodingAgentError) -> AppError {
    use hand_agent::AgentError;
    use hand_ai_model::ClientError;

    match err {
        // The model loop failed at run level; classify by `AgentError`.
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
        // Session/settings/tool/serialization/io failures from the coding-agent
        // layer: our own assembly, but still a generic internal code.
        _ => AppError::internal_error("the agent run failed to complete"),
    }
}

/// Event sink for a coding-agent run — the single choke point through which a
/// driven turn reaches HandBox's three Tauri channels.
///
/// `on_event` receives `{ sessionId, event }`, `on_closed` the terminal
/// `{ sessionId }` EXACTLY ONCE, and the optional `on_error` the sanitized
/// `{ sessionId, error }` envelope BEFORE `on_closed`. Without `on_error` the
/// envelope falls back to `on_event`, so the error still reaches the UI without
/// a second closed emit site.
#[derive(Clone)]
pub struct CodingRunSink {
    on_event: Arc<dyn Fn(Value) + Send + Sync>,
    on_closed: Arc<dyn Fn(Value) + Send + Sync>,
    on_error: Option<Arc<dyn Fn(Value) + Send + Sync>>,
    on_lifecycle: Option<Arc<dyn Fn(Value) + Send + Sync>>,
}

impl CodingRunSink {
    /// Construct a sink. `on_event` receives `{ sessionId, event }`; `on_closed`
    /// receives the terminal `{ sessionId }`. The error envelope falls back to
    /// `on_event` until [`CodingRunSink::with_error`] injects a dedicated
    /// channel; lifecycle signals are dropped until
    /// [`CodingRunSink::with_lifecycle`] injects a channel.
    pub fn new(
        on_event: Arc<dyn Fn(Value) + Send + Sync>,
        on_closed: Arc<dyn Fn(Value) + Send + Sync>,
    ) -> Self {
        Self {
            on_event,
            on_closed,
            on_error: None,
            on_lifecycle: None,
        }
    }

    /// Inject a dedicated channel for the run-level `Err` envelope, yielding a
    /// sink that routes `{ sessionId, error }` to `on_error` instead of
    /// `on_event`.
    pub fn with_error(mut self, on_error: Arc<dyn Fn(Value) + Send + Sync>) -> Self {
        self.on_error = Some(on_error);
        self
    }

    /// Inject a dedicated channel for session-lifecycle signals (compaction /
    /// session-info), routing the tagged `{ sessionId, kind, .. }` payload to
    /// `on_lifecycle`. When absent, lifecycle signals are dropped (they never
    /// fall back to `on_event` — they must not enter the run-event reducer).
    pub fn with_lifecycle(mut self, on_lifecycle: Arc<dyn Fn(Value) + Send + Sync>) -> Self {
        self.on_lifecycle = Some(on_lifecycle);
        self
    }
}

/// Abort / steer handles for a driven run, cloned out before the session moves
/// into the background task — afterwards the `&mut self` borrow inside
/// `send_message` makes the session unreachable.
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

/// Live steer / abort controls for one driven run. These are the SAME `Arc`s the
/// in-flight `send_message` wired into its cancel token and
/// `get_steering_messages` closure, so touching them reaches the running turn.
struct RunControl {
    cancel: Arc<Mutex<hand_agent::CancellationToken>>,
    steering: Arc<Mutex<Vec<Message>>>,
}

/// Process-level `session_id → RunControl` registry.
///
/// Companion to `commands::agent_run::active_coding_runs`: that set gates
/// concurrency, this map carries the live steer / abort handles. It is
/// process-level because the driver task owns the session for the turn, leaving
/// nowhere instance-level for the stateless command handlers to reach. An entry
/// exists for exactly the run's lifetime; an absent entry is a clean no-op.
fn run_controls() -> &'static Mutex<HashMap<String, RunControl>> {
    static CONTROLS: OnceLock<Mutex<HashMap<String, RunControl>>> = OnceLock::new();
    CONTROLS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Inject `text` as a user [`Message`] into the session's IN-FLIGHT turn.
///
/// Empty / whitespace-only text enqueues nothing, and a session with no active
/// run is a CLEAN no-op — the frontend may race a steer against a run that just
/// ended. Otherwise the message joins the CURRENT turn via the steering queue,
/// which the agent loop drains at the next mid-turn boundary; the follow-up
/// queue is deliberately untouched so nothing auto-continues after the turn.
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

/// Abort the session's in-flight turn by flipping its cancellation token AND
/// fail-closing any approval the turn is parked on.
///
/// An unknown / already-finished session is a CLEAN no-op. Cancelling the token
/// makes the agent loop unwind at its next await point and synthesize a
/// `stopReason=aborted` turn, so `agent_stream_closed` still fires exactly once.
/// The registry entry is NOT removed here — removal stays owned by the driver
/// task's closed emit site, so a stale abort cannot drop a live entry.
///
/// PENDING-APPROVAL FAIL-CLOSE: the permission hook awaits the user's decision
/// on a bare `rx.await` that does not race the cancel token, so the token alone
/// cannot unblock a turn parked on consent. Dropping the pending sender resolves
/// that await to a fail-closed `Cancel`, guaranteeing the dangerous tool never
/// runs; a late "allow" then finds no entry and is likewise a no-op.
pub fn abort_run(session_id: &str) {
    let controls = run_controls().lock().unwrap();
    if let Some(control) = controls.get(session_id) {
        control.cancel.lock().unwrap().cancel();
    }
    // Drop the run-controls lock before touching the approval registry to avoid
    // ordering two unrelated process-global locks under one critical section.
    drop(controls);
    // Fail-close any approval the (now-cancelled) turn is parked on, so the bare
    // approval await unblocks and the dangerous tool never runs.
    crate::services::agent_permission::deny_pending_for_session(session_id);
    // Same for an open question panel: `ask_question` awaits a bare oneshot, so
    // the cancel token alone would leave the turn parked on it forever.
    crate::services::extensions::ask_question::cancel_pending_questions_for_session(session_id);
}

/// Register a run's steer / abort controls under `session_id`. Called before the
/// driver task is spawned, so a steer / abort issued the instant the command
/// returns already reaches the run.
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
/// Non-blocking: returns [`RunDriveHandles`] immediately while the turn runs in
/// the background and events arrive asynchronously via `sink`.
///
/// Lifecycle guarantees:
/// - `Agent` events forward on `on_event` as `{ sessionId, event }` in order;
/// - lifecycle signals go to the sink's lifecycle channel, never `on_event`;
/// - a run-level `Err` emits the sanitized envelope BEFORE closing;
/// - `on_closed` fires EXACTLY ONCE, regardless of Ok/Err.
///
/// An empty `images` drives the plain-text path, so a turn whose attachments
/// were all dropped at the boundary still runs normally.
pub fn drive_agent_run(
    mut session: AgentSession,
    session_id: String,
    input: String,
    images: Vec<ImageContent>,
    sink: CodingRunSink,
) -> RunDriveHandles {
    // Capture the handles BEFORE the session moves into the task, and register
    // them so `steer_run` / `abort_run` reach this run; the entry is removed at
    // the closed emit site below.
    let cancel = session.cancel_handle();
    let steering = session.steering_queue_handle();
    register_run(&session_id, Arc::clone(&cancel), Arc::clone(&steering));

    // The subscribed callback is invoked synchronously by the session for each
    // event emitted during `send_message`.
    let event_sink = Arc::clone(&sink.on_event);
    let lifecycle_sink = sink.on_lifecycle.clone();
    let event_session = session_id.clone();
    session.subscribe(
        move |event: AgentSessionEvent| match map_session_event(&event) {
            MappedEvent::Forward(event_json) => {
                event_sink(json!({
                    "sessionId": event_session,
                    "event": event_json,
                }));
            }
            // Dedicated channel only, with `sessionId` merged into the payload.
            MappedEvent::Lifecycle(mut payload) => {
                if let Some(emit_lifecycle) = &lifecycle_sink {
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert("sessionId".to_string(), json!(event_session));
                    }
                    emit_lifecycle(payload);
                }
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
        // Streaming events have already been forwarded through the subscribe
        // callback by the time this resolves.
        let images_arg = if images.is_empty() {
            None
        } else {
            Some(images)
        };
        let result = session.send_message_with_images(&input, images_arg).await;

        // Emit the sanitized envelope BEFORE closing. An in-band
        // stop_reason=error turn is an `Ok` and never reaches here, so an error
        // is never reported twice.
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

        // The single closed emit site: fires exactly once for both Ok and Err.
        on_closed(json!({ "sessionId": closed_session }));

        // Deregister after the closed emit, so a steer / abort that observes the
        // run as still-registered always targets a live turn.
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

    /// A finished assistant `Message` carrying `text`, so synthetic
    /// `AgentEvent`s look realistic.
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

    /// A finished assistant `Message` carrying real (non-zero) token usage — the
    /// shape of a NORMAL turn the frontend renders a usage row for.
    fn assistant_message_with_usage(input: u64, output: u64) -> Message {
        let usage = Usage {
            input,
            output,
            total_tokens: input + output,
            ..Usage::default()
        };
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![AssistantContentBlock::Text(TextContent::new(
                "done".to_string(),
            ))],
            api: Api::OpenAICompletions,
            provider: hand_ai_model::types::Provider::OpenAI,
            model: "gpt-4o".to_string(),
            usage,
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
            response_model: None,
            response_id: None,
            diagnostics: None,
        })
    }

    /// An aborted-turn assistant `Message`, matching what hand-agent's
    /// `synthesize_aborted_message` produces: empty content, zeroed usage,
    /// `stop_reason = Aborted`. The frontend's usage suppression keys off it.
    fn aborted_assistant_message() -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![],
            api: Api::OpenAICompletions,
            provider: hand_ai_model::types::Provider::OpenAI,
            model: "gpt-4o".to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Aborted,
            error_message: Some("Aborted by caller".to_string()),
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
        lifecycle: Arc<StdMutex<Vec<Value>>>,
    }

    impl CapturingSink {
        fn into_run_sink(self) -> CodingRunSink {
            let events = Arc::clone(&self.events);
            let closed = Arc::clone(&self.closed);
            let errors = Arc::clone(&self.errors);
            let lifecycle = Arc::clone(&self.lifecycle);
            CodingRunSink::new(
                Arc::new(move |v| events.lock().unwrap().push(v)),
                Arc::new(move |v| closed.lock().unwrap().push(v)),
            )
            .with_error(Arc::new(move |v| errors.lock().unwrap().push(v)))
            .with_lifecycle(Arc::new(move |v| lifecycle.lock().unwrap().push(v)))
        }
    }

    /// Replays a scripted event sequence through the same subscribe-callback +
    /// terminal-close logic `drive_agent_run` uses, without an `AgentSession` or
    /// the network. `error` mirrors a `send_message` outcome: `Some` exercises
    /// the envelope-before-closed path.
    fn replay_through_sink(
        session_id: &str,
        sink: &CodingRunSink,
        events: Vec<AgentSessionEvent>,
        error: Option<CodingAgentError>,
    ) {
        for event in &events {
            match map_session_event(event) {
                MappedEvent::Forward(event_json) => (sink.on_event)(json!({
                    "sessionId": session_id,
                    "event": event_json,
                })),
                MappedEvent::Lifecycle(mut payload) => {
                    if let Some(emit_lifecycle) = &sink.on_lifecycle {
                        if let Some(obj) = payload.as_object_mut() {
                            obj.insert("sessionId".to_string(), json!(session_id));
                        }
                        emit_lifecycle(payload);
                    }
                }
                MappedEvent::Logged => {}
            }
        }
        // Terminal sequencing: error envelope BEFORE the single closed emit,
        // matching the spawned task.
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

    /// An `AgentSessionEvent::Agent` stream maps to `{ sessionId, event }`
    /// shapes and produces EXACTLY ONE closed.
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

        // Each payload is exactly `{ sessionId, event }`, the event carrying the
        // snake_case `type` tag the frontend reads.
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

        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
        assert_eq!(sink.errors.lock().unwrap().len(), 0, "no error on Ok path");
        assert_eq!(
            sink.closed.lock().unwrap()[0].get("sessionId").unwrap(),
            session_id
        );
    }

    /// Pins the usage wire contract the frontend's suppression predicate keys
    /// off: a normal finalized turn forwards its real non-zero usage, while an
    /// aborted turn forwards all zeros — never the preceding turn's numbers.
    #[test]
    fn forwarded_message_end_usage_is_real_for_normal_turn_and_zero_for_aborted() {
        let session_id = "sess-usage";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        // Turn 1 consumed real tokens; turn 2 is aborted and must not inherit
        // turn 1's usage.
        let events = vec![
            AgentSessionEvent::Agent(Box::new(AgentEvent::MessageEnd {
                message: assistant_message_with_usage(123, 45),
            })),
            AgentSessionEvent::Agent(Box::new(AgentEvent::MessageEnd {
                message: aborted_assistant_message(),
            })),
        ];

        replay_through_sink(session_id, &run_sink, events, None);

        let captured = sink.events.lock().unwrap();
        assert_eq!(captured.len(), 2, "both message_end events forwarded");

        // Normal turn: real non-zero usage + stopReason "stop".
        let normal = captured[0].get("event").unwrap().get("message").unwrap();
        assert_eq!(normal.get("stopReason").unwrap(), "stop");
        let normal_usage = normal.get("usage").unwrap();
        assert_eq!(normal_usage.get("input").unwrap(), 123);
        assert_eq!(normal_usage.get("output").unwrap(), 45);
        assert_eq!(normal_usage.get("totalTokens").unwrap(), 168);

        // Aborted turn: zeros across the board, not turn 1's 123/45.
        let aborted = captured[1].get("event").unwrap().get("message").unwrap();
        assert_eq!(aborted.get("stopReason").unwrap(), "aborted");
        let aborted_usage = aborted.get("usage").unwrap();
        assert_eq!(aborted_usage.get("input").unwrap(), 0);
        assert_eq!(aborted_usage.get("output").unwrap(), 0);
        assert_eq!(aborted_usage.get("totalTokens").unwrap(), 0);
    }

    /// Lifecycle events and a bare session error never reach
    /// `agent_stream_event`; the run still closes exactly once.
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

        assert_eq!(
            sink.events.lock().unwrap().len(),
            1,
            "only Agent events reach agent_stream_event"
        );
        // The bare Error reaches neither channel.
        assert_eq!(
            sink.lifecycle.lock().unwrap().len(),
            3,
            "compaction start/end + session-info reach the lifecycle channel"
        );
        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
    }

    /// Lifecycle signals reach their channel as a tagged `{ sessionId, kind, .. }`
    /// payload — the shape the frontend narrows on for the compaction indicator
    /// and sidebar title — and never land on the run-event channel.
    #[test]
    fn lifecycle_signals_carry_session_id_and_kind_on_lifecycle_channel() {
        let session_id = "sess-lifecycle";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        let events = vec![
            AgentSessionEvent::CompactionStart,
            AgentSessionEvent::CompactionEnd {
                summary: "summary text".into(),
            },
            AgentSessionEvent::SessionInfoChanged {
                name: Some("new title".into()),
            },
        ];

        replay_through_sink(session_id, &run_sink, events, None);

        assert_eq!(
            sink.events.lock().unwrap().len(),
            0,
            "lifecycle signals never reach agent_stream_event"
        );

        let lifecycle = sink.lifecycle.lock().unwrap();
        assert_eq!(lifecycle.len(), 3, "all three lifecycle signals captured");

        for payload in lifecycle.iter() {
            assert_eq!(payload.get("sessionId").unwrap(), session_id);
            assert!(payload.get("kind").and_then(Value::as_str).is_some());
        }
        assert_eq!(lifecycle[0].get("kind").unwrap(), "compaction_start");
        assert_eq!(lifecycle[1].get("kind").unwrap(), "compaction_end");
        assert_eq!(lifecycle[1].get("summary").unwrap(), "summary text");
        assert_eq!(lifecycle[2].get("kind").unwrap(), "session_info_changed");
        assert_eq!(lifecycle[2].get("name").unwrap(), "new title");
    }

    /// A run-level `Err` emits a sanitized `{ sessionId, error }` envelope on
    /// the dedicated error channel BEFORE the single closed signal, and the
    /// envelope never echoes raw provider text.
    #[test]
    fn run_level_error_emits_sanitized_envelope_before_close() {
        let session_id = "sess-err";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        // ProviderNotFound under the model loop: the canonical run-level error.
        let err = CodingAgentError::Agent(AgentError::Client(
            hand_ai_model::ClientError::ProviderNotFound {
                api: Api::OpenAICompletions,
                model_id: "gpt-4o".to_string(),
            },
        ));

        replay_through_sink(session_id, &run_sink, vec![], Some(err));

        // The envelope lands on the dedicated error channel, not on on_event.
        let errors = sink.errors.lock().unwrap();
        assert_eq!(errors.len(), 1, "one sanitized error envelope");
        assert_eq!(errors[0].get("sessionId").unwrap(), session_id);
        let error_obj = errors[0].get("error").unwrap();
        assert_eq!(error_obj.get("code").unwrap(), "AUTH_ERROR");
        assert_eq!(sink.events.lock().unwrap().len(), 0);
        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
    }

    /// A mid-run disconnect surfacing as a run-level `Err` does NOT retract the
    /// assistant text already forwarded: the sanitized envelope is additive and
    /// lands before the single closed signal.
    #[test]
    fn mid_run_error_does_not_retract_already_forwarded_text() {
        let session_id = "sess-disconnect";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        // Text streams, then the connection drops and the run resolves to a
        // run-level NETWORK error.
        let streamed = vec![
            AgentSessionEvent::Agent(Box::new(AgentEvent::AgentStart)),
            AgentSessionEvent::Agent(Box::new(AgentEvent::MessageEnd {
                message: assistant_message("answer streamed before the drop"),
            })),
        ];
        let disconnect = CodingAgentError::Agent(AgentError::Proxy {
            status: 502,
            message: "connection reset by peer".to_string(),
        });

        replay_through_sink(session_id, &run_sink, streamed, Some(disconnect));

        let events = sink.events.lock().unwrap();
        assert_eq!(
            events.len(),
            2,
            "already-forwarded events are not retracted"
        );
        let text = events[1]
            .get("event")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array)
            .and_then(|b| b.first())
            .and_then(|b| b.get("text"))
            .and_then(Value::as_str)
            .unwrap();
        assert_eq!(text, "answer streamed before the drop");
        drop(events);

        // The envelope carries the normalized NETWORK code.
        let errors = sink.errors.lock().unwrap();
        assert_eq!(errors.len(), 1, "exactly one error envelope");
        assert_eq!(
            errors[0].get("error").unwrap().get("code").unwrap(),
            "NETWORK_ERROR"
        );
        drop(errors);

        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
    }

    /// `map_session_event` classification is exhaustive and stable: Agent
    /// forwards, compaction / session-info become tagged lifecycle signals, a
    /// bare session error logs.
    #[test]
    fn map_session_event_classifies_each_variant() {
        assert!(matches!(
            map_session_event(&AgentSessionEvent::Agent(Box::new(AgentEvent::AgentStart))),
            MappedEvent::Forward(_)
        ));

        let MappedEvent::Lifecycle(start) = map_session_event(&AgentSessionEvent::CompactionStart)
        else {
            panic!("CompactionStart must map to a lifecycle signal");
        };
        assert_eq!(start.get("kind").unwrap(), "compaction_start");

        // The summary rides the wire but the frontend only toggles the indicator.
        let MappedEvent::Lifecycle(end) = map_session_event(&AgentSessionEvent::CompactionEnd {
            summary: "compacted 12 messages".into(),
        }) else {
            panic!("CompactionEnd must map to a lifecycle signal");
        };
        assert_eq!(end.get("kind").unwrap(), "compaction_end");
        assert_eq!(end.get("summary").unwrap(), "compacted 12 messages");

        let MappedEvent::Lifecycle(info) =
            map_session_event(&AgentSessionEvent::SessionInfoChanged {
                name: Some("renamed session".into()),
            })
        else {
            panic!("SessionInfoChanged must map to a lifecycle signal");
        };
        assert_eq!(info.get("kind").unwrap(), "session_info_changed");
        assert_eq!(info.get("name").unwrap(), "renamed session");

        assert_eq!(
            map_session_event(&AgentSessionEvent::Error("x".into())),
            MappedEvent::Logged
        );
    }

    fn image_attachment(name: &str, mime: &str, data: &[u8]) -> AgentRunAttachment {
        AgentRunAttachment {
            name: name.to_string(),
            mime_type: mime.to_string(),
            data: data.to_vec(),
        }
    }

    #[test]
    fn normal_image_survives_and_is_base64_encoded() {
        let raw = b"\x89PNG\r\n\x1a\n fake png bytes";
        let images = images_from_attachments(&[image_attachment("shot.png", "image/png", raw)]);
        assert_eq!(images.len(), 1, "a normal image survives");
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(images[0].data, BASE64_STANDARD.encode(raw));
    }

    #[test]
    fn non_image_attachment_is_dropped() {
        let images =
            images_from_attachments(&[image_attachment("notes.txt", "text/plain", b"hello")]);
        assert!(images.is_empty(), "non-image attachments are dropped");
    }

    #[test]
    fn mixed_attachments_keep_only_images() {
        let images = images_from_attachments(&[
            image_attachment("a.png", "image/png", b"img-a"),
            image_attachment("b.txt", "text/plain", b"text-b"),
            image_attachment("c.jpg", "image/jpeg", b"img-c"),
        ]);
        assert_eq!(images.len(), 2, "only the two images survive");
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(images[1].mime_type, "image/jpeg");
    }

    #[test]
    fn oversize_image_is_dropped_normal_kept() {
        let small = vec![0u8; 32];
        let oversize = vec![0u8; ATTACHMENT_BYTE_CAP + 1];
        let images = images_from_attachments(&[
            image_attachment("small.png", "image/png", &small),
            image_attachment("huge.png", "image/png", &oversize),
        ]);
        assert_eq!(images.len(), 1, "only the in-cap image survives");
        assert_eq!(images[0].data, BASE64_STANDARD.encode(&small));
    }

    #[test]
    fn overflow_attachment_count_is_truncated() {
        let attachments: Vec<AgentRunAttachment> = (0..(ATTACHMENT_MAX_COUNT + 5))
            .map(|i| image_attachment(&format!("img{i}.png"), "image/png", b"x"))
            .collect();
        let images = images_from_attachments(&attachments);
        assert_eq!(
            images.len(),
            ATTACHMENT_MAX_COUNT,
            "attachment count is bounded to ATTACHMENT_MAX_COUNT"
        );
    }

    /// An empty batch, or one where everything was dropped, yields an empty
    /// `Vec`, so the driver falls back to the plain-text path and still runs.
    #[test]
    fn all_dropped_attachments_yield_empty_so_turn_still_runs() {
        let images = images_from_attachments(&[]);
        assert!(images.is_empty(), "no attachments → empty image set");

        let all_invalid = images_from_attachments(&[
            image_attachment("a.txt", "text/plain", b"nope"),
            image_attachment("huge.png", "image/png", &vec![0u8; ATTACHMENT_BYTE_CAP + 1]),
        ]);
        assert!(
            all_invalid.is_empty(),
            "an all-invalid batch collapses to the plain-text path"
        );
    }

    /// Assert a sanitized `AppError` echoes none of `secrets` in its `message`
    /// or `hint`.
    fn assert_no_leak(err: &AppError, secrets: &[&str]) {
        for secret in secrets {
            assert!(
                !err.message.contains(secret),
                "sanitized message leaked {secret:?}: {}",
                err.message
            );
            if let Some(hint) = &err.hint {
                assert!(
                    !hint.contains(secret),
                    "sanitized hint leaked {secret:?}: {hint}"
                );
            }
        }
    }

    /// The sanitizer maps every error family to a stable code (AUTH / NETWORK /
    /// RATE_LIMIT / INTERNAL) and never echoes raw transport text — no API key,
    /// credentialed URL, or upstream body may leak.
    #[test]
    fn sanitizer_maps_codes_without_leaking_raw_text() {
        // Things raw transport text could carry; none may appear in any output.
        let secrets = [
            "sk-secret",
            "sk-proj-LEAK",
            "https://api.example.com/v1?key=sk-secret",
            "Incorrect API key provided",
        ];

        let proxy_401 = CodingAgentError::Agent(AgentError::Proxy {
            status: 401,
            message: "https://api.example.com/v1?key=sk-secret rejected".to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_401);
        assert_eq!(e.code, "AUTH_ERROR");
        assert_no_leak(&e, &secrets);

        // An authorization failure shares the auth code.
        let proxy_403 = CodingAgentError::Agent(AgentError::Proxy {
            status: 403,
            message: "Incorrect API key provided: sk-proj-LEAK".to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_403);
        assert_eq!(e.code, "AUTH_ERROR");
        assert_no_leak(&e, &secrets);

        let proxy_429 = CodingAgentError::Agent(AgentError::Proxy {
            status: 429,
            message: "too many requests, key sk-secret".to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_429);
        assert_eq!(e.code, "RATE_LIMIT");
        assert_no_leak(&e, &secrets);

        // Any other status (500 / timeout / connection drop) is NETWORK_ERROR —
        // the mid-disconnect run-level code.
        let proxy_500 = CodingAgentError::Agent(AgentError::Proxy {
            status: 502,
            message: "upstream connection reset to https://api.example.com/v1?key=sk-secret"
                .to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_500);
        assert_eq!(e.code, "NETWORK_ERROR");
        assert_no_leak(&e, &secrets);

        // ProviderNotFound may reference the non-secret model id, never a key.
        let provider_not_found = CodingAgentError::Agent(AgentError::Client(
            hand_ai_model::ClientError::ProviderNotFound {
                api: Api::OpenAICompletions,
                model_id: "gpt-4o".to_string(),
            },
        ));
        let e = sanitize_coding_agent_error(&provider_not_found);
        assert_eq!(e.code, "AUTH_ERROR");
        assert!(
            e.message.contains("gpt-4o"),
            "provider-not-found may reference the non-secret model id for locatability"
        );
        assert_no_leak(&e, &secrets);

        let stream_ended = CodingAgentError::Agent(AgentError::Client(
            hand_ai_model::ClientError::StreamEndedWithoutResult,
        ));
        let e = sanitize_coding_agent_error(&stream_ended);
        assert_eq!(e.code, "NETWORK_ERROR");
        assert_no_leak(&e, &secrets);

        // The normal abort path is an Ok aborted turn; an Err-shaped abort still
        // gets a non-leaking code.
        let aborted = CodingAgentError::Agent(AgentError::Aborted);
        let e = sanitize_coding_agent_error(&aborted);
        assert_eq!(e.code, "INTERNAL_ERROR");
        assert_no_leak(&e, &secrets);

        // Catch-all AgentError text comes from our own code but still takes a
        // generic code.
        let other_agent = CodingAgentError::Agent(AgentError::Other(
            "lifecycle failure mentioning sk-secret".to_string(),
        ));
        let e = sanitize_coding_agent_error(&other_agent);
        assert_eq!(e.code, "INTERNAL_ERROR");
        assert_no_leak(&e, &secrets);

        for err in [
            CodingAgentError::Session("no session found, key sk-secret".to_string()),
            CodingAgentError::Settings("bad settings sk-secret".to_string()),
            CodingAgentError::Tool("tool blew up sk-secret".to_string()),
            CodingAgentError::Model("model assembly sk-secret".to_string()),
            CodingAgentError::Other("misc sk-secret".to_string()),
        ] {
            let e = sanitize_coding_agent_error(&err);
            assert_eq!(e.code, "INTERNAL_ERROR", "lifecycle variant code");
            assert_no_leak(&e, &secrets);
        }
    }

    /// An error-stopped assistant message rides an `Ok` stream and so bypasses
    /// the run-level sanitizer: its `errorMessage` must be scrubbed at the
    /// mapping layer while the already-streamed text survives verbatim.
    #[test]
    fn inband_error_message_is_scrubbed_but_text_is_preserved() {
        // The raw upstream body a 401 could carry, echoing the offending key.
        let raw_upstream = "Incorrect API key provided: sk-proj-LEAK-1234";
        let mut error_msg = assistant_message("partial answer before the drop");
        if let Message::Assistant(m) = &mut error_msg {
            m.stop_reason = StopReason::Error;
            m.error_message = Some(raw_upstream.to_string());
        }

        let event =
            AgentSessionEvent::Agent(Box::new(AgentEvent::MessageEnd { message: error_msg }));

        let mapped = map_session_event(&event);
        let MappedEvent::Forward(value) = mapped else {
            panic!("an Agent MessageEnd must be Forward");
        };

        let message = value.get("message").expect("MessageEnd carries a message");
        let forwarded_err = message
            .get("errorMessage")
            .and_then(Value::as_str)
            .expect("error turn keeps an errorMessage field (now generic)");
        assert!(
            !forwarded_err.contains("sk-proj-LEAK"),
            "in-band errorMessage must not echo the upstream key fragment: {forwarded_err}"
        );
        assert!(
            !forwarded_err.contains("Incorrect API key"),
            "in-band errorMessage must not echo the raw upstream body: {forwarded_err}"
        );
        assert_eq!(forwarded_err, INBAND_ERROR_REDACTION);
        // The error signal survives so the frontend still renders the turn as
        // errored.
        assert_eq!(
            message.get("stopReason").and_then(Value::as_str),
            Some("error")
        );

        let text = message
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| blocks.first())
            .and_then(|b| b.get("text"))
            .and_then(Value::as_str)
            .expect("the streamed text block survives the scrub");
        assert_eq!(text, "partial answer before the drop");
    }

    /// The scrub touches only error-stopped messages; a normal finished turn is
    /// left byte-for-byte unchanged.
    #[test]
    fn inband_scrub_leaves_normal_turn_untouched() {
        let event = AgentSessionEvent::Agent(Box::new(AgentEvent::MessageEnd {
            message: assistant_message("all good"),
        }));
        let MappedEvent::Forward(value) = map_session_event(&event) else {
            panic!("Agent event must Forward");
        };
        let message = value.get("message").unwrap();
        assert_eq!(
            message.get("stopReason").and_then(Value::as_str),
            Some("stop")
        );
        // A healthy turn never serialized an errorMessage (skip_serializing_if).
        assert!(
            message.get("errorMessage").is_none(),
            "a normal turn must not gain an errorMessage from the scrub"
        );
    }

    // The run-control registry is process-global, so each test below uses a
    // fresh random `session_id` to stay isolated from the rest of the binary.

    fn fresh_session_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Register a run the way `drive_agent_run` does, returning clones of both
    /// handles so a test observes the same `Arc`s the live turn would.
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

    fn is_registered(session_id: &str) -> bool {
        run_controls().lock().unwrap().contains_key(session_id)
    }

    /// The lifetime contract `drive_agent_run` relies on: register on entry,
    /// deregister at the closed emit site.
    #[test]
    fn register_then_deregister_brackets_the_run() {
        let session_id = fresh_session_id();
        assert!(!is_registered(&session_id), "absent before register");

        let _handles = register_test_run(&session_id);
        assert!(is_registered(&session_id), "present after register");

        deregister_run(&session_id);
        assert!(!is_registered(&session_id), "absent after deregister");
    }

    /// `steer_run` pushes onto the SAME queue the live turn drains, so a mid-run
    /// steer joins the current turn as a user message.
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

    /// Blank steer text enqueues nothing and leaves the active run undisturbed.
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

    /// Steering a session with no active run is a clean no-op; the frontend may
    /// race a steer against a run that just ended.
    #[test]
    fn steer_with_no_active_run_is_noop() {
        let session_id = fresh_session_id();
        assert!(!is_registered(&session_id));
        steer_run(&session_id, "hello".to_string());
        assert!(
            !is_registered(&session_id),
            "steer never registers a run on its own"
        );
    }

    /// `abort_run` flips the SAME token the live turn drives on, so the agent
    /// loop unwinds and the run finishes "aborted". The entry is NOT removed —
    /// removal stays owned by the closed emit site.
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

    /// Aborting a session with no active run is a clean no-op.
    #[test]
    fn abort_with_no_active_run_is_noop() {
        let session_id = fresh_session_id();
        assert!(!is_registered(&session_id));
        abort_run(&session_id);
        assert!(!is_registered(&session_id));
    }

    /// `abort_run` must also fail-close a parked approval: the permission hook
    /// awaits on a bare `rx.await` that does not race the cancel token, so
    /// without that leg the tool call would hang and a late "allow" could still
    /// run it.
    ///
    /// Mirrors the production wiring where the hook is driven with the
    /// coding-agent's internal in-memory `ExtensionContext.session_id`, which
    /// differs from the HandBox session UUID `abort_run` is called with. The
    /// pending registry must therefore key off the extension's HandBox id, or
    /// the abort can never match.
    #[tokio::test]
    async fn abort_run_unblocks_a_pending_approval_to_cancel() {
        use crate::services::agent_permission::{PermissionExtension, APPROVAL_REQUEST_EVENT};
        use hand_coding_agent::core::extensions::api::ToolCallEvent;
        use hand_coding_agent::{Extension, ExtensionContext, HookDecision};
        use std::path::Path;

        // The HandBox DB session UUID: what the IPC layer passes to `abort_run`
        // and what `build_agent_session` threads into the PermissionExtension.
        let handbox_session_id = fresh_session_id();
        // What the host actually puts in `cx.session_id` for this turn: the
        // coding-agent's internal id, unrelated to the HandBox UUID.
        let coding_agent_internal_id = format!("s_{}_internal", uuid::Uuid::new_v4());
        assert_ne!(
            handbox_session_id, coding_agent_internal_id,
            "the cx id and the HandBox id must differ — this is the production reality"
        );

        // Register under the HandBox UUID, as `drive_agent_run` would, so the
        // abort finds the session; the approval fail-close uses that same key.
        let (_cancel, _steering) = register_test_run(&handbox_session_id);

        // A recording emitter so we can wait for the approval request to land
        // before aborting (otherwise we'd race the await registration).
        let recorded: Arc<StdMutex<Vec<Value>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);
        let emitter: Arc<dyn Fn(Value) + Send + Sync> =
            Arc::new(move |payload| sink.lock().unwrap().push(payload));
        assert_eq!(APPROVAL_REQUEST_EVENT, "agent_approval_request");

        // Keyed off the HandBox UUID (as build_agent_session wires it), not the
        // cx id the hook is driven with.
        let ext = Arc::new(PermissionExtension::new(
            handbox_session_id.clone(),
            Some(emitter),
        ));
        let hook_ext = Arc::clone(&ext);
        let hook_cx_id = coding_agent_internal_id.clone();
        let task = tokio::spawn(async move {
            // The host passes the coding-agent internal id here, so the pending
            // registry must key off the ext's HandBox id for the abort to land.
            let cx = ExtensionContext {
                cwd: Path::new("/tmp").to_path_buf(),
                session_id: hook_cx_id,
                data_dir: Path::new("/tmp").join(".hand").join("data"),
            };
            let event = ToolCallEvent {
                tool_name: "bash".to_string(),
                arguments: json!({ "command": "rm -rf /" }),
                call_id: "call-1".to_string(),
            };
            hook_ext
                .on_before_tool_call(&cx, &event)
                .await
                .expect("permission hook never returns Err")
        });

        // Wait until the turn is parked on the approval await (request emitted).
        for _ in 0..1000 {
            if !recorded.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(
            !recorded.lock().unwrap().is_empty(),
            "the dangerous tool must have emitted an approval request before abort"
        );
        // The emitted sessionId must be the HandBox UUID the frontend routes by,
        // not the coding-agent internal id.
        assert_eq!(
            recorded.lock().unwrap()[0].get("sessionId").unwrap(),
            &Value::String(handbox_session_id.clone()),
            "the approval request must carry the HandBox session id, not the cx id"
        );

        // Abort with the HandBox UUID: flips the token AND fail-closes the
        // pending approval keyed off that same UUID.
        abort_run(&handbox_session_id);

        let decision = task
            .await
            .expect("hook task joins after abort (did not hang)");
        assert!(
            matches!(decision, HookDecision::Cancel(_)),
            "abort_run must fail-close the pending approval to Cancel even though the \
             cx.session_id differs from the abort id — the bash tool must not run"
        );

        deregister_run(&handbox_session_id);
    }

    /// Once a run deregisters, a steer / abort for that session is again a clean
    /// no-op, so no residue leaks into the next run.
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
