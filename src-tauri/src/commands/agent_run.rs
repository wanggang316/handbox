// Agent-mode run commands.
//
// `agent_run_stream` runs one turn of a coding-agent `AgentSession` in a
// background task and maps its events onto three Tauri channels:
// `agent_stream_event` (`{ sessionId, event }`), `agent_stream_closed`
// (`{ sessionId }`, exactly once per run), and `agent_stream_error`
// (`{ sessionId, error }`, emitted before closed).
//
// `agent_run_abort` / `agent_run_steer` act through `coding_agent_runtime`'s
// process-level run-handle registry (registered by `drive_agent_run` when a turn
// starts, removed on closed) to flip the cancel token / push steering messages.
//
// Sessions persist via JSONL (`resume_session = <session_id>`): transcripts live
// at `<app_data_dir>/sessions/<flattened-cwd>/<session_id>.jsonl`, appended each
// turn. Resume context is restored from that JSONL's `build_context()`; only when
// the JSONL has no messages yet does assembly seed from the SQLite transcript
// (covers legacy sessions that predate JSONL persistence).

use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Emitter, Manager, State, Window};

use crate::models::AppError;
use crate::services::agent_hook_rules::{NotifyEmitter, HOOK_RULE_NOTIFY_EVENT};
use crate::services::agent_permission::{
    respond_to_approval, ApprovalDecision, ApprovalEmitter, APPROVAL_REQUEST_EVENT,
};
use crate::services::coding_agent_session::HookEmitters;
use crate::services::coding_agent_session::{build_agent_session, config_from_rows};
use crate::services::extensions::ask_question::{
    self, QuestionEmitter, QuestionResponse, QUESTION_REQUEST_EVENT,
};
use crate::services::extensions::{render_app, render_card, web_search};
use crate::services::skills::Skill;
use crate::services::{
    abort_run, drive_agent_run, images_from_attachments, steer_run, AgentRunRequest, AgentService,
    AgentSessionService, CodingRunSink, GenUiService, HookRuleService, McpService, ProviderService,
    SettingsService, SkillService,
};
use crate::storage::types::UUID;
use hand_ai_model::Message;

/// Per-event channel; payload is `{ sessionId, event }`.
const EVENT_NAME: &str = "agent_stream_event";
/// Turn-termination signal; payload is `{ sessionId }` (exactly once per run).
const CLOSED_NAME: &str = "agent_stream_closed";
/// Run-level error envelope; payload is
/// `{ sessionId, error: { code, message, hint } }`, emitted **before** closed.
const ERROR_NAME: &str = "agent_stream_error";
/// Session lifecycle signals (compaction / session-info); payload is
/// `{ sessionId, kind, .. }`. Independent of the three run channels: these are
/// not run events and never enter the `agent_stream_event` reducer, so the
/// closed-once invariant is unaffected.
const LIFECYCLE_NAME: &str = "agent_session_lifecycle";

/// Process-level one-run-per-session registry.
///
/// The coding-agent `AgentSession` is owned by the background task and holds
/// `&mut self` for the whole `send_message`, so there is nowhere to hang an
/// instance-level registry. A process-level `HashSet<session_id>` dedupes
/// concurrency: while a session has an active run, a second `agent_run_stream`
/// is rejected with `AGENT_RUN_ALREADY_ACTIVE`. Entries are removed when the run
/// terminates — at the same point closed is emitted.
fn active_coding_runs() -> &'static Mutex<HashSet<UUID>> {
    static RUNS: OnceLock<Mutex<HashSet<UUID>>> = OnceLock::new();
    RUNS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Marker wrapping each forced-skill body. Naming the skill keeps the marker
/// structurally distinct from the `<available_skills>` index (which uses
/// `<skill>`/`<name>`/`<description>` elements), so even a body embedding
/// `</available_skills>` cannot break the index boundary. `name` is not escaped:
/// skill bodies are trusted local content (the same `SKILL.md` text is already
/// fully reachable by the model via the coding-agent's skill tool) and skills
/// only come from local roots (app-data / `~/.agents/skills` /
/// `<workingDir>/.handbox/skills`), so this is not an injection vector.
fn open_forced_marker(name: &str) -> String {
    format!("<forced_skill name=\"{name}\">")
}
const FORCED_MARKER_CLOSE: &str = "</forced_skill>";

