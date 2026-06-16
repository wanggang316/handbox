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
//! - `CompactionStart` / `CompactionEnd` / `SessionInfoChanged` are out-of-band
//!   *session lifecycle* signals — they are NOT run events and must never enter
//!   the `agent_stream_event` reducer (doing so would risk the closed-once
//!   invariant and pollute the `AgentEvent` union). They are surfaced on a
//!   SEPARATE Tauri channel (`agent_session_lifecycle`) as a tagged
//!   `{ sessionId, kind, .. }` payload so the frontend can render a
//!   distinguishable "整理上下文中" compaction indicator and reflect a renamed
//!   session in the sidebar without reopen. `CompactionEnd`'s `summary` is
//!   carried on the wire but is DELIBERATELY NOT rendered into the timeline:
//!   the indicator is the only visible artifact, the conversation continues
//!   in-line, and the turn still closes exactly once.
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

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use hand_ai_model::{ImageContent, Message, UserMessage};
use hand_coding_agent::{AgentSession, AgentSessionEvent, CodingAgentError};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

use crate::models::AppError;
use crate::services::agent_runtime::AgentRunAttachment;

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
    /// A session-lifecycle signal (compaction / session-info) emitted on the
    /// SEPARATE `agent_session_lifecycle` channel as `{ sessionId, .. }` with
    /// the carried fields merged in (the `kind` tag is set by the lifecycle
    /// classifier below). Never enters the `agent_stream_event` reducer.
    Lifecycle(Value),
    /// An out-of-band signal with no frontend surface (a session `Error`):
    /// logged for diagnostics and dropped.
    Logged,
}

/// Per-image byte cap enforced at the IPC boundary (CLAUDE.md「输入验证必须完备」).
/// The frontend already limits attachments to 10 MiB, but the backend never
/// trusts the frontend: an oversize image is defensively dropped so unbounded
/// bytes never get base64'd into the model context. Mirrors
/// `agent_runtime::ATTACHMENT_BYTE_CAP` so both engines enforce the same bound.
const ATTACHMENT_BYTE_CAP: usize = 10 * 1024 * 1024;
/// Per-turn attachment count cap. Attachments beyond this count are dropped so a
/// pathological request cannot blow up the assembled message. Mirrors
/// `agent_runtime::ATTACHMENT_MAX_COUNT`.
const ATTACHMENT_MAX_COUNT: usize = 16;

