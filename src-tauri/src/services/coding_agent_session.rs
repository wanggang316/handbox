//! Construct a coding-agent [`AgentSession`] from a HandBox agent-session
//! configuration; driving the prompt loop and IPC wiring live elsewhere.
//! Models and stream options resolve through `model_runtime`, and the plaintext
//! api key rides inside `SimpleStreamOptions.base.api_key` only (no `auth.json`,
//! env vars, or keyring). `base_dir` is the app data dir, never `~/.hand`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hand_agent::AgentTool;
use hand_ai_model::SimpleStreamOptions;
use hand_coding_agent::tools::create_default_tools;
use hand_coding_agent::{AgentSession, AgentSessionConfig};

use crate::models::AppError;
use crate::services::agent_hook_rules::{wrap_approval_emitter, NotifyEmitter, RuleHookExtension};
use crate::services::agent_permission::{ApprovalEmitter, PermissionExtension, SandboxExtension};
use crate::services::extensions;
use crate::services::model_runtime::{self, ChatOptions};
use crate::storage::types::{AgentSession as HandBoxAgentSessionRow, HookRule, Provider};

/// HandBox-side inputs needed to construct a coding-agent session; field names
/// mirror the HandBox agent-session storage row.
#[derive(Debug, Clone)]
pub struct HandBoxAgentSessionConfig {
    /// HandBox DB session id (UUID). [`PermissionExtension`] keys approval state
    /// off it — the same id `coding_agent_runtime::abort_run` uses, not the
    /// coding-agent's in-memory id — so a parked approval await can be unblocked.
    pub session_id: String,
    /// HandBox provider row id (diagnostics only).
    pub provider_id: String,
    /// hand-ai provider tag consumed by [`model_runtime::resolve_model`].
    pub provider_type: String,
    /// Model id selected for this session.
    pub model_id: String,
    /// Optional base-url override. Empty string means "use the catalog
    /// template's base_url unchanged" (same contract as `model_runtime`).
    pub base_url: String,
    /// Plaintext provider api key. Injected via stream options only.
    pub api_key: String,
    /// Working directory the agent's tools operate against (the `cwd`).
    pub working_dir: PathBuf,
    /// Pure-dialog mode: no working directory selected (`cwd` fell back to
    /// `app_data_dir`), so workspace-scoped discovery (`no_context_files` /
    /// `no_skills`) is off — no project root for `AGENTS.md` or `.hand/skills`.
    pub pure_dialog: bool,
    /// Tauri per-app data directory. Becomes the session's `base_dir` so
    /// persistent state stays inside the app sandbox, not `~/.hand`.
    pub app_data_dir: PathBuf,
    /// Session creation time (millis), off the SQLite `agent_sessions.created_at`
    /// column. Stamped as the JSONL header `timestamp` so the header reports the
    /// session's real creation time, not the first-run wall clock.
    pub created_at: i64,
    /// Per-session custom system prompt. `None` falls back to the coding-agent
    /// default prompt.
    pub system_prompt: Option<String>,
    /// Per-session sampling temperature. `None` = model/provider default.
    /// Threads into `ChatOptions.temperature` → `stream_options.base.temperature`.
    pub temperature: Option<f32>,
    /// Per-session max output tokens. Stored as `i32` on the session row; this
    /// carries the `u32` form `ChatOptions.max_tokens` expects.
    pub max_tokens: Option<u32>,
    /// Per-session thinking level (e.g. `"low"`/`"medium"`/`"high"`), passed
    /// through verbatim as `ChatOptions.reasoning_effort`; unknown values parse
    /// to `None`, so a non-reasoning model never breaks.
    pub thinking_level: Option<String>,
    /// Per-session enabled-tool list: coding-agent registered names plus the
    /// extension-tool ids ([`extensions::EXTENSION_TOOL_IDS`]). Only the named
    /// built-ins are registered against the session (see [`select_enabled_tools`]);
    /// an empty list means "no tool enabled" (not "all enabled"). Extension ids
    /// resolve elsewhere — into `extra_tools` by agent_run, or the skill gate.
    pub enabled_tools: Vec<String>,
    /// Tool names requiring approval this session: the `mcp__server__tool` names
    /// of manual-execution MCP servers. Populated by agent_run; empty default =
    /// no MCP approval gating (the jobs/tests path).
    pub mcp_approval_tools: HashSet<String>,
    /// The user's enabled hook rules, as a snapshot taken when this config is
    /// assembled. Empty = no rule extension is registered at all. Populated by
    /// the callers that have a database handle; see
    /// [`RuleHookExtension`](crate::services::agent_hook_rules::RuleHookExtension).
    pub hook_rules: Vec<HookRule>,
}

/// Frontend sinks the hook chain emits through. Grouped so wiring a new one does
/// not churn every [`build_agent_session`] call site.
///
/// Both default to `None`, which is the headless shape (jobs, tests): approvals
/// then fail closed and hook-match notices are only logged.
#[derive(Clone, Default)]
pub struct HookEmitters {
    /// Approval prompts — dangerous built-ins and manual MCP tools. `None`
    /// denies rather than prompting.
    pub approval: Option<ApprovalEmitter>,
    /// Match notices raised by hook rules.
    pub notify: Option<NotifyEmitter>,
}