/// Appends resolved forced-skill bodies to `system_prompt` in place.
///
/// Each name in `forced` resolves against this turn's EFFECTIVE skill set
/// (discovered-and-validated minus the global `skills.disabled`):
/// - unknown / undiscovered / invalid / disabled / empty names are silently
///   skipped (disabled wins over forced — a disabled skill is not in `effective`);
/// - duplicate names inject the body once (first occurrence in the forced list);
/// - an opt-in (`disable_model_invocation`) skill that survives into `effective`
///   is still injected (explicit user intent overrides the auto-invocation opt-out).
///
/// Surviving bodies append in forced-list order, each wrapped in
/// `<forced_skill name="...">` … `</forced_skill>`, copied verbatim (no escaping,
/// truncation, or size limit). A non-empty `system_prompt` is separated from the
/// block by a blank line; if nothing resolves, `system_prompt` is left untouched.
fn append_forced_skill_bodies(system_prompt: &mut String, forced: &[String], effective: &[Skill]) {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut block = String::new();
    for name in forced {
        if name.is_empty() || !seen.insert(name.as_str()) {
            // Empty names never match; duplicates inject once.
            continue;
        }
        let Some(skill) = effective.iter().find(|s| &s.name == name) else {
            // Unknown / undiscovered / invalid / disabled: silently skipped.
            continue;
        };
        if !block.is_empty() {
            block.push('\n');
        }
        block.push_str(&open_forced_marker(&skill.name));
        block.push('\n');
        // Verbatim body: no escaping, no truncation.
        block.push_str(&skill.body);
        block.push('\n');
        block.push_str(FORCED_MARKER_CLOSE);
    }

    if block.is_empty() {
        return;
    }
    if system_prompt.is_empty() {
        *system_prompt = block;
    } else {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&block);
    }
}

/// Frozen generative-UI catalog prompt (generated by `npm run gen:gui-prompt`,
/// byte-locked by a vitest drift test). Teaches the model to emit a complete
/// JSON-Render spec.
const GENERATIVE_UI_PROMPT: &str = include_str!("../../resources/generative-ui-prompt.txt");

/// Appends the generative-UI instructions to `system_prompt` in place.
///
/// The catalog prompt is appended unconditionally (the caller has already checked
/// that the source agent enables generative_ui); a non-blank `example` (the linked
/// GenUI template's spec text) adds a few-shot output sample. A non-empty
/// `system_prompt` is separated by a blank line; a None / blank example injects
/// the catalog prompt only.
fn append_generative_ui_prompt(system_prompt: &mut String, example: Option<&str>) {
    if !system_prompt.is_empty() {
        system_prompt.push_str("\n\n");
    }
    system_prompt.push_str(GENERATIVE_UI_PROMPT.trim_end());

    if let Some(example) = example {
        let example = example.trim();
        if !example.is_empty() {
            system_prompt.push_str(
                "\n\nWhen you reply with a spec, imitate the structure of this example \
                 template (adapt its content to the actual answer):\n```json\n",
            );
            system_prompt.push_str(example);
            system_prompt.push_str("\n```");
        }
    }
}