/// Validate `attachments` at the IPC boundary and convert the surviving images
/// into `ImageContent` blocks for `send_message_with_images`.
///
/// The new coding-agent driver path consumed only the prompt text and dropped
/// attachments entirely; this restores the legacy `agent_runtime` boundary
/// discipline (VAL-CARUN-018) so the same images that reached the model under
/// the old engine reach it under the new one, with the identical caps:
///
/// - non-`image/*` mimes are dropped (the frontend filters to `image/*`; this
///   is belt-and-suspenders);
/// - an image larger than [`ATTACHMENT_BYTE_CAP`] is dropped (no unbounded
///   bytes base64'd into context);
/// - only the first [`ATTACHMENT_MAX_COUNT`] attachments are considered; the
///   tail is dropped.
///
/// Every drop is SILENT — the turn still runs (an all-dropped batch yields an
/// empty `Vec`, and the caller falls back to the plain-text path). Each
/// surviving image's bytes are base64 STANDARD encoded into an `ImageContent`,
/// matching `agent_runtime::build_user_message`.
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
/// SECURITY (the in-band leg of the never-echo-raw-provider-text contract): an
/// `AssistantMessage` finalized with `stopReason == "error"` carries an
/// `errorMessage` set by the upstream transport from the raw provider response
/// body. That message rides an `Ok` stream (it is NOT a run-level `Err`, so it
/// never passes through [`sanitize_coding_agent_error`]) and would otherwise be
/// forwarded VERBATIM to the frontend. We replace only the `errorMessage` field
/// with a generic constant, leaving every other field — crucially the message's
/// text `content` — untouched, so already-streamed assistant text is preserved
/// (VAL-CARUN-010) while the raw upstream body never leaks (VAL-CARUN-013).
///
/// The scrub walks every `message` / `messages` an `AgentEvent` variant can
/// carry (`MessageEnd` / `MessageStart` / `MessageUpdate` / `TurnEnd` /
/// `AgentEnd`), redacting only objects whose `stopReason == "error"` — a normal
/// finished turn (`stopReason == "stop"`) never has an `errorMessage` and is
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
/// - `Agent(e)` → `Forward(serde_json::to_value(e))`: the inner `AgentEvent`
///   serializes to exactly the shape the legacy sink produced (same
///   `#[serde(tag = "type", rename_all = "snake_case", rename_all_fields =
///   "camelCase")]` type), so the frontend contract is unchanged — EXCEPT that
///   an in-band `errorMessage` (raw upstream body on a `stopReason == "error"`
///   message) is scrubbed before forwarding (see
///   [`redact_inband_error_messages`]), never echoing provider text to the UI.
/// - `CompactionStart` / `CompactionEnd` / `SessionInfoChanged` →
///   `Lifecycle(payload)`: a tagged `{ kind, .. }` value routed to the SEPARATE
///   `agent_session_lifecycle` channel (compaction indicator + sidebar rename),
///   never into the `agent_stream_event` reducer.
/// - `Error(_)` → `Logged`: a bare session error string has no frontend surface
///   (the run-level error main-path is `send_message` returning `Err`), so it
///   is observed for diagnostics and dropped.
fn map_session_event(event: &AgentSessionEvent) -> MappedEvent {
    match event {
        AgentSessionEvent::Agent(agent_event) => {
            // The inner `AgentEvent` is the same type the frontend already
            // consumes; a serialize failure is structural and must never break
            // the stream — fall back to a diagnostic object (mirrors the legacy
            // sink's serializeError fallback).
            let mut value = serde_json::to_value(agent_event.as_ref())
                .unwrap_or_else(|e| json!({ "type": "serializeError", "message": e.to_string() }));
            // SECURITY: scrub any in-band raw-provider `errorMessage` before it
            // reaches the frontend (the in-band leg never passes through the
            // run-level sanitizer). Text content is preserved.
            redact_inband_error_messages(&mut value);
            MappedEvent::Forward(value)
        }
        // Session-lifecycle signals → the dedicated `agent_session_lifecycle`
        // channel. `kind` is the discriminator the frontend narrows on. The
        // `summary` on `CompactionEnd` is carried for completeness but is NOT
        // rendered into the timeline (the indicator is the only visible
        // artifact — stable summary destination, VAL-CARUN-019).
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

/// Abort the session's in-flight turn by flipping its cancellation token AND
/// fail-closing any approval the turn is parked on.
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
///
/// PENDING-APPROVAL FAIL-CLOSE (VAL-CAPERM-016): the permission hook awaits the
/// user's decision on a BARE `rx.await` that does NOT race the cancel token, so
/// flipping the token alone cannot unblock a turn parked waiting for consent on a
/// write/edit/bash call. We therefore ALSO
/// [`deny_pending_for_session`](crate::services::agent_permission::deny_pending_for_session):
/// it drops the pending sender(s) for this session, resolving the await to a
/// fail-closed `Cancel` so the pending dangerous tool is guaranteed NOT to
/// execute — and a late user "allow" arriving after the abort finds no entry (a
/// clean no-op), so it can never run the tool post-abort either. A session with
/// no pending approval is a clean no-op on that leg.
pub fn abort_run(session_id: &str) {
    let controls = run_controls().lock().unwrap();
    if let Some(control) = controls.get(session_id) {
        control.cancel.lock().unwrap().cancel();
    }
    // Drop the run-controls lock before touching the approval registry to avoid
    // ordering two unrelated process-global locks under one critical section.
    drop(controls);
    // Fail-close any approval the (now-cancelled) turn is parked on, so the bare
    // approval await unblocks and the dangerous tool never runs (VAL-CAPERM-016).
    crate::services::agent_permission::deny_pending_for_session(session_id);
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
/// - session-lifecycle signals (compaction / session-info) are routed to the
///   sink's lifecycle channel, NEVER onto `on_event`, so the closed-once
///   invariant and the `AgentEvent` reducer are untouched;
/// - on `send_message` → `Err`, a sanitized `{ sessionId, error }` envelope is
///   emitted (via `on_error`, or `on_event` as fallback) BEFORE closing;
/// - `on_closed` fires EXACTLY ONCE with `{ sessionId }`, regardless of Ok/Err.
///
/// `images` carries the IPC-validated image attachments for this turn (see
/// [`images_from_attachments`]); an empty `Vec` drives the plain-text path
/// (`send_message_with_images(text, None)` ≡ `send_message(text)`), so a turn
/// whose attachments were all dropped at the boundary still runs normally.
pub fn drive_agent_run(
    mut session: AgentSession,
    session_id: String,
    input: String,
    images: Vec<ImageContent>,
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
            // Lifecycle signals go to their dedicated channel, with `sessionId`
            // merged into the tagged payload. Never onto `on_event`.
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
        // Drive exactly one turn. Streaming events have already been forwarded
        // through the subscribe callback by the time this resolves. An empty
        // `images` collapses to the plain-text path (`None` ≡ `send_message`).
        let images_arg = if images.is_empty() {
            None
        } else {
            Some(images)
        };
        let result = session.send_message_with_images(&input, images_arg).await;

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

    /// An ABORTED-turn assistant `Message`, byte-for-byte the shape hand-agent's
    /// `synthesize_aborted_message` produces: empty content, `Usage::default()`
    /// (all zeros), `stop_reason = Aborted`. The frontend keys its
    /// usage-suppression off exactly this shape (VAL-CARUN-008).
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

    /// VAL-CARUN-008 (per-turn usage; aborted/error turns don't carry
    /// misleading non-zero usage). The driver forwards `AgentEvent`s verbatim,
    /// so the usage the frontend renders is exactly the `AssistantMessage.usage`
    /// hand-agent put on the finalized turn. This pins the wire contract the
    /// frontend's usage-suppression predicate (`AgentTimeline.hasUsage`) keys off:
    ///   - a NORMAL finalized turn forwards its real, non-zero usage and
    ///     `stopReason: "stop"`;
    ///   - an ABORTED turn forwards `usage` all-zeros (input/output/totalTokens
    ///     == 0) with `stopReason: "aborted"` — never the previous turn's
    ///     numbers. So an aborted turn can NEVER surface a stale / misleading
    ///     non-zero usage downstream.
    #[test]
    fn forwarded_message_end_usage_is_real_for_normal_turn_and_zero_for_aborted() {
        let session_id = "sess-usage";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        // Turn 1: a normal turn that consumed real tokens. Turn 2: an aborted
        // turn (synthesized aborted message). The aborted turn must NOT inherit
        // turn 1's usage — hand-agent zeroes it at the source; we assert the
        // forwarded shape reflects that.
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

        // Aborted turn: zeros across the board + stopReason "aborted". Crucially
        // NOT turn 1's 123/45 — no carry-over, no misleading non-zero usage.
        let aborted = captured[1].get("event").unwrap().get("message").unwrap();
        assert_eq!(aborted.get("stopReason").unwrap(), "aborted");
        let aborted_usage = aborted.get("usage").unwrap();
        assert_eq!(aborted_usage.get("input").unwrap(), 0);
        assert_eq!(aborted_usage.get("output").unwrap(), 0);
        assert_eq!(aborted_usage.get("totalTokens").unwrap(), 0);
    }

    /// Out-of-band session-lifecycle events (compaction / session-info) and a
    /// bare session error are NOT forwarded onto `agent_stream_event`; lifecycle
    /// signals land on the dedicated lifecycle channel instead, and the run
    /// still closes exactly once (closed-once is independent of lifecycle —
    /// VAL-CARUN-019).
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

        // Only the single Agent event was forwarded onto the run-event channel.
        assert_eq!(
            sink.events.lock().unwrap().len(),
            1,
            "only Agent events reach agent_stream_event"
        );
        // The three lifecycle signals (compaction start/end + session-info)
        // landed on the dedicated lifecycle channel; the bare Error did not.
        assert_eq!(
            sink.lifecycle.lock().unwrap().len(),
            3,
            "compaction start/end + session-info reach the lifecycle channel"
        );
        assert_eq!(sink.closed.lock().unwrap().len(), 1, "closed exactly once");
    }

    /// Lifecycle signals reach the dedicated lifecycle channel as a tagged
    /// `{ sessionId, kind, .. }` payload — the wire shape the frontend narrows
    /// on to render the compaction indicator and update the sidebar title
    /// (VAL-CARUN-019 / VAL-CARUN-020). Crucially they NEVER land on the run
    /// event channel.
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

        // Nothing leaked onto the run-event channel.
        assert_eq!(
            sink.events.lock().unwrap().len(),
            0,
            "lifecycle signals never reach agent_stream_event"
        );

        let lifecycle = sink.lifecycle.lock().unwrap();
        assert_eq!(lifecycle.len(), 3, "all three lifecycle signals captured");

        // Every payload carries the session id + a discriminating kind.
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

    /// VAL-CARUN-010 (driver leg): a mid-run disconnect that surfaces as a
    /// run-level `Err` (e.g. a proxy 502 / connection reset → NETWORK_ERROR) does
    /// NOT retract the assistant text already forwarded before the break. The
    /// previously emitted events stay on `agent_stream_event`; the sanitized
    /// envelope is ADDITIVE and lands before the single closed signal. (The
    /// frontend likewise never clears `streamingText` on error/closed, so the
    /// partial answer remains visible.)
    #[test]
    fn mid_run_error_does_not_retract_already_forwarded_text() {
        let session_id = "sess-disconnect";
        let sink = CapturingSink::default();
        let run_sink = sink.clone().into_run_sink();

        // Streamed text arrives, THEN the connection drops mid-stream and the
        // run resolves to a run-level NETWORK error (no terminal MessageEnd).
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

        // The two streamed events were forwarded and remain — nothing retracted.
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

        // The envelope is additive and carries the normalized NETWORK code.
        let errors = sink.errors.lock().unwrap();
        assert_eq!(errors.len(), 1, "exactly one error envelope");
        assert_eq!(
            errors[0].get("error").unwrap().get("code").unwrap(),
            "NETWORK_ERROR"
        );
        drop(errors);

        // closed-once holds on this path too (VAL-CARUN-009).
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

        // CompactionStart → lifecycle { kind: "compaction_start" }.
        let MappedEvent::Lifecycle(start) = map_session_event(&AgentSessionEvent::CompactionStart)
        else {
            panic!("CompactionStart must map to a lifecycle signal");
        };
        assert_eq!(start.get("kind").unwrap(), "compaction_start");

        // CompactionEnd → lifecycle { kind: "compaction_end", summary }. The
        // summary rides the wire but the frontend does not render it (it only
        // toggles the indicator off) — stable summary destination.
        let MappedEvent::Lifecycle(end) = map_session_event(&AgentSessionEvent::CompactionEnd {
            summary: "compacted 12 messages".into(),
        }) else {
            panic!("CompactionEnd must map to a lifecycle signal");
        };
        assert_eq!(end.get("kind").unwrap(), "compaction_end");
        assert_eq!(end.get("summary").unwrap(), "compacted 12 messages");

        // SessionInfoChanged → lifecycle { kind: "session_info_changed", name }.
        let MappedEvent::Lifecycle(info) =
            map_session_event(&AgentSessionEvent::SessionInfoChanged {
                name: Some("renamed session".into()),
            })
        else {
            panic!("SessionInfoChanged must map to a lifecycle signal");
        };
        assert_eq!(info.get("kind").unwrap(), "session_info_changed");
        assert_eq!(info.get("name").unwrap(), "renamed session");

        // A bare session error has no frontend surface → Logged.
        assert_eq!(
            map_session_event(&AgentSessionEvent::Error("x".into())),
            MappedEvent::Logged
        );
    }

    // --- attachment IPC-boundary validation (VAL-CARUN-018) ---
    //
    // The new coding-agent driver path now runs the SAME cap/count/non-image
    // discipline the legacy `agent_runtime` enforced, so an oversize / overflow
    // / non-image attachment is silently dropped and never reaches the model
    // context. These tests pin `images_from_attachments` directly (no session,
    // no network).

    /// An `image/*` attachment in [`AgentRunAttachment`] shape.
    fn image_attachment(name: &str, mime: &str, data: &[u8]) -> AgentRunAttachment {
        AgentRunAttachment {
            name: name.to_string(),
            mime_type: mime.to_string(),
            data: data.to_vec(),
        }
    }

    /// A normal-size image survives validation and is base64 STANDARD encoded
    /// into an `ImageContent` carrying its mime — the happy path the model sees.
    #[test]
    fn normal_image_survives_and_is_base64_encoded() {
        let raw = b"\x89PNG\r\n\x1a\n fake png bytes";
        let images = images_from_attachments(&[image_attachment("shot.png", "image/png", raw)]);
        assert_eq!(images.len(), 1, "a normal image survives");
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(images[0].data, BASE64_STANDARD.encode(raw));
    }

    /// A non-image attachment is silently dropped — never base64'd into context.
    #[test]
    fn non_image_attachment_is_dropped() {
        let images =
            images_from_attachments(&[image_attachment("notes.txt", "text/plain", b"hello")]);
        assert!(images.is_empty(), "non-image attachments are dropped");
    }

    /// A mix of image + non-image keeps ONLY the image blocks (VAL-CARUN-018).
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

    /// An oversize image (> `ATTACHMENT_BYTE_CAP`) is dropped while a normal
    /// image in the same batch survives — unbounded bytes never enter context.
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

    /// Attachment count is bounded to `ATTACHMENT_MAX_COUNT`; the tail beyond
    /// the cap is dropped (VAL-CARUN-018 over-count).
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

    /// An empty batch (or one where every attachment was dropped) yields an
    /// empty `Vec`, so the driver falls back to the plain-text path and the
    /// turn still runs normally (VAL-CARUN-018: dropped, but the run still goes).
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

    /// Assert a sanitized `AppError` never echoes any of `secrets` in either its
    /// `message` or its `hint` — the never-leak contract applied to every case.
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

    /// The sanitizer maps EVERY error family to a stable code (AUTH / NETWORK /
    /// RATE_LIMIT / INTERNAL) and never echoes raw transport text (security: no
    /// API key / credentialed URL / upstream body leakage). Covers the full
    /// classification table the frontend's error rendering keys off
    /// (VAL-CARUN-011/012/013).
    #[test]
    fn sanitizer_maps_codes_without_leaking_raw_text() {
        // A credentialed URL + key fragment + upstream body the raw transport
        // text could carry; none of these may appear in any sanitized output.
        let secrets = [
            "sk-secret",
            "sk-proj-LEAK",
            "https://api.example.com/v1?key=sk-secret",
            "Incorrect API key provided",
        ];

        // --- Proxy transport: status → AUTH / RATE_LIMIT / NETWORK ---
        // 401 → AUTH_ERROR.
        let proxy_401 = CodingAgentError::Agent(AgentError::Proxy {
            status: 401,
            message: "https://api.example.com/v1?key=sk-secret rejected".to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_401);
        assert_eq!(e.code, "AUTH_ERROR");
        assert_no_leak(&e, &secrets);

        // 403 → AUTH_ERROR (authorization failure shares the auth code).
        let proxy_403 = CodingAgentError::Agent(AgentError::Proxy {
            status: 403,
            message: "Incorrect API key provided: sk-proj-LEAK".to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_403);
        assert_eq!(e.code, "AUTH_ERROR");
        assert_no_leak(&e, &secrets);

        // 429 → RATE_LIMIT.
        let proxy_429 = CodingAgentError::Agent(AgentError::Proxy {
            status: 429,
            message: "too many requests, key sk-secret".to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_429);
        assert_eq!(e.code, "RATE_LIMIT");
        assert_no_leak(&e, &secrets);

        // Any other status (500 / timeout / connection drop arrive here) →
        // NETWORK_ERROR. This is the mid-disconnect run-level error code.
        let proxy_500 = CodingAgentError::Agent(AgentError::Proxy {
            status: 502,
            message: "upstream connection reset to https://api.example.com/v1?key=sk-secret"
                .to_string(),
        });
        let e = sanitize_coding_agent_error(&proxy_500);
        assert_eq!(e.code, "NETWORK_ERROR");
        assert_no_leak(&e, &secrets);

        // --- Client errors: provider/auth/stream-empty ---
        // ProviderNotFound → AUTH_ERROR; only the (non-secret) model id is
        // referenced, never a key.
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

        // StreamEndedWithoutResult → NETWORK_ERROR (premature stream end).
        let stream_ended = CodingAgentError::Agent(AgentError::Client(
            hand_ai_model::ClientError::StreamEndedWithoutResult,
        ));
        let e = sanitize_coding_agent_error(&stream_ended);
        assert_eq!(e.code, "NETWORK_ERROR");
        assert_no_leak(&e, &secrets);

        // --- Aborted-as-Err → INTERNAL_ERROR (the normal abort path is an Ok
        // aborted turn; an Err-shaped abort still gets a non-leaking code). ---
        let aborted = CodingAgentError::Agent(AgentError::Aborted);
        let e = sanitize_coding_agent_error(&aborted);
        assert_eq!(e.code, "INTERNAL_ERROR");
        assert_no_leak(&e, &secrets);

        // --- AgentError catch-all (SchemaValidation / InvalidState / Other):
        // text originates from our own code, but still takes a generic code. ---
        let other_agent = CodingAgentError::Agent(AgentError::Other(
            "lifecycle failure mentioning sk-secret".to_string(),
        ));
        let e = sanitize_coding_agent_error(&other_agent);
        assert_eq!(e.code, "INTERNAL_ERROR");
        assert_no_leak(&e, &secrets);

        // --- Non-Agent coding-agent lifecycle variants all → INTERNAL_ERROR. ---
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

    /// VAL-CARUN-013 (in-band leg): an `Ok`-stream assistant message finalized
    /// with `stopReason == "error"` carries the raw upstream response body in its
    /// `errorMessage` (set by the proxy transport). The driver forwards it via
    /// `MessageEnd`, NOT through the run-level sanitizer — so it must be scrubbed
    /// at the mapping layer. This pins: (a) the `errorMessage` is replaced with a
    /// generic constant (no raw upstream body / key fragment leaks), and (b) the
    /// already-streamed text `content` is preserved verbatim (VAL-CARUN-010 —
    /// mid-disconnect text is never erased, whether the break lands before or
    /// after MessageEnd).
    #[test]
    fn inband_error_message_is_scrubbed_but_text_is_preserved() {
        // The raw upstream body a 401 proxy phase-2 read could carry: it echoes
        // the offending key fragment, exactly what must not reach the UI.
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
        // (a) errorMessage scrubbed — no raw body / key fragment leaks.
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
        // The error signal itself is preserved so the frontend still renders the
        // turn as errored.
        assert_eq!(
            message.get("stopReason").and_then(Value::as_str),
            Some("error")
        );

        // (b) Already-streamed text is preserved verbatim — the disconnect does
        // not erase visible content.
        let text = message
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| blocks.first())
            .and_then(|b| b.get("text"))
            .and_then(Value::as_str)
            .expect("the streamed text block survives the scrub");
        assert_eq!(text, "partial answer before the drop");
    }

    /// A NORMAL finished turn (`stopReason == "stop"`, no `errorMessage`) is left
    /// byte-for-byte unchanged by the in-band scrub — the redaction only touches
    /// error-stopped messages and never disturbs a healthy turn's payload.
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

    /// VAL-CAPERM-016 (abort承重, end-to-end, REALISTIC id mismatch) — `abort_run`
    /// not only flips the cancel token but ALSO fail-closes any approval the turn
    /// is parked on. The permission hook awaits on a BARE `rx.await` that does not
    /// race the cancel token, so without the second leg the awaiting tool call
    /// would hang and a late "allow" could still run the tool.
    ///
    /// This test reproduces the PRODUCTION wiring the old version masked: the
    /// permission hook is driven with the coding-agent's INTERNAL in-memory
    /// `ExtensionContext.session_id` (an `s_…`-style id minted by
    /// `SessionManager::in_memory()` because HandBox builds sessions with
    /// `no_session: true`), which is DELIBERATELY DIFFERENT from the HandBox
    /// session UUID `abort_run` is called with. The extension is constructed with
    /// the HandBox UUID, so it keys its pending registry off THAT — meaning the
    /// abort can match and drop the entry even though the cx id differs. The OLD
    /// test faked `cx.session_id == abort id`, hiding the bug where production
    /// keyed the registry off the (mismatched) cx id and the abort never matched
    /// → permanent hang.
    #[tokio::test]
    async fn abort_run_unblocks_a_pending_approval_to_cancel() {
        use crate::services::agent_permission::{PermissionExtension, APPROVAL_REQUEST_EVENT};
        use hand_coding_agent::core::extensions::api::ToolCallEvent;
        use hand_coding_agent::{Extension, ExtensionContext, HookDecision};
        use std::path::Path;

        // The HandBox DB session UUID: what the IPC layer passes to `abort_run`
        // and what `build_agent_session` threads into the PermissionExtension.
        let handbox_session_id = fresh_session_id();
        // The coding-agent's INTERNAL in-memory session id: what the host actually
        // puts in `cx.session_id` for this turn. UNRELATED to the HandBox UUID —
        // exactly the mismatch the production hang stemmed from.
        let coding_agent_internal_id = format!("s_{}_internal", uuid::Uuid::new_v4());
        assert_ne!(
            handbox_session_id, coding_agent_internal_id,
            "the cx id and the HandBox id must differ — this is the production reality"
        );

        // Register the run's controls (as `drive_agent_run` would) under the
        // HandBox UUID, so `abort_run(handbox_session_id)` finds the session and
        // flips its token. The pending-approval fail-close is keyed off the SAME
        // HandBox UUID.
        let (_cancel, _steering) = register_test_run(&handbox_session_id);

        // A recording emitter so we can wait for the approval request to land
        // before aborting (otherwise we'd race the await registration).
        let recorded: Arc<StdMutex<Vec<Value>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);
        let emitter: Arc<dyn Fn(Value) + Send + Sync> =
            Arc::new(move |payload| sink.lock().unwrap().push(payload));
        assert_eq!(APPROVAL_REQUEST_EVENT, "agent_approval_request");

        // The extension is keyed off the HandBox UUID (as build_agent_session
        // wires it), NOT the cx id the hook is driven with.
        let ext = Arc::new(PermissionExtension::new(
            handbox_session_id.clone(),
            Some(emitter),
        ));
        let hook_ext = Arc::clone(&ext);
        let hook_cx_id = coding_agent_internal_id.clone();
        let task = tokio::spawn(async move {
            // The host passes the coding-agent INTERNAL id here — different from
            // the HandBox UUID. The pending registry must key off the ext's
            // HandBox id, or this await can never be unblocked by the abort.
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
        // The emitted sessionId is the HandBox UUID (what the frontend routes by),
        // not the coding-agent internal id — pin that the payload carries the
        // right key.
        assert_eq!(
            recorded.lock().unwrap()[0].get("sessionId").unwrap(),
            &Value::String(handbox_session_id.clone()),
            "the approval request must carry the HandBox session id, not the cx id"
        );

        // Abort with the HandBox UUID: flips the token AND fail-closes the pending
        // approval keyed off that same UUID (the cx id is irrelevant to the match).
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