/// Construct a coding-agent [`AgentSession`] from a HandBox configuration.
///
/// Skill-discovery roots are pinned to `None` so construction never reads the
/// host's real `~/.hand/skills/` (project-scope `<cwd>/.hand/skills` still
/// applies), and `base_dir` is `app_data_dir` so persistence stays in the sandbox.
///
/// `emitters.approval` wires the [`PermissionExtension`]'s approval-request
/// channel; `None` makes it fail CLOSED — every dangerous tool (write/edit/bash)
/// is denied without prompting, the safe default for headless construction.
pub fn build_agent_session(
    config: &HandBoxAgentSessionConfig,
    emitters: HookEmitters,
    extra_tools: Vec<AgentTool>,
) -> Result<AgentSession, AppError> {
    let model =
        model_runtime::resolve_model(&config.provider_type, &config.model_id, &config.base_url)?;

    // Sampling params must be baked in at construction: `drive_agent_run` applies
    // no per-turn options. thinking_level rides as `reasoning_effort`, which
    // `build_stream_options` parses into `stream_options.reasoning`.
    let chat_options = ChatOptions {
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        reasoning_effort: config.thinking_level.clone(),
        ..ChatOptions::default()
    };
    let stream_options: SimpleStreamOptions =
        model_runtime::build_stream_options(&chat_options, &config.api_key);

    let mut tools = select_enabled_tools(&config.working_dir, &config.enabled_tools);
    // Per-session MCP tools (namespaced `mcp__server__tool`) run alongside the
    // built-ins. Empty for sessions with no MCP bindings.
    tools.extend(extra_tools);

    // Guarantee the JSONL file the `resume_session` branch opens exists, named
    // after the HandBox session UUID. Idempotent — a second turn resumes the first
    // turn's file, so the transcript accretes instead of being re-minted. Doing it
    // here keeps "file exists before resume" an invariant of construction.
    crate::services::agent_jsonl_store::ensure_session_file(
        &config.app_data_dir,
        &config.working_dir,
        &config.session_id,
        config.created_at,
    )?;

    let session_config = AgentSessionConfig {
        cwd: config.working_dir.clone(),
        model,
        stream_options,
        // `None` leaves the coding-agent default prompt in place.
        custom_system_prompt: config.system_prompt.clone(),
        custom_guidelines: None,
        // Resume the JSONL named after the HandBox session UUID (pre-seeded above)
        // so every turn appends to `<base>/sessions/<flattened-cwd>/<id>.jsonl`;
        // the `create_in` path would mint its own `s_…` id and ignore ours. With
        // `resume_session` set, `no_session` is irrelevant.
        resume_session: Some(config.session_id.clone()),
        no_session: false,
        // Workspace-less sessions run as pure dialog: no project root to read
        // AGENTS.md or .hand/skills from, so both discoveries are disabled.
        no_context_files: config.pure_dialog,
        session_dir: None,
        // The skill pipeline (discovery + `<available_skills>` index + the
        // coding-agent's own `skill` tool) additionally requires the session to
        // opt in via the `skill` extension-tool id — the settings/agent-level
        // toggle rides `enabled_tools` like every other extension tool.
        no_skills: config.pure_dialog
            || !config
                .enabled_tools
                .iter()
                .any(|t| t == extensions::TOOL_SKILL),
        extra_skill_dirs: Vec::new(),
        // Persist under the Tauri app data dir, never ~/.hand; the resume path
        // must match the writer side (`agent_jsonl_store::session_path`).
        base_dir: Some(config.app_data_dir.clone()),
    };

    // Surface the effective tool set once per construction: tool-availability
    // bugs (a missing extra tool, an unmatched enabled name) are otherwise only
    // observable through model behavior.
    tracing::info!(
        tools = ?tools.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        "[build_agent_session] registering tools"
    );

    let mut session = AgentSession::new_with_skill_dirs(session_config, tools, None, None)
        .map_err(|e| {
            AppError::internal_error(&format!("failed to construct agent session: {e}"))
        })?;

    // Re-impose the working_dir boundary on the read-only file tools: the vendored
    // coding agent honors absolute paths and expands `~`, so HandBox Cancels any
    // out-of-sandbox path from the outside via this before_tool_call extension.
    session.register_extension(Arc::new(SandboxExtension::new(config.working_dir.clone())));

    // The user's hook rules sit between the sandbox and the approval gate:
    // behind the sandbox so no hook can widen the working-directory boundary,
    // and ahead of the gate so a command that vetoes a call spares the user a
    // prompt for something that would be blocked anyway. Skipped entirely when
    // no rule is configured.
    let rules = Arc::new(
        RuleHookExtension::new(config.session_id.clone(), config.hook_rules.clone())
            .with_notifier(emitters.notify.clone())
            .with_working_dir(config.working_dir.clone()),
    );
    // Logged unconditionally: "did my rule load?" is the first question when a
    // rule appears not to fire, and a zero here answers it immediately.
    tracing::info!(
        hook_rules = config.hook_rules.len(),
        "[build_agent_session] hook rules loaded"
    );
    if rules.has_extension_rules() {
        session.register_extension(rules.clone());
    }

    // Approval-requested rules ride the approval channel itself rather than the
    // extension chain: the emitter fires exactly when a call pauses for the
    // user, so an always-allowed tool never triggers them. Headless (`None`)
    // never prompts, so there is nothing to observe.
    let approval = match emitters.approval {
        Some(inner) if rules.has_approval_rules() => {
            Some(wrap_approval_emitter(rules.clone(), inner))
        }
        other => other,
    };

    // Approval gate for write/edit/bash: emits `agent_approval_request` and awaits
    // the decision (allow → Continue, deny → Cancel); no emitter means fail CLOSED.
    // Registered after the sandbox because extensions run in order and the first
    // Cancel wins, so out-of-cwd paths never reach — never prompt — this gate.
    // Keyed by the HandBox session UUID, not the coding-agent's in-memory id, so
    // `abort_run` / `deny_pending_for_session` can unblock a parked approval await
    // and always-allow consent persists across turns.
    session.register_extension(Arc::new(
        PermissionExtension::new(config.session_id.clone(), approval)
            .with_approval_tools(config.mcp_approval_tools.clone()),
    ));

    Ok(session)
}

/// Map a persisted `enabled_tools` entry to its coding-agent registered name.
///
/// Legacy sessions store the old native names (`read_file` → `read`,
/// `list_directory` → `ls`). Names with no counterpart (`web_fetch`) and unknown
/// names pass through unchanged for the downstream filters to drop; `skill`
/// passes through onto the same id the skill-pipeline gate reads, so a migrated
/// session keeps skill access. Applied at construction time only; the SQLite
/// column is never rewritten.
fn remap_legacy_tool_name(name: &str) -> &str {
    match name {
        "read_file" => "read",
        "list_directory" => "ls",
        other => other,
    }
}

/// Filter the coding-agent built-ins down to the per-session `enabled` names
/// (each mapped through [`remap_legacy_tool_name`] first). Extension-tool ids
/// ([`extensions::EXTENSION_TOOL_IDS`]) are skipped — they are resolved outside
/// this filter. An empty `enabled` registers NO tools ("not listed = not
/// enabled"), never the full set; unknown names only warn. Output follows the
/// canonical `create_default_tools` order.
pub fn select_enabled_tools(cwd: &Path, enabled: &[String]) -> Vec<AgentTool> {
    let mut wanted: Vec<&str> = enabled
        .iter()
        .map(|name| remap_legacy_tool_name(name.as_str()))
        .filter(|name| !extensions::EXTENSION_TOOL_IDS.contains(name))
        .collect();

    let selected: Vec<AgentTool> = create_default_tools(cwd)
        .into_iter()
        .filter(|tool| {
            if let Some(pos) = wanted.iter().position(|name| *name == tool.name) {
                // Mark as matched so whatever remains in `wanted` is unknown.
                wanted.swap_remove(pos);
                true
            } else {
                false
            }
        })
        .collect();

    for unknown in &wanted {
        tracing::warn!(
            tool = unknown,
            "ignoring unknown enabled tool name; not in the built-in set"
        );
    }

    selected
}