/// Starts one streaming agent run, driven by a coding-agent `AgentSession`.
///
/// Claims the session in the one-run-per-session registry (rejecting with
/// `AGENT_RUN_ALREADY_ACTIVE` when a run is already active), assembles the
/// session, and delegates to `drive_agent_run`, which spawns a background task
/// that drives one turn, maps events onto the three channels, and emits closed
/// exactly once. `drive_agent_run` is non-blocking, so this command returns
/// `Ok(())` immediately; real output arrives via events. A watcher task removes
/// the registry entry when the drive task ends (in step with closed).
///
/// Any assembly error removes the registry placeholder before propagating, so
/// the session is never left permanently wedged.
#[tauri::command]
// Tauri command: one State param per injected service; splitting them into a
// struct would not reduce real complexity.
#[allow(clippy::too_many_arguments)]
pub async fn agent_run_stream(
    request: AgentRunRequest,
    window: Window,
    sessions: State<'_, AgentSessionService>,
    providers: State<'_, ProviderService>,
    skills: State<'_, Arc<SkillService>>,
    settings: State<'_, SettingsService>,
    mcp: State<'_, McpService>,
    agents: State<'_, AgentService>,
    genui: State<'_, GenUiService>,
    hook_rules: State<'_, HookRuleService>,
) -> Result<(), AppError> {
    let session_id = request.session_id.clone();

    {
        let mut runs = active_coding_runs().lock().unwrap();
        if runs.contains(&session_id) {
            return Err(AppError::with_hint(
                "AGENT_RUN_ALREADY_ACTIVE",
                &format!("a run is already active for session: {}", session_id),
                "请等待当前回合结束后再发送",
            ));
        }
        runs.insert(session_id.clone());
    }

    // From here on, every early return must remove the placeholder first.
    match assemble_and_drive(
        request,
        &window,
        &sessions,
        &providers,
        &skills,
        &settings,
        &mcp,
        &agents,
        &genui,
        &hook_rules,
    )
    .await
    {
        Ok(handles) => {
            // Remove the session from the registry once the drive task ends
            // (closed already emitted), so the next turn can start.
            let cleanup_session = session_id;
            tokio::spawn(async move {
                let _ = handles.task.await;
                active_coding_runs()
                    .lock()
                    .unwrap()
                    .remove(&cleanup_session);
            });
            Ok(())
        }
        Err(e) => {
            active_coding_runs().lock().unwrap().remove(&session_id);
            Err(e)
        }
    }
}