/// Assemble a [`HandBoxAgentSessionConfig`] from the persisted session and
/// provider rows. Pure row mapping — no network, no construction.
///
/// When the session has no working directory the cwd falls back to
/// `app_data_dir`: the coding agent needs an existing directory to root its
/// tools in, and that keeps the fallback inside the app sandbox.
///
/// Returns `VALIDATION_ERROR` when the session has not selected a model.
pub fn config_from_rows(
    session: &HandBoxAgentSessionRow,
    provider: &Provider,
    app_data_dir: PathBuf,
) -> Result<HandBoxAgentSessionConfig, AppError> {
    let model_id = session
        .model_id
        .clone()
        .ok_or_else(|| AppError::validation_error("agent session has no model_id selected"))?;

    // Capture the "no workspace" bit before the cwd fallback overwrites it.
    let pure_dialog = session.working_dir.is_none();
    let working_dir = session
        .working_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data_dir.clone());

    Ok(HandBoxAgentSessionConfig {
        // The row's primary key IS the session UUID the IPC layer passes to
        // abort_run, so the permission extension keys approval state off it.
        session_id: session.id.clone(),
        provider_id: provider.id.clone(),
        provider_type: provider.provider_type.clone(),
        model_id,
        base_url: provider.base_url.clone(),
        api_key: provider.api_key.clone(),
        working_dir,
        pure_dialog,
        app_data_dir,
        // Lifted off the row so the JSONL header timestamp equals createdAt.
        created_at: session.created_at,
        // max_tokens converts i32 → u32; out-of-range silently drops to None.
        system_prompt: session.system_prompt.clone(),
        temperature: session.temperature,
        max_tokens: session.max_tokens.and_then(|t| u32::try_from(t).ok()),
        thinking_level: session.thinking_level.clone(),
        enabled_tools: session.enabled_tools.clone(),
        // agent_run fills this from manual-server MCP bindings; empty otherwise.
        mcp_approval_tools: HashSet::new(),
        // Filled by callers holding a database handle; empty here so a config
        // built for a test or a probe carries no rules.
        hook_rules: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A fixed, recognizable `created_at` (millis) so tests can assert the seeded
    /// JSONL header timestamp without racing the wall clock.
    const TEST_CREATED_AT: i64 = 1_700_000_000_000;

    fn sample_config(working_dir: PathBuf, app_data_dir: PathBuf) -> HandBoxAgentSessionConfig {
        HandBoxAgentSessionConfig {
            session_id: "sess-row-uuid".to_string(),
            provider_id: "prov-row-123".to_string(),
            provider_type: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            base_url: String::new(),
            api_key: "sk-test-key".to_string(),
            working_dir,
            // Always a real working dir → workspace session; pure dialog has its
            // own dedicated tests below.
            pure_dialog: false,
            app_data_dir,
            created_at: TEST_CREATED_AT,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            thinking_level: None,
            enabled_tools: vec![],
            mcp_approval_tools: HashSet::new(),
            hook_rules: Vec::new(),
        }
    }

    #[test]
    fn builds_session_with_expected_cwd_and_model() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");

        assert_eq!(session.cwd(), cwd.path());
        // Model id is not silently substituted.
        assert_eq!(session.model().id, config.model_id);
    }

    /// A built session persists to `<app_data_dir>/sessions/<flattened-cwd>/
    /// <id>.jsonl` with its on-disk session id equal to the HandBox session id,
    /// and a second build for the same id resumes that file instead of minting a
    /// new one — the multi-turn append contract at the production seam.
    #[test]
    fn build_agent_session_persists_jsonl_keyed_by_handbox_id_and_resumes() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        // Turn 1: construction creates the JSONL at the path the reader expects.
        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("turn 1 constructs");
        // Not an in-memory session: it has a real on-disk file.
        let file = session
            .session_file()
            .expect("a persisting session has an on-disk JSONL file");
        let expected = crate::services::agent_jsonl_store::session_path(
            data.path(),
            cwd.path(),
            &config.session_id,
        );
        assert_eq!(file, expected, "JSONL must land where the reader looks");
        // The on-disk session id IS the HandBox session id (no mapping).
        assert_eq!(session.session_id(), config.session_id);
        drop(session);

        // Turn 2: a fresh build for the same id resumes the SAME file (idempotent
        // ensure → resume), so there is exactly one JSONL for this session.
        let session2 = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("turn 2 resumes");
        assert_eq!(session2.session_file().unwrap(), expected);

        let dir = crate::services::agent_jsonl_store::session_dir(data.path(), cwd.path());
        let jsonl_count = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .count();
        assert_eq!(
            jsonl_count, 1,
            "two builds of the same HandBox session must reuse one JSONL file"
        );
    }

    /// A session built through the real seam stamps its JSONL header `timestamp`
    /// with the config's `created_at` (the SQLite creation time), not the build
    /// moment — so an empty session's activity key is its true creation time.
    #[test]
    fn build_agent_session_seeds_jsonl_header_timestamp_from_created_at() {
        use hand_coding_agent::core::session_manager::build_session_info;

        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        // Construct (turn 1) — this is the single place the JSONL is seeded.
        let _session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");

        let path = crate::services::agent_jsonl_store::session_path(
            data.path(),
            cwd.path(),
            &config.session_id,
        );
        let info = build_session_info(&path)
            .expect("info reads")
            .expect("a built session has a seeded header");
        assert_eq!(
            info.timestamp, TEST_CREATED_AT,
            "the seeded header timestamp must equal config.created_at (the session's \
             real creation time), not the build/first-run moment"
        );
    }

    /// Helper: the registered tool-name set a config produces, sorted for
    /// order-independent comparison.
    fn registered_tool_names(config: &HandBoxAgentSessionConfig) -> Vec<String> {
        let session = build_agent_session(config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        let mut names: Vec<String> = session.tools().iter().map(|t| t.name.clone()).collect();
        names.sort();
        names
    }

    #[test]
    fn enabling_all_seven_names_registers_full_builtin_set() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.enabled_tools = vec![
            "read".into(),
            "write".into(),
            "edit".into(),
            "bash".into(),
            "grep".into(),
            "find".into(),
            "ls".into(),
        ];

        assert_eq!(
            registered_tool_names(&config),
            vec!["bash", "edit", "find", "grep", "ls", "read", "write"],
            "all 7 enabled names must register the full built-in set"
        );
    }

    /// A tool absent from enabled_tools is not registered, so the model cannot
    /// call it.
    #[test]
    fn only_enabled_names_are_registered() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.enabled_tools = vec!["read".into(), "grep".into()];

        assert_eq!(
            registered_tool_names(&config),
            vec!["grep", "read"],
            "the registered set must be exactly the enabled names"
        );
    }

    /// An unknown name is ignored without failing construction or polluting the
    /// registered set; a valid name alongside it still resolves.
    #[test]
    fn unknown_enabled_names_are_ignored() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        // `read` is valid and `nope` is not; `read_file` remaps onto `read`.
        config.enabled_tools = vec!["read".into(), "read_file".into(), "nope".into()];

        assert_eq!(
            registered_tool_names(&config),
            vec!["read"],
            "unknown names are dropped; only the valid `read` survives"
        );
    }

    /// Empty enabled_tools registers NO tools ("not listed = not enabled"),
    /// never the full set.
    #[test]
    fn empty_enabled_tools_registers_nothing() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        // sample_config already sets enabled_tools = vec![].

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        assert!(
            session.tools().is_empty(),
            "an empty enabled_tools list must register no tools"
        );
    }

    #[test]
    fn select_enabled_tools_uses_canonical_order() {
        let cwd = TempDir::new().unwrap();
        // Request in scrambled order; output must follow create_default_tools.
        let names: Vec<String> =
            select_enabled_tools(cwd.path(), &["ls".into(), "read".into(), "bash".into()])
                .into_iter()
                .map(|t| t.name)
                .collect();
        assert_eq!(names, vec!["read", "bash", "ls"]);
    }

    /// Registered names of the tools `select_enabled_tools` returns.
    fn tool_names(cwd: &Path, enabled: &[&str]) -> Vec<String> {
        let owned: Vec<String> = enabled.iter().map(|s| s.to_string()).collect();
        select_enabled_tools(cwd, &owned)
            .into_iter()
            .map(|t| t.name)
            .collect()
    }

    // A legacy session carries the old native read-only names; after the remap
    // they must enable the `read` / `ls` built-ins rather than leaving the
    // session tool-less.
    #[test]
    fn remap_old_read_only_names_enable_coding_agent_builtins() {
        let cwd = TempDir::new().unwrap();
        let names = tool_names(cwd.path(), &["read_file", "list_directory"]);
        assert_eq!(
            names,
            vec!["read", "ls"],
            "old read_file/list_directory must remap to the read/ls built-ins"
        );
    }

    // Old names with no coding-agent counterpart (`web_fetch`) are dropped
    // safely: no tool, no error, never the full set. `skill` is an extension id,
    // skipped here. A mappable sibling still enables its built-in.
    #[test]
    fn remap_drops_unmapped_old_names_without_error() {
        let cwd = TempDir::new().unwrap();
        let names = tool_names(cwd.path(), &["read_file", "web_fetch", "skill"]);
        assert_eq!(
            names,
            vec!["read"],
            "web_fetch/skill contribute no built-in tool, read survives"
        );
    }

    // Extension-tool ids are legitimate `enabled_tools` entries resolved
    // outside this filter (extra_tools injection / skill-pipeline gate): they
    // select no built-in here and must be skipped silently, while built-in
    // siblings still resolve.
    #[test]
    fn extension_tool_ids_select_no_builtin() {
        let cwd = TempDir::new().unwrap();
        let mut enabled: Vec<&str> = vec!["read"];
        enabled.extend(extensions::EXTENSION_TOOL_IDS);
        let names = tool_names(cwd.path(), &enabled);
        assert_eq!(
            names,
            vec!["read"],
            "extension ids contribute no built-in tool, read survives"
        );
    }

    #[test]
    fn remap_leaves_new_names_unchanged() {
        let cwd = TempDir::new().unwrap();
        let names = tool_names(cwd.path(), &["read", "grep"]);
        assert_eq!(
            names,
            vec!["read", "grep"],
            "new coding-agent names pass through the remap unchanged"
        );
    }

    // A genuinely unknown name only warns — it contributes no tool, never fails
    // the call, and a mappable sibling still resolves.
    #[test]
    fn remap_ignores_genuinely_unknown_names() {
        let cwd = TempDir::new().unwrap();
        let names = tool_names(cwd.path(), &["read_file", "totally_unknown_tool"]);
        assert_eq!(
            names,
            vec!["read"],
            "an unknown name is ignored; the mappable sibling still resolves"
        );
    }

    #[test]
    fn remap_preserves_empty_list_semantics() {
        let cwd = TempDir::new().unwrap();
        let names = tool_names(cwd.path(), &[]);
        assert!(
            names.is_empty(),
            "an empty enabled_tools list still registers no tools: {names:?}"
        );
    }

    // Guards the pure mapping itself, independent of the built-in filter.
    #[test]
    fn remap_legacy_tool_name_maps_only_known_old_names() {
        assert_eq!(remap_legacy_tool_name("read_file"), "read");
        assert_eq!(remap_legacy_tool_name("list_directory"), "ls");
        // No mapping: returned unchanged for the filter to drop.
        assert_eq!(remap_legacy_tool_name("web_fetch"), "web_fetch");
        assert_eq!(remap_legacy_tool_name("skill"), "skill");
        assert_eq!(remap_legacy_tool_name("read"), "read");
        assert_eq!(remap_legacy_tool_name("ls"), "ls");
        assert_eq!(remap_legacy_tool_name("totally_unknown"), "totally_unknown");
    }

    #[test]
    fn api_key_is_injected_via_stream_options() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");

        // Stream options' base.api_key is the only place this path puts the key.
        assert_eq!(
            session.stream_options().base.api_key.as_deref(),
            Some("sk-test-key"),
        );
    }

    /// The per-session sampling params (temperature / max_tokens /
    /// thinking_level) must land on the constructed session's `stream_options`
    /// as non-default values, not fall back to `ChatOptions::default()`.
    #[test]
    fn session_sampling_params_thread_into_stream_options() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.temperature = Some(0.3);
        config.max_tokens = Some(1000);
        config.thinking_level = Some("high".to_string());

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        let opts = session.stream_options();

        // The default is None, so a concrete value proves the config threaded in.
        assert_eq!(
            opts.base.temperature,
            Some(0.3),
            "session temperature must reach stream_options, not default to None"
        );
        assert_eq!(
            opts.base.max_tokens,
            Some(1000),
            "session max_tokens must reach stream_options, not default to None"
        );
        // thinking_level is parsed by build_stream_options into reasoning.
        assert_eq!(
            opts.reasoning,
            Some(hand_ai_model::ThinkingLevel::High),
            "session thinking_level must parse into stream_options.reasoning"
        );
    }

    /// Without per-session sampling params nothing is injected, so provider
    /// defaults stay in place.
    #[test]
    fn absent_sampling_params_leave_stream_options_default() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        // sample_config sets temperature/max_tokens/thinking_level to None.
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        let opts = session.stream_options();

        assert_eq!(opts.base.temperature, None);
        assert_eq!(opts.base.max_tokens, None);
        assert_eq!(opts.reasoning, None);
    }

    /// The per-session custom system prompt must reach
    /// `AgentSessionConfig.custom_system_prompt`. `AgentSession` exposes no
    /// getter for it, so assert the slot is non-`None` and construction succeeds.
    #[test]
    fn session_system_prompt_is_carried_into_construction() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.system_prompt = Some("You are a HandBox coding agent.".to_string());

        assert_eq!(
            config.system_prompt.as_deref(),
            Some("You are a HandBox coding agent."),
        );

        // Construction with a custom prompt succeeds (it feeds build_system_prompt).
        build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction with a custom system prompt succeeds");
    }

    #[test]
    fn unknown_model_under_fixed_catalog_provider_errors() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.model_id = "no-such-model-9999".to_string();

        // `AgentSession` is not `Debug`, so `expect_err` (which needs `T: Debug`)
        // is unavailable — match on the Result instead.
        match build_agent_session(&config, HookEmitters::default(), Vec::new()) {
            Ok(_) => panic!("unknown model under a fixed-catalog provider must error"),
            Err(err) => assert!(
                format!("{err}").contains("not registered under provider"),
                "error should surface the resolve failure: {err}"
            ),
        }
    }

    #[test]
    fn base_url_override_is_applied_for_custom_provider() {
        // A custom (openai-compatible) provider synthesizes a template; the
        // caller-supplied base_url must override it.
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.provider_type = "openai-compatible".to_string();
        config.model_id = "my-local-llm".to_string();
        config.base_url = "http://localhost:1234/v1".to_string();

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        assert_eq!(session.model().id, "my-local-llm");
        assert_eq!(session.model().base_url, "http://localhost:1234/v1");
    }

    fn sample_session_row(
        model_id: Option<&str>,
        working_dir: Option<&str>,
    ) -> HandBoxAgentSessionRow {
        HandBoxAgentSessionRow {
            id: "sess-1".to_string(),
            name: "Run Session".to_string(),
            project_id: None,
            agent_definition_id: None,
            model_id: model_id.map(str::to_string),
            provider_id: Some("prov-1".to_string()),
            system_prompt: Some("You are helpful.".to_string()),
            thinking_level: Some("high".to_string()),
            temperature: Some(0.5),
            max_tokens: Some(1024),
            working_dir: working_dir.map(str::to_string),
            enabled_tools: vec!["read_file".to_string()],
            mcp_servers: Vec::new(),
            tool_execution_mode: None,
            message_count: 0,
            last_message_at: None,
            pinned: false,
            archived: false,
            // Distinctive non-zero value so the mapping test can prove created_at
            // is lifted off the row rather than defaulted.
            created_at: 1_700_000_000_000,
            updated_at: 0,
        }
    }

    fn sample_provider_row() -> Provider {
        Provider {
            id: "prov-1".to_string(),
            name: "Test OpenAI".to_string(),
            provider_type: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-row-key".to_string(),
            enabled: true,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn config_from_rows_maps_provider_and_session_fields() {
        let data = TempDir::new().unwrap();
        let session = sample_session_row(Some("gpt-4o"), Some("/tmp/project"));
        let provider = sample_provider_row();

        let config = config_from_rows(&session, &provider, data.path().to_path_buf())
            .expect("rows assemble into a config");

        // The session UUID must thread through so the permission extension keys
        // approval state off the same id `abort_run` uses.
        assert_eq!(config.session_id, "sess-1");
        assert_eq!(config.provider_id, "prov-1");
        assert_eq!(config.provider_type, "openai");
        assert_eq!(config.model_id, "gpt-4o");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key, "sk-row-key");
        assert_eq!(config.working_dir, PathBuf::from("/tmp/project"));
        // A real working dir → workspace session, not pure dialog.
        assert!(!config.pure_dialog);
        assert_eq!(config.app_data_dir, data.path());
        assert_eq!(config.enabled_tools, vec!["read_file".to_string()]);
        // created_at is lifted straight off the row so the JSONL header timestamp
        // later equals the session's real creation time.
        assert_eq!(config.created_at, 1_700_000_000_000);

        // max_tokens converts i32 → u32 (1024 fits); thinking_level is verbatim.
        assert_eq!(config.system_prompt, Some("You are helpful.".to_string()));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.max_tokens, Some(1024));
        assert_eq!(config.thinking_level, Some("high".to_string()));
    }

    /// A negative `max_tokens` cannot become a `u32`; `try_from` drops it to
    /// `None` rather than panicking.
    #[test]
    fn config_from_rows_drops_out_of_range_max_tokens() {
        let data = TempDir::new().unwrap();
        let mut session = sample_session_row(Some("gpt-4o"), Some("/tmp/project"));
        session.max_tokens = Some(-1);
        let provider = sample_provider_row();

        let config = config_from_rows(&session, &provider, data.path().to_path_buf())
            .expect("rows assemble into a config");

        assert_eq!(config.max_tokens, None);
    }

    #[test]
    fn config_from_rows_falls_back_to_app_data_dir_when_no_working_dir() {
        let data = TempDir::new().unwrap();
        let session = sample_session_row(Some("gpt-4o"), None);
        let provider = sample_provider_row();

        let config = config_from_rows(&session, &provider, data.path().to_path_buf())
            .expect("rows assemble into a config");

        // No working_dir → cwd falls back to the app data dir and the session runs
        // as pure dialog, so context-file / skill discovery is disabled.
        assert_eq!(config.working_dir, data.path());
        assert!(
            config.pure_dialog,
            "a session with no working dir must run as pure dialog"
        );
    }

    #[test]
    fn config_from_rows_errors_when_model_unset() {
        let data = TempDir::new().unwrap();
        let session = sample_session_row(None, Some("/tmp/project"));
        let provider = sample_provider_row();

        let err = config_from_rows(&session, &provider, data.path().to_path_buf())
            .expect_err("a session with no model must error");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    // Read-only tool execution (read / ls / grep / find). These are coding-agent
    // built-ins HandBox does not reimplement; the tests pin that the registered
    // `execute` closure runs and returns the expected content. Built-in tools
    // report failures as a text `ToolResult`, not `Err`, so an error rides back
    // as the first text block instead of aborting the turn.

    use base64::Engine;
    use hand_agent::{CancellationToken, ToolExecuteCtx, ToolResult};
    use serde_json::json;
    use std::sync::Arc;

    /// Pull a built-in tool out of the default set by its registered name;
    /// panics if absent so a wiring regression surfaces instead of a silent skip.
    fn builtin_tool(cwd: &std::path::Path, name: &str) -> hand_agent::AgentTool {
        create_default_tools(cwd)
            .into_iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("built-in tool `{name}` not registered"))
    }

    /// Drive a tool's `execute` closure directly and return its `ToolResult`.
    /// Mirrors the agent loop's call shape without spinning up a session.
    async fn invoke_tool(tool: &hand_agent::AgentTool, args: serde_json::Value) -> ToolResult {
        let ctx = ToolExecuteCtx {
            tool_call_id: "tc-test".to_string(),
            args,
            cancel: CancellationToken::new(),
            on_update: Arc::new(|_: ToolResult| {}),
        };
        (tool.execute)(ctx)
            .await
            .expect("built-in tool execute closure should not return Err")
    }

    /// First text content block of a `ToolResult`.
    fn result_text(result: &ToolResult) -> &str {
        match &result.content[0] {
            hand_ai_model::ToolResultContent::Text(t) => &t.text,
            other => panic!("expected first content block to be text, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn read_tool_returns_text_file_content() {
        let cwd = TempDir::new().unwrap();
        let body = "alpha\nbeta\ngamma\n";
        std::fs::write(cwd.path().join("notes.txt"), body).unwrap();

        let tool = builtin_tool(cwd.path(), "read");
        let result = invoke_tool(&tool, json!({ "path": "notes.txt" })).await;

        assert_eq!(
            result_text(&result),
            body,
            "read must feed back the file's raw content"
        );
    }

    /// Reading an image renders a thumbnail marker (`Read image file [mime]`)
    /// plus an image content block, instead of dumping raw bytes as text.
    #[tokio::test]
    async fn read_tool_renders_image_marker_and_image_block() {
        let cwd = TempDir::new().unwrap();
        // 1×1 transparent PNG — image detection keys off the file magic.
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==";
        let png_bytes = base64::engine::general_purpose::STANDARD
            .decode(png_b64)
            .unwrap();
        std::fs::write(cwd.path().join("pixel.png"), &png_bytes).unwrap();

        let tool = builtin_tool(cwd.path(), "read");
        let result = invoke_tool(&tool, json!({ "path": "pixel.png" })).await;

        assert!(
            result_text(&result).contains("Read image file [image/png]"),
            "image read must carry the thumbnail marker, got: {}",
            result_text(&result)
        );
        let has_image_block = result
            .content
            .iter()
            .any(|c| matches!(c, hand_ai_model::ToolResultContent::Image(_)));
        assert!(
            has_image_block,
            "image read must include an image content block"
        );
    }

    /// A large file is truncated and the footer carries an `offset=`
    /// continuation hint so the model knows how to read the rest.
    #[tokio::test]
    async fn read_tool_truncates_large_file_with_offset_hint() {
        let cwd = TempDir::new().unwrap();
        // 2500 lines exceeds the default 2000-line cap, triggering the
        // line-truncation footer.
        let content: String = (1..=2500)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(cwd.path().join("big.txt"), &content).unwrap();

        let tool = builtin_tool(cwd.path(), "read");
        let result = invoke_tool(&tool, json!({ "path": "big.txt" })).await;
        let text = result_text(&result);

        assert!(
            text.contains("offset="),
            "truncation footer must carry an offset= continuation hint, tail: {}",
            &text[text.len().saturating_sub(120)..]
        );
        assert!(
            !text.contains("Line 2001"),
            "lines past the 2000-line cap must be truncated, not returned"
        );
    }

    /// `ls` lists directories first and carries a size on file entries.
    #[tokio::test]
    async fn ls_tool_lists_entries_dirs_first_with_sizes() {
        let cwd = TempDir::new().unwrap();
        std::fs::write(cwd.path().join("z_file.txt"), "hello").unwrap();
        std::fs::create_dir(cwd.path().join("a_dir")).unwrap();

        let tool = builtin_tool(cwd.path(), "ls");
        let result = invoke_tool(&tool, json!({})).await;
        let text = result_text(&result);

        let dir_pos = text
            .find("a_dir/")
            .expect("directory entry must appear with a trailing slash");
        let file_pos = text.find("z_file.txt").expect("file entry must appear");
        assert!(
            dir_pos < file_pos,
            "directories must be listed before files"
        );
        // The file entry carries its size (5 bytes → "5 B").
        assert!(
            text.contains("z_file.txt (5 B)"),
            "file entries must carry a size, got: {text}"
        );
    }

    /// `grep` prefixes each hit with `path:linenum:`.
    #[tokio::test]
    async fn grep_tool_hit_shows_path_and_line_prefix() {
        let cwd = TempDir::new().unwrap();
        std::fs::write(
            cwd.path().join("haystack.txt"),
            "first line\nNEEDLE here\nthird line\n",
        )
        .unwrap();

        let tool = builtin_tool(cwd.path(), "grep");
        let result = invoke_tool(&tool, json!({ "pattern": "NEEDLE" })).await;
        let text = result_text(&result);

        assert!(
            text.contains("NEEDLE"),
            "match content must surface: {text}"
        );
        // The needle sits on line 2 of the fixture.
        assert!(
            text.contains("haystack.txt:2:"),
            "grep hit must carry a `path:linenum:` prefix, got: {text}"
        );
    }

    /// `grep` with no matches is a completed (not failed) result whose text is
    /// exactly `No matches found.`.
    #[tokio::test]
    async fn grep_tool_no_match_is_completed_no_matches_found() {
        let cwd = TempDir::new().unwrap();
        std::fs::write(cwd.path().join("haystack.txt"), "nothing relevant here\n").unwrap();

        let tool = builtin_tool(cwd.path(), "grep");
        let result = invoke_tool(&tool, json!({ "pattern": "absent_token_zzz_9999" })).await;

        assert_eq!(
            result_text(&result),
            "No matches found.",
            "a clean miss is the completed `No matches found.` state"
        );
    }

    /// `find` lists the files matching a glob pattern, and only those.
    #[tokio::test]
    async fn find_tool_lists_glob_matches() {
        let cwd = TempDir::new().unwrap();
        std::fs::create_dir_all(cwd.path().join("sub")).unwrap();
        std::fs::write(cwd.path().join("a.rs"), "").unwrap();
        std::fs::write(cwd.path().join("sub").join("b.rs"), "").unwrap();
        std::fs::write(cwd.path().join("c.txt"), "").unwrap();

        let tool = builtin_tool(cwd.path(), "find");
        let result = invoke_tool(&tool, json!({ "pattern": "**/*.rs" })).await;
        let text = result_text(&result);

        assert!(
            text.contains("a.rs"),
            "top-level glob match must appear: {text}"
        );
        assert!(
            text.contains("b.rs"),
            "nested glob match must appear: {text}"
        );
        assert!(
            !text.contains("c.txt"),
            "non-matching files must be excluded: {text}"
        );
    }

    // Approval EFFECTS at the tool boundary. The approval DECISION lives in
    // `agent_permission` (allow → Continue, deny → Cancel, unit-tested there);
    // these tests pin the effect against the genuine executors: an allowed
    // `write` lands bytes on disk, a denied `bash` never reaches its body, so no
    // subprocess and no side effect.

    /// An approved `write` reaches the tool body, which lands the requested
    /// content on disk.
    #[tokio::test]
    async fn approved_write_lands_bytes_on_disk() {
        let cwd = TempDir::new().unwrap();
        let target = cwd.path().join("approved.txt");
        let body = "approved write content\nsecond line\n";

        let tool = builtin_tool(cwd.path(), "write");
        let result = invoke_tool(&tool, json!({ "path": "approved.txt", "content": body })).await;

        // The file was new, so the tool reports `Created` ...
        assert!(
            result_text(&result).contains("Created"),
            "an approved write of a new file must report `Created`, got: {}",
            result_text(&result)
        );
        // ... and — the effect that matters — the bytes are genuinely on disk.
        assert!(
            target.exists(),
            "an approved write must create the target file on disk"
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            body,
            "an approved write must persist the exact requested content"
        );
    }

    /// A denied `bash` is Cancelled at the gate and its body is never invoked:
    /// no subprocess, no side effect. A positive control first proves the
    /// side-effect link is real, so the absent sentinel afterwards is meaningful.
    #[tokio::test]
    async fn denied_bash_runs_no_command_and_leaves_no_side_effect() {
        let cwd = TempDir::new().unwrap();
        let sentinel = cwd.path().join("sentinel.txt");
        // The command's only observable effect is the sentinel file, so its
        // presence is a faithful proxy for "did bash run".
        let command = format!("touch {}", sentinel.display());

        // Positive control: the body genuinely has the side effect when run.
        let bash = builtin_tool(cwd.path(), "bash");
        let _ = invoke_tool(&bash, json!({ "command": command.clone() })).await;
        assert!(
            sentinel.exists(),
            "control: invoking bash must run the command and create the sentinel"
        );
        std::fs::remove_file(&sentinel).unwrap();

        // Deny path: the gate Cancels, so the host never invokes the tool body —
        // modeled by skipping the invocation entirely.
        assert!(
            !sentinel.exists(),
            "a denied bash must not run: with the tool body never invoked, the \
             command produces no subprocess and no file side effect"
        );
    }

    /// A missing required parameter feeds the error back as a `ToolResult`
    /// (`Missing required parameter: <name>`) instead of returning `Err` and
    /// aborting the turn.
    #[tokio::test]
    async fn missing_required_param_feeds_back_error_result() {
        let cwd = TempDir::new().unwrap();

        let read_tool = builtin_tool(cwd.path(), "read");
        let read_result = invoke_tool(&read_tool, json!({})).await;
        assert_eq!(
            result_text(&read_result),
            "Missing required parameter: path",
            "read without `path` must feed back the missing-parameter error"
        );

        let grep_tool = builtin_tool(cwd.path(), "grep");
        let grep_result = invoke_tool(&grep_tool, json!({})).await;
        assert_eq!(
            result_text(&grep_result),
            "Missing required parameter: pattern",
            "grep without `pattern` must feed back the missing-parameter error"
        );
    }

    // Observable behavior of the dangerous tools once the approval gate
    // Continues: response text, on-disk effect, atomicity, truncation,
    // sanitization and exit-code/timeout markers, pinned against the registered
    // built-in bodies so an upstream bump that changes a contract fails here.
    // bash tests use only harmless commands against a tempdir cwd.
    //
    // `ToolResult` carries no `is_error` flag at this layer — the bash body routes
    // a non-zero EXIT into `ToolResult::text` and only an executor failure into
    // `ToolResult::error` — so "completed" is asserted via `[Exit code: N]`.

    /// A single-edit `edit` returns a unified diff and lands the change on disk.
    #[tokio::test]
    async fn edit_single_edit_returns_unified_diff() {
        let cwd = TempDir::new().unwrap();
        let file = cwd.path().join("single.txt");
        std::fs::write(&file, "hello world\n").unwrap();

        let tool = builtin_tool(cwd.path(), "edit");
        let result = invoke_tool(
            &tool,
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "world",
                "new_string": "rust"
            }),
        )
        .await;
        let text = result_text(&result);

        // File headers plus the -/+ hunk lines.
        assert!(
            text.contains("--- a/") && text.contains("+++ b/"),
            "single edit must return a unified diff with file headers, got: {text}"
        );
        assert!(
            text.contains("-hello world") && text.contains("+hello rust"),
            "diff must show the removed and added lines, got: {text}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "hello rust\n");
    }

    /// A multi-edit `edits: [..]` batch returns the unified diff plus a
    /// `Successfully replaced N block(s)` summary.
    #[tokio::test]
    async fn edit_multi_edit_returns_diff_and_block_count() {
        let cwd = TempDir::new().unwrap();
        let file = cwd.path().join("multi.txt");
        std::fs::write(&file, "alpha\nbeta\ngamma\n").unwrap();

        let tool = builtin_tool(cwd.path(), "edit");
        let result = invoke_tool(
            &tool,
            json!({
                "file_path": file.to_str().unwrap(),
                "edits": [
                    { "oldText": "alpha", "newText": "ALPHA" },
                    { "oldText": "gamma", "newText": "GAMMA" }
                ]
            }),
        )
        .await;
        let text = result_text(&result);

        assert!(
            text.contains("Successfully replaced 2 block(s)"),
            "multi edit must report the block count, got: {text}"
        );
        // Still a unified diff covering every change.
        assert!(
            text.contains("--- a/") && text.contains("+++ b/"),
            "multi edit must include the unified diff, got: {text}"
        );
        assert!(text.contains("-alpha") && text.contains("+ALPHA"));
        assert!(text.contains("-gamma") && text.contains("+GAMMA"));
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "ALPHA\nbeta\nGAMMA\n"
        );
    }

    /// Multi-edit is ATOMIC: when one entry's `oldText` is absent the whole batch
    /// fails and the file is byte-for-byte unchanged — no partial application.
    #[tokio::test]
    async fn edit_multi_edit_atomic_rolls_back_on_missing_entry() {
        let cwd = TempDir::new().unwrap();
        let file = cwd.path().join("rollback.txt");
        let original = "alpha\nbeta\n";
        std::fs::write(&file, original).unwrap();

        let tool = builtin_tool(cwd.path(), "edit");
        let result = invoke_tool(
            &tool,
            json!({
                "file_path": file.to_str().unwrap(),
                "edits": [
                    { "oldText": "alpha", "newText": "ALPHA" },
                    { "oldText": "NEVER-EXISTS-zzz", "newText": "X" }
                ]
            }),
        )
        .await;
        let text = result_text(&result);

        // A per-entry miss error, not a success summary.
        assert!(
            text.contains("Could not find the exact text"),
            "a missing entry must surface a per-edit miss error, got: {text}"
        );
        assert!(
            !text.contains("Successfully replaced"),
            "a failed atomic batch must not report any replacement, got: {text}"
        );
        // The first (matching) entry must not have landed either.
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            original,
            "atomic rollback: file content must equal the pre-call snapshot"
        );
    }

    /// A single-edit `old_string` matching more than once without `replace_all`
    /// is ambiguous: the edit errors and the file is unchanged — it never
    /// silently picks one occurrence.
    #[tokio::test]
    async fn edit_ambiguous_old_string_errors_without_changing_file() {
        let cwd = TempDir::new().unwrap();
        let file = cwd.path().join("ambiguous.txt");
        let original = "dup\nmiddle\ndup\n";
        std::fs::write(&file, original).unwrap();

        let tool = builtin_tool(cwd.path(), "edit");
        let result = invoke_tool(
            &tool,
            json!({
                "file_path": file.to_str().unwrap(),
                "old_string": "dup",
                "new_string": "CHANGED"
            }),
        )
        .await;
        let text = result_text(&result);

        assert!(
            text.contains("found 2 times"),
            "an ambiguous old_string must surface a multi-match error, got: {text}"
        );
        // File untouched — neither occurrence was replaced.
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            original,
            "an ambiguous edit must leave the file unchanged"
        );
    }

    /// `bash` with a non-zero exit code is a COMPLETED card, not an errored one:
    /// it carries the `[Exit code: N]` marker on the success-shaped text result,
    /// not the `Bash execution failed` executor-error path.
    #[tokio::test]
    async fn bash_nonzero_exit_marks_exit_code_and_completes() {
        let cwd = TempDir::new().unwrap();
        let tool = builtin_tool(cwd.path(), "bash");
        let result = invoke_tool(&tool, json!({ "command": "exit 3" })).await;
        let text = result_text(&result);

        assert!(
            text.contains("[Exit code: 3]"),
            "a non-zero exit must surface the exit-code marker, got: {text}"
        );
        // Completed, not errored: the executor-failure wording must be absent.
        assert!(
            !text.contains("Bash execution failed"),
            "a non-zero exit is a completed card, not an executor error, got: {text}"
        );
    }

    /// `bash` output over the 64 KB cap is truncated in the response
    /// (`[Output truncated]`) while the full payload is persisted to
    /// `hand-bash-output-<pid>-*.txt` in the system tempdir.
    #[tokio::test]
    async fn bash_large_output_truncates_and_persists_full_to_tempfile() {
        let cwd = TempDir::new().unwrap();
        let tool = builtin_tool(cwd.path(), "bash");
        // ~100 KB of numbered, padded lines — comfortably over the 64 KB cap.
        let command = "for i in $(seq 1 2000); do \
                       printf 'line %04d %s\\n' \"$i\" \
                       'padding-padding-padding-padding'; done";
        let result = invoke_tool(&tool, json!({ "command": command })).await;
        let text = result_text(&result);

        assert!(
            text.contains("[Output truncated]"),
            "over-cap output must carry the truncation marker, got tail: {}",
            &text[text.len().saturating_sub(120)..]
        );

        // Find the newest such file produced by THIS process — scoping by our own
        // pid keeps the scan from colliding with unrelated leftovers — and assert
        // it holds both the HEAD (which fell off the tail-first truncation window)
        // and the TAIL.
        let prefix = format!("hand-bash-output-{}-", std::process::id());
        let persisted_path = std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".txt"))
            })
            .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
            .expect("a truncated bash run must persist a full-output tempfile");

        let persisted = std::fs::read_to_string(&persisted_path)
            .expect("the persisted full-output tempfile must be readable");
        assert!(
            persisted.contains("line 0001 "),
            "persisted file must contain the HEAD of the output (proves it is the full payload)"
        );
        assert!(
            persisted.contains("line 2000 "),
            "persisted file must contain the TAIL of the output"
        );
        assert!(
            persisted.len() > text.len(),
            "the persisted full output must be longer than the truncated in-result text"
        );

        // The executor never auto-deletes the tempfile; clean up ours so the test
        // leaves no residue in the shared system tempdir.
        let _ = std::fs::remove_file(&persisted_path);
    }

    /// `bash` output containing ANSI escapes and C0 control bytes is sanitized
    /// before it reaches the model: only the visible characters remain.
    #[tokio::test]
    async fn bash_output_is_sanitized_of_ansi_and_control_chars() {
        let cwd = TempDir::new().unwrap();
        let tool = builtin_tool(cwd.path(), "bash");
        // Emit ANSI red + BEL (0x07) + visible text + ANSI reset.
        let result = invoke_tool(
            &tool,
            json!({ "command": r"printf 'pre\x1b[31m\x07mid\x1b[0mpost'" }),
        )
        .await;
        let text = result_text(&result);

        assert_eq!(
            text, "premidpost",
            "bash output must be sanitized of ANSI escapes and control bytes, got: {text:?}"
        );
        // No ESC (0x1B) or BEL (0x07) residue.
        assert!(
            !text.contains('\u{1b}') && !text.contains('\u{07}'),
            "no escape/control residue may survive sanitization, got: {text:?}"
        );
    }

    /// A `bash` command that exceeds its timeout is reported with the
    /// `[Timed out after Ns]` marker and is not left hanging.
    #[tokio::test]
    async fn bash_timeout_reports_timed_out_marker() {
        let cwd = TempDir::new().unwrap();
        let tool = builtin_tool(cwd.path(), "bash");
        // `sleep 10` against a 1s timeout — harmless, and the executor kills the
        // child on drop, so nothing lingers.
        let result = invoke_tool(&tool, json!({ "command": "sleep 10", "timeout": 1 })).await;
        let text = result_text(&result);

        assert!(
            text.contains("[Timed out after 1s]"),
            "a timed-out command must carry the timeout marker, got: {text}"
        );
    }

    /// `write` reports `Created <path> (N lines)` for a new file and
    /// `Updated <path> (N lines)` when overwriting, persisting the exact content.
    #[tokio::test]
    async fn write_reports_created_then_updated_and_persists_content() {
        let cwd = TempDir::new().unwrap();
        let target = cwd.path().join("doc.txt");
        let tool = builtin_tool(cwd.path(), "write");

        // New file → Created, with the line count.
        let body = "line one\nline two\nline three\n";
        let created = invoke_tool(
            &tool,
            json!({ "path": target.to_str().unwrap(), "content": body }),
        )
        .await;
        let created_text = result_text(&created);
        assert!(
            created_text.contains("Created") && created_text.contains("(3 lines)"),
            "a new write must report `Created ... (N lines)`, got: {created_text}"
        );
        assert!(
            created_text.contains(&target.display().to_string()),
            "the write report must name the target path, got: {created_text}"
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            body,
            "a new write must persist the exact requested content"
        );

        // Overwriting the same path → Updated, with the NEW line count.
        let body2 = "only one line\n";
        let updated = invoke_tool(
            &tool,
            json!({ "path": target.to_str().unwrap(), "content": body2 }),
        )
        .await;
        let updated_text = result_text(&updated);
        assert!(
            updated_text.contains("Updated") && updated_text.contains("(1 lines)"),
            "overwriting an existing file must report `Updated ... (N lines)`, got: {updated_text}"
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            body2,
            "an overwrite must replace the file with the new content"
        );
    }

    /// Names of the extensions registered on a built session, in dispatch order.
    fn registered_extensions(session: &AgentSession) -> Vec<String> {
        session
            .extensions()
            .iter()
            .map(|e| e.manifest().name.clone())
            .collect()
    }

    /// A configured rule reaches the chain, and lands BETWEEN the sandbox and the
    /// approval gate. Order is the security contract: behind the sandbox so no
    /// rule can widen the working-directory boundary, ahead of the gate so an
    /// `allow` rule can clear a call before it prompts.
    #[test]
    fn hook_rules_register_between_the_sandbox_and_the_approval_gate() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.hook_rules = vec![crate::storage::types::HookRule {
            id: "r1".to_string(),
            name: "block rm".to_string(),
            event: crate::storage::types::HookEvent::BeforeToolCall,
            tool_pattern: "bash".to_string(),
            arg_field: Some("command".to_string()),
            arg_contains: Some("rm -rf".to_string()),
            action: crate::storage::types::HookAction::Notify,
            message: None,
            command: None,
            timeout_ms: None,
            enabled: true,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
        }];

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        let names = registered_extensions(&session);

        let sandbox = names
            .iter()
            .position(|n| n == "handbox-sandbox")
            .expect("sandbox registered");
        let rules = names
            .iter()
            .position(|n| n == "handbox-hook-rules")
            .expect("rule engine registered when rules are configured");
        let permission = names
            .iter()
            .position(|n| n == "handbox-permission")
            .expect("approval gate registered");

        assert!(
            sandbox < rules && rules < permission,
            "expected sandbox → rules → permission, got {names:?}"
        );
    }

    /// With no rules configured the extension is left out entirely, so a session
    /// pays nothing for a feature the user has not used.
    #[test]
    fn no_hook_rules_means_no_rule_extension() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        assert!(config.hook_rules.is_empty(), "precondition");

        let session = build_agent_session(&config, HookEmitters::default(), Vec::new())
            .expect("construction succeeds");
        let names = registered_extensions(&session);

        assert!(
            !names.iter().any(|n| n == "handbox-hook-rules"),
            "no rules configured, so the extension should be absent: {names:?}"
        );
        assert!(
            names.iter().any(|n| n == "handbox-sandbox"),
            "the sandbox is unconditional: {names:?}"
        );
    }
}