/// Assembles the coding-agent session and drives one turn. Registry claim and
/// cleanup are the caller's job — splitting this out keeps `agent_run_stream`'s
/// failure rollback simple: any assembly error removes the placeholder in one place.
#[allow(clippy::too_many_arguments)]
async fn assemble_and_drive(
    request: AgentRunRequest,
    window: &Window,
    sessions: &AgentSessionService,
    providers: &ProviderService,
    skills: &SkillService,
    settings: &SettingsService,
    mcp: &McpService,
    agents: &AgentService,
    genui: &GenUiService,
    hook_rules: &HookRuleService,
) -> Result<crate::services::RunDriveHandles, AppError> {
    let session_id = request.session_id.clone();

    let session_row = sessions.get_session(session_id.clone()).await?;
    let provider_id = session_row
        .provider_id
        .clone()
        .ok_or_else(|| AppError::validation_error("agent session has no provider_id selected"))?;
    let provider = providers.get_provider(&provider_id).await?;

    // app_data_dir is the session's base_dir (sandbox persistence root) and the
    // cwd fallback when working_dir is unset.
    let app_data_dir =
        window.app_handle().path().app_data_dir().map_err(|e| {
            AppError::internal_error(&format!("failed to resolve app data dir: {e}"))
        })?;

    let mut config = config_from_rows(&session_row, &provider, app_data_dir)?;

    // The session's source agent (linked via agent_definition_id) is resolved
    // live — agent_sessions has no snapshot column — so agent-config edits take
    // effect on the next turn of existing sessions. A dangling definition or
    // query failure silently degrades to None and never blocks the run. Shared
    // by the generative-UI and pinned-skill injection below.
    let definition = match session_row.agent_definition_id.clone() {
        Some(def_id) => agents.get_agent(def_id).await.ok(),
        None => None,
    };

    // Generative-UI injection: when the source agent enables generative_ui,
    // append the frozen catalog prompt to this turn's system prompt; its linked
    // GenUI template (agents.genui_id → genui.spec) adds a few-shot output
    // example. A deleted template silently degrades to catalog-only.
    if let Some(definition) = definition
        .as_ref()
        .filter(|d| d.generative_ui == Some(true))
    {
        let example = match definition.genui_id.clone() {
            Some(genui_id) => genui.get_genui(genui_id).await.ok().map(|g| g.spec),
            None => None,
        };
        let mut system_prompt = config.system_prompt.take().unwrap_or_default();
        append_generative_ui_prompt(&mut system_prompt, example.as_deref());
        config.system_prompt = Some(system_prompt);
    }

    // Skill injection: definition-pinned skills (`agents.skills`, applied every
    // turn for all of this agent's sessions) merge ahead of
    // `request.forced_skills` (wire `forcedSkills`, explicitly forced by the
    // user this turn). The coding-agent owns ambient skill discovery but has NO
    // forced-skill API, so HandBox resolves the combined names against its own
    // effective set (discovered across app-data / user / project scopes minus
    // the global `skills.disabled` opt-out) and splices the surviving bodies
    // into the system prompt before construction; duplicates (incl.
    // pinned∩forced overlap) inject once. See `append_forced_skill_bodies` for
    // the resolution rules.
    let pinned_skills: &[String] = definition.as_ref().map_or(&[], |d| d.skills.as_slice());
    if !pinned_skills.is_empty() || !request.forced_skills.is_empty() {
        let working_dir = session_row.working_dir.as_deref().map(std::path::Path::new);
        let (discovered, skill_errs) = skills.discover(working_dir);
        if !skill_errs.is_empty() {
            tracing::warn!(
                "[agent_run_stream] skill discovery produced {} non-fatal diagnostic(s)",
                skill_errs.len()
            );
        }
        let disabled = settings.get_settings()?.skills.disabled;
        let effective: Vec<Skill> = discovered
            .into_iter()
            .filter(|s| !disabled.contains(&s.name))
            .collect();

        let combined: Vec<String> = pinned_skills
            .iter()
            .chain(request.forced_skills.iter())
            .cloned()
            .collect();
        let mut system_prompt = config.system_prompt.take().unwrap_or_default();
        append_forced_skill_bodies(&mut system_prompt, &combined, &effective);
        // Keep `None` (default prompt) when nothing was injected AND there was no
        // base prompt, so a session without a custom prompt still falls back to
        // the coding-agent default rather than an empty override.
        config.system_prompt = (!system_prompt.is_empty()).then_some(system_prompt);
    }

    // Approval emitter for the PermissionExtension: a dangerous tool call
    // (write/edit/bash) pushes an `agent_approval_request`
    // `{ sessionId, callId, toolName, args, requestId }` to the frontend and
    // awaits the user's decision (answered via the `agent_approval_respond` IPC).
    // Wrap `window.emit` so the extension stays decoupled from Tauri.
    let approval_window = window.clone();
    let approval_emitter: ApprovalEmitter = Arc::new(move |payload| {
        if let Err(e) = approval_window.emit(APPROVAL_REQUEST_EVENT, payload) {
            tracing::warn!(
                "[agent_run_stream] failed to emit {}: {}",
                APPROVAL_REQUEST_EVENT,
                e
            );
        }
    });

    // Same shape for `notify` hook rules, which report a matching tool result
    // rather than gating anything.
    let notify_window = window.clone();
    let notify_emitter: NotifyEmitter = Arc::new(move |payload| {
        if let Err(e) = notify_window.emit(HOOK_RULE_NOTIFY_EVENT, payload) {
            tracing::warn!(
                "[agent_run_stream] failed to emit {}: {}",
                HOOK_RULE_NOTIFY_EVENT,
                e
            );
        }
    });

    // The user's enabled rules, snapshotted for this turn. A failure to read them
    // degrades to "no rules" rather than blocking the run: the sandbox and the
    // approval gate are the guardrails that must never be skipped, and both are
    // independent of this.
    config.hook_rules = match hook_rules.list_enabled().await {
        Ok(rules) => rules,
        Err(e) => {
            tracing::warn!("[agent_run_stream] failed to load hook rules: {}", e);
            Vec::new()
        }
    };

    // Resolve this session's MCP server bindings into AgentTools and inject them
    // into the loop. Per-binding `enabled_tools` overrides the server's global
    // selection; failures degrade to no MCP tools rather than aborting the run.
    let mcp_tools = if session_row.mcp_servers.is_empty() {
        Vec::new()
    } else {
        let ids: Vec<String> = session_row
            .mcp_servers
            .iter()
            .map(|c| c.server_id.clone())
            .collect();
        let mut servers = mcp.get_servers_by_ids(&ids).await.unwrap_or_default();
        for server in &mut servers {
            if let Some(cfg) = session_row
                .mcp_servers
                .iter()
                .find(|c| c.server_id == server.id)
            {
                server.enabled_tools = cfg.enabled_tools.clone();
            }
        }
        // Manual-execution servers' tools require approval: collect their
        // namespaced names into the config so PermissionExtension gates them.
        let mut manual = std::collections::HashSet::new();
        for server in &servers {
            let is_manual = session_row
                .mcp_servers
                .iter()
                .find(|c| c.server_id == server.id)
                .is_some_and(|c| c.execution_mode == "manual");
            if is_manual {
                for tool in &server.enabled_tools {
                    manual.insert(format!("mcp__{}__{}", server.id, tool));
                }
            }
        }
        config.mcp_approval_tools = manual;
        mcp.build_mcp_agent_tools(&servers)
    };

    // Web-search tool: session opt-in via `enabled_tools` + a configured API
    // key. Injected through `extra_tools` (like the MCP tools) because it is
    // not a coding-agent built-in `select_enabled_tools` knows about. No key →
    // the tool is simply not registered, so the model never sees it and the
    // run proceeds without it.
    let mut extra_tools = mcp_tools;
    if config
        .enabled_tools
        .iter()
        .any(|t| t == web_search::WEB_SEARCH_TOOL_NAME)
    {
        let web_search_settings = settings.get_settings()?.agent.web_search;
        if web_search_settings.api_key.trim().is_empty() {
            tracing::debug!(
                "[agent_run_stream] web_search enabled but no API key configured; tool skipped"
            );
        } else {
            extra_tools.push(web_search::create_web_search_tool(
                web_search_settings.provider,
                web_search_settings.api_key,
            ));
        }
    }

    // Presentational tools ride the same extra_tools channel as MCP tools and
    // are gated like web_search: only sessions whose `enabled_tools` name them
    // get the registration (settings default + per-agent capability set +
    // session edits control that list). The Rust handlers only validate and
    // acknowledge; the frontend renders the card / app panel from the toolcall
    // blocks' arguments, so no extra IPC exists.
    if config
        .enabled_tools
        .iter()
        .any(|t| t == render_card::TOOL_RENDER_CARD)
    {
        extra_tools.push(render_card::make_render_card_tool());
    }
    if config
        .enabled_tools
        .iter()
        .any(|t| t == render_app::TOOL_RENDER_APP)
    {
        extra_tools.push(render_app::make_render_app_tool());
    }

    // ask_question is the one INTERACTIVE extension tool: its handler emits
    // `agent_question_request` and parks until the user answers through the
    // `agent_question_respond` IPC, so it needs both the HandBox session id
    // (the key `abort_run` cancels parked questions by) and an emitter. Same
    // opt-in gate as the others.
    if config
        .enabled_tools
        .iter()
        .any(|t| t == ask_question::TOOL_ASK_QUESTION)
    {
        let question_window = window.clone();
        let question_emitter: QuestionEmitter = Arc::new(move |payload| {
            if let Err(e) = question_window.emit(QUESTION_REQUEST_EVENT, payload) {
                tracing::warn!(
                    "[agent_run_stream] failed to emit {}: {}",
                    QUESTION_REQUEST_EVENT,
                    e
                );
            }
        });
        extra_tools.push(ask_question::make_ask_question_tool(
            session_id.clone(),
            Some(question_emitter),
        ));
    }

    let mut session = build_agent_session(
        &config,
        HookEmitters {
            approval: Some(approval_emitter),
            notify: Some(notify_emitter),
        },
        extra_tools,
    )?;

    // Resume context: the JSONL is the source of truth. `build_agent_session`
    // constructs with `resume_session = <session_id>`, so the coding-agent has
    // already restored history into the in-memory context via that JSONL's
    // `build_context()`, and this turn's new messages append back to the same
    // file. For sessions with JSONL history, seeding again would clobber the
    // restored context with SQLite.
    //
    // Only when the JSONL has no messages yet do we seed the in-memory context
    // from the SQLite transcript — covers legacy sessions that predate JSONL
    // persistence (SQLite transcript only) on their first resume. This fills
    // memory only: `set_messages` touches neither the JSONL nor SQLite.
    let jsonl_message_count = session.messages().len();
    if jsonl_message_count == 0 {
        let history = sessions.list_messages(session_id.clone()).await?;
        if !history.is_empty() {
            let mut seeded: Vec<Message> = Vec::with_capacity(history.len());
            for row in history {
                let msg: Message = serde_json::from_value(row.payload).map_err(|e| {
                    AppError::internal_error(&format!(
                        "agent transcript payload (seq {}) is not a valid hand-agent Message: {}",
                        row.seq, e
                    ))
                })?;
                seeded.push(msg);
            }
            session.set_messages(seeded);
        }
    }

    let event_window = window.clone();
    let error_window = window.clone();
    let closed_window = window.clone();
    let lifecycle_window = window.clone();

    let sink = CodingRunSink::new(
        Arc::new(move |payload| {
            if let Err(e) = event_window.emit(EVENT_NAME, payload) {
                tracing::warn!("[agent_run_stream] failed to emit {}: {}", EVENT_NAME, e);
            }
        }),
        Arc::new(move |payload| {
            if let Err(e) = closed_window.emit(CLOSED_NAME, payload) {
                tracing::warn!("[agent_run_stream] failed to emit {}: {}", CLOSED_NAME, e);
            }
        }),
    )
    // Run-level `Err` envelope goes out as a distinct window event, before closed.
    .with_error(Arc::new(move |payload| {
        if let Err(e) = error_window.emit(ERROR_NAME, payload) {
            tracing::warn!("[agent_run_stream] failed to emit {}: {}", ERROR_NAME, e);
        }
    }))
    // Lifecycle signals (compaction / session-info) use their own channel,
    // separate from the three run channels; they never enter the run reducer.
    .with_lifecycle(Arc::new(move |payload| {
        if let Err(e) = lifecycle_window.emit(LIFECYCLE_NAME, payload) {
            tracing::warn!(
                "[agent_run_stream] failed to emit {}: {}",
                LIFECYCLE_NAME,
                e
            );
        }
    }));

    // Validate image attachments at the IPC boundary (oversized / excess /
    // non-image are silently dropped) and convert survivors into ImageContent
    // blocks; an empty set takes the plain-text path.
    let images = images_from_attachments(&request.attachments);

    Ok(drive_agent_run(
        session,
        session_id,
        request.input,
        images,
        sink,
    ))
}

/// Aborts an agent session's active run, if any.
///
/// Flips the cancel token held in `coding_agent_runtime`'s process-level
/// registry — the **same** token passed to the coding-agent's `send_message` —
/// so the agent loop unwinds at the next await boundary and synthesizes a
/// `stopReason=aborted` final turn; the drive task then emits
/// `agent_stream_closed` from its single emit site (closed-once holds on the
/// abort path too).
///
/// A clean no-op (`Ok(())`, no error) for unknown / already-finished sessions —
/// the frontend may race this command against a natural run end.
#[tauri::command]
pub async fn agent_run_abort(session_id: UUID) -> Result<(), AppError> {
    abort_run(&session_id);
    Ok(())
}

/// Merges a steering message into an agent session's **in-flight** run.
///
/// Pushes `text` as a user `Message` onto the run's steering queue (via
/// `coding_agent_runtime`'s process-level registry); the agent loop drains it at
/// the next turn boundary, so the message joins the **current** turn — no
/// concurrent run, no follow-up queue that auto-continues after this turn.
///
/// Empty / whitespace-only `text` is a no-op; a session with no active run is
/// also a clean no-op (`Ok(())`, no error).
#[tauri::command]
pub async fn agent_run_steer(session_id: UUID, text: String) -> Result<(), AppError> {
    steer_run(&session_id, text);
    Ok(())
}

/// Feeds a tool-approval decision (with scope) back to the awaiting
/// `PermissionExtension` hook.
///
/// A dangerous tool call (write/edit/bash) makes `PermissionExtension` emit
/// `agent_approval_request` and await a oneshot keyed by `request_id`; the
/// frontend dialog answers through this command. `decision`:
///  - `deny` → the tool is `Cancel`led (the model sees a rejected result).
///  - `allow_once` → allowed this time (`Continue`), not remembered; the same
///    tool prompts again next call.
///  - `allow_always` → allowed and remembered for **this session** (an
///    in-process always-allow set keyed by session_id); later calls of that tool
///    in the same session skip the dialog. Memory only — never persisted, so it
///    survives neither other sessions nor restarts.
///
/// Idempotent: the first response wins; duplicate / unknown `request_id`s are a
/// clean no-op (no entry left in the registry, nothing happens, no error) — the
/// frontend may answer twice in a race, or answer a request that vanished with
/// an aborted run.
#[tauri::command]
pub async fn agent_approval_respond(
    request_id: String,
    decision: ApprovalDecision,
) -> Result<(), AppError> {
    respond_to_approval(&request_id, decision);
    Ok(())
}

/// Feeds the user's answers back to the awaiting `ask_question` tool call.
///
/// The tool emits `agent_question_request` and parks on a oneshot keyed by
/// `request_id`; the question panel answers through this command. `response`:
///  - `{ kind: "answered", answers: [{ questionId, values }] }` → the answers
///    become the tool result the model reads. Questions with no values are
///    reported to the model as explicitly unanswered.
///  - `{ kind: "dismissed" }` → the user chose to keep talking instead; the
///    model is told to continue without the answers rather than re-ask.
///
/// Idempotent: the first response wins; duplicate / unknown `request_id`s are a
/// clean no-op — the panel may answer twice in a race, or answer a request that
/// vanished with an aborted run.
#[tauri::command]
pub async fn agent_question_respond(
    request_id: String,
    response: QuestionResponse,
) -> Result<(), AppError> {
    ask_question::respond_to_question(&request_id, response);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one-run-per-session gate, exercised directly against the
    /// process-level `active_coding_runs` registry — the same check-and-insert
    /// `agent_run_stream` performs before assembling a session. Going through
    /// the registry instead of the full async command (which needs Tauri
    /// `State`/`Window`) keeps the test hermetic: no DB, no window, no network.
    /// A fresh uuid isolates each test from the process-global registry.
    fn try_claim(session_id: &UUID) -> Result<(), AppError> {
        let mut runs = active_coding_runs().lock().unwrap();
        if runs.contains(session_id) {
            return Err(AppError::with_hint(
                "AGENT_RUN_ALREADY_ACTIVE",
                &format!("a run is already active for session: {session_id}"),
                "请等待当前回合结束后再发送",
            ));
        }
        runs.insert(session_id.clone());
        Ok(())
    }

    fn release(session_id: &UUID) {
        active_coding_runs().lock().unwrap().remove(session_id);
    }

    #[test]
    fn second_concurrent_run_is_rejected_then_reclaimable_after_close() {
        let session_id = uuid::Uuid::new_v4().to_string();

        // (1) first run claims the session.
        try_claim(&session_id).expect("first run claims the session");

        // (2) a second start on the same session is rejected — no concurrent run.
        let err = try_claim(&session_id).expect_err("second concurrent run must be rejected");
        assert_eq!(err.code, "AGENT_RUN_ALREADY_ACTIVE");

        // (3) once the run's closed-emit releases the entry, the session is
        // claimable again — a later turn is not permanently wedged.
        release(&session_id);
        try_claim(&session_id).expect("session is reclaimable after the run closes");

        // Cleanup so the process-global registry is left empty for other tests.
        release(&session_id);
    }

    use crate::services::skills::{SourceInfo, SourceScope};
    use std::path::PathBuf;

    /// Build a discovered [`Skill`] fixture (name + body) for the forced-skill
    /// injection tests. Scope/path are immaterial to `append_forced_skill_bodies`,
    /// which only reads `name`/`body`.
    fn skill(name: &str, body: &str) -> Skill {
        Skill {
            name: name.to_string(),
            description: format!("desc for {name}"),
            body: body.to_string(),
            disable_model_invocation: false,
            source: SourceInfo {
                scope: SourceScope::AppData,
                path: PathBuf::from(format!("/skills/{name}/SKILL.md")),
            },
        }
    }

    fn forced(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // A forced skill whose name resolves in the effective set has its body
    // appended VERBATIM, bracketed by a `<forced_skill>` marker naming the
    // skill, after the base prompt (separated by a blank line).
    #[test]
    fn forced_skill_body_is_appended_verbatim_with_marker() {
        let effective = vec![skill("deploy", "Run `make deploy`.\nThen verify.")];
        let mut prompt = "Base system prompt.".to_string();

        append_forced_skill_bodies(&mut prompt, &forced(&["deploy"]), &effective);

        assert_eq!(
            prompt,
            "Base system prompt.\n\n<forced_skill name=\"deploy\">\n\
             Run `make deploy`.\nThen verify.\n</forced_skill>",
        );
    }

    // With an empty base prompt, the forced block stands alone (no leading
    // separator).
    #[test]
    fn forced_skill_into_empty_prompt_has_no_leading_separator() {
        let effective = vec![skill("alpha", "alpha body")];
        let mut prompt = String::new();

        append_forced_skill_bodies(&mut prompt, &forced(&["alpha"]), &effective);

        assert_eq!(
            prompt,
            "<forced_skill name=\"alpha\">\nalpha body\n</forced_skill>",
        );
    }

    // An empty forced list (the default for a legacy payload) leaves the prompt
    // completely untouched — injection is purely additive.
    #[test]
    fn empty_forced_list_leaves_prompt_untouched() {
        let effective = vec![skill("alpha", "alpha body")];
        let mut prompt = "Base.".to_string();

        append_forced_skill_bodies(&mut prompt, &[], &effective);
        assert_eq!(prompt, "Base.", "no forced names → prompt unchanged");
    }

    // A forced name absent from the effective set is silently skipped — this is
    // exactly how the global `skills.disabled` opt-out "wins
    // over forced" (the caller filters disabled skills out of `effective`
    // before calling), and how unknown / undiscovered names are handled. The
    // prompt is left untouched when NOTHING resolves.
    #[test]
    fn unresolved_forced_names_are_silently_skipped() {
        let effective = vec![skill("present", "present body")];
        let mut prompt = "Base.".to_string();

        // `disabled` (filtered out by the caller) + `ghost` (never discovered).
        append_forced_skill_bodies(&mut prompt, &forced(&["disabled", "ghost"]), &effective);
        assert_eq!(
            prompt, "Base.",
            "no resolvable forced skill → prompt unchanged"
        );

        // Mixed: only the resolvable one is injected.
        append_forced_skill_bodies(&mut prompt, &forced(&["ghost", "present"]), &effective);
        assert_eq!(
            prompt,
            "Base.\n\n<forced_skill name=\"present\">\npresent body\n</forced_skill>",
        );
    }

    // Multiple forced skills inject in forced-list order, and a repeated name
    // injects its body only ONCE (first occurrence).
    #[test]
    fn forced_skills_inject_in_order_and_dedup_by_name() {
        let effective = vec![skill("alpha", "A"), skill("beta", "B")];
        let mut prompt = String::new();

        append_forced_skill_bodies(&mut prompt, &forced(&["beta", "alpha", "beta"]), &effective);

        assert_eq!(
            prompt,
            "<forced_skill name=\"beta\">\nB\n</forced_skill>\n\
             <forced_skill name=\"alpha\">\nA\n</forced_skill>",
            "forced-list order, beta deduped to a single block"
        );
    }

    // An empty forced name never matches and is skipped (it would otherwise
    // spuriously short-circuit dedup).
    #[test]
    fn empty_forced_name_is_skipped() {
        let effective = vec![skill("alpha", "A")];
        let mut prompt = String::new();

        append_forced_skill_bodies(&mut prompt, &forced(&["", "alpha"]), &effective);
        assert_eq!(
            prompt, "<forced_skill name=\"alpha\">\nA\n</forced_skill>",
            "the empty name is skipped; alpha still injects",
        );
    }

    // Generative-UI injection: catalog prompt appends after the base prompt
    // with a blank-line separator; no example section without an example.
    #[test]
    fn generative_ui_appends_catalog_after_base_prompt() {
        let mut prompt = String::from("base");
        append_generative_ui_prompt(&mut prompt, None);
        assert!(prompt.starts_with("base\n\n"), "blank-line separated");
        assert!(
            prompt.contains(GENERATIVE_UI_PROMPT.trim_end()),
            "catalog prompt embedded verbatim"
        );
        assert!(
            !prompt.contains("example template"),
            "no example section without an example"
        );
    }

    // A linked template spec appends as a fenced few-shot example after the
    // catalog prompt; blank example degrades to catalog-only.
    #[test]
    fn generative_ui_appends_example_when_present() {
        let mut prompt = String::new();
        append_generative_ui_prompt(&mut prompt, Some("{\"root\":\"card\"}"));
        assert!(
            prompt.ends_with("```json\n{\"root\":\"card\"}\n```"),
            "example fenced at the end, got tail: {:?}",
            &prompt[prompt.len().saturating_sub(60)..]
        );

        let mut blank = String::new();
        append_generative_ui_prompt(&mut blank, Some("   "));
        assert!(
            !blank.contains("example template"),
            "blank example injects catalog prompt only"
        );
    }
}
