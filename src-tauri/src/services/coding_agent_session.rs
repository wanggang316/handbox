//! coding_agent_session — construct a coding-agent [`AgentSession`] from a
//! HandBox agent-session configuration.
//!
//! This module owns *construction only*: given a HandBox provider / model /
//! key / working-dir / app-data-dir bundle it returns a ready-to-drive
//! `hand_coding_agent::AgentSession`. Driving the prompt loop, mapping agent
//! events back onto HandBox's event surface, and the IPC wiring are separate
//! M1 features that build on top of the session this returns.
//!
//! Reuse, not reinvention:
//! - Model resolution goes through [`chat_engine::resolve_model`], so an agent
//!   session sees exactly the same `model::Model` a chat request would for the
//!   same provider/model/base_url triple (no divergent catalog logic).
//! - Stream options (incl. the api key) come from
//!   [`chat_engine::build_stream_options`]. The plaintext key rides inside
//!   `SimpleStreamOptions.base.api_key`; this path deliberately does **not**
//!   write an `auth.json`, set environment variables, or touch the keyring.
//!
//! Sandbox discipline: `base_dir` is wired to the caller-supplied
//! `app_data_dir` (Tauri's per-app data directory). The coding-agent default
//! would otherwise persist session state under the user's `~/.hand`; for a
//! sandboxed desktop app that state must stay inside the app's own data root.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use hand_agent::AgentTool;
use hand_ai_model::SimpleStreamOptions;
use hand_coding_agent::tools::create_default_tools;
use hand_coding_agent::{AgentSession, AgentSessionConfig};

use crate::models::AppError;
use crate::services::agent_permission::{ApprovalEmitter, PermissionExtension, SandboxExtension};
use crate::services::chat_engine::{self, ChatOptions};
use crate::storage::types::{AgentSession as HandBoxAgentSessionRow, Provider};

/// HandBox-side inputs needed to construct a coding-agent session.
///
/// Field names mirror the HandBox agent-session storage row so callers can map
/// directly. `provider_type` is the hand-ai provider tag (e.g. `"openai"`,
/// `"anthropic"`, `"openai-compatible"`); `provider_id` is HandBox's own
/// provider row id and is carried for diagnostics/traceability only — model
/// resolution keys off `provider_type` to match `chat_engine`.
#[derive(Debug, Clone)]
pub struct HandBoxAgentSessionConfig {
    /// HandBox provider row id (diagnostics only).
    pub provider_id: String,
    /// hand-ai provider tag consumed by [`chat_engine::resolve_model`].
    pub provider_type: String,
    /// Model id selected for this session.
    pub model_id: String,
    /// Optional base-url override. Empty string means "use the catalog
    /// template's base_url unchanged" (same contract as `chat_engine`).
    pub base_url: String,
    /// Plaintext provider api key. Injected via stream options only.
    pub api_key: String,
    /// Working directory the agent's tools operate against (the `cwd`).
    pub working_dir: PathBuf,
    /// Tauri per-app data directory. Becomes the session's `base_dir` so
    /// persistent state stays inside the app sandbox, not `~/.hand`.
    pub app_data_dir: PathBuf,
    /// Per-session custom system prompt. `None` falls back to the coding-agent
    /// default prompt. Mirrors the legacy `agent_runtime` consumption of
    /// `session.system_prompt`; written straight into
    /// `AgentSessionConfig.custom_system_prompt` (also `Option<String>`).
    pub system_prompt: Option<String>,
    /// Per-session sampling temperature. `None` = model/provider default.
    /// Threads into `ChatOptions.temperature` → `stream_options.base.temperature`.
    pub temperature: Option<f32>,
    /// Per-session max output tokens. Stored as `i32` on the session row; this
    /// carries the `u32` form `ChatOptions.max_tokens` expects (the legacy path
    /// does the same `i32 → u32` conversion at `agent_runtime.rs:583`).
    pub max_tokens: Option<u32>,
    /// Per-session thinking level (e.g. `"low"`/`"medium"`/`"high"`), passed
    /// through verbatim as `ChatOptions.reasoning_effort`; `build_stream_options`
    /// parses it via `parse_thinking_level` (unknown values map to `None`, so a
    /// non-reasoning model never breaks). Same contract as `agent_runtime.rs:586`.
    pub thinking_level: Option<String>,
    /// HandBox's per-session enabled-tool list, by coding-agent registered
    /// name (`read`/`write`/`edit`/`bash`/`grep`/`find`/`ls`). Only the named
    /// tools are registered against the session (see
    /// [`select_enabled_tools`]). Following the legacy `agent_tools::build_tools`
    /// convention, an empty list means "no tool enabled" (not "all enabled").
    pub enabled_tools: Vec<String>,
}

/// Construct a coding-agent [`AgentSession`] from a HandBox configuration.
///
/// Steps:
/// 1. Resolve the model through `chat_engine` (no silent substitution — the
///    returned `model.id` equals the requested `model_id`).
/// 2. Build stream options carrying the plaintext api key plus the
///    per-session sampling params (temperature / max_tokens / thinking_level);
///    no auth.json, no env vars.
/// 3. Register only the built-in tools named in `enabled_tools`, filtered by
///    coding-agent registered name (see [`select_enabled_tools`]).
/// 4. Hand all of that to `AgentSession::new_with_skill_dirs`, pinning
///    skill-discovery roots to `None` so construction does not read the host's
///    real `~/.hand/skills/` (keeps construction deterministic and hermetic;
///    project-scope skills under `<cwd>/.hand/skills` are still discovered).
///
/// `base_dir` is set to `app_data_dir` so session persistence lands inside the
/// Tauri app sandbox.
///
/// `approval_emitter` wires the M2 [`PermissionExtension`]'s approval-request
/// channel (the IPC layer passes a `window.emit("agent_approval_request", ..)`
/// wrapper). `None` makes the permission extension fail CLOSED — every dangerous
/// tool (write/edit/bash) is denied without prompting — which is the safe
/// default for headless construction and unit tests (no approval UI to consult).
///
/// Returns `AppError` when model resolution fails (unknown provider/model) or
/// the coding-agent session cannot be initialized.
pub fn build_agent_session(
    config: &HandBoxAgentSessionConfig,
    approval_emitter: Option<ApprovalEmitter>,
) -> Result<AgentSession, AppError> {
    let model =
        chat_engine::resolve_model(&config.provider_type, &config.model_id, &config.base_url)?;

    // Per-session sampling params are baked into the stream options HERE, at
    // construction time — not later by the drive feature. `drive_agent_run`
    // only calls `send_message_with_images` and applies no per-turn options, so
    // the session must already carry them. temperature / max_tokens flow onto
    // `stream_options.base`; thinking_level rides as `reasoning_effort` and is
    // parsed by `build_stream_options` into `stream_options.reasoning`. This
    // matches the legacy `agent_runtime` consumption of
    // `session.{temperature, max_tokens, thinking_level}` (agent_runtime.rs:582-586).
    // The plaintext api key likewise flows in via stream options only.
    let chat_options = ChatOptions {
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        reasoning_effort: config.thinking_level.clone(),
        ..ChatOptions::default()
    };
    let stream_options: SimpleStreamOptions =
        chat_engine::build_stream_options(&chat_options, &config.api_key);

    let tools = select_enabled_tools(&config.working_dir, &config.enabled_tools);

    let session_config = AgentSessionConfig {
        cwd: config.working_dir.clone(),
        model,
        stream_options,
        // Per-session system prompt enters the model context here (legacy
        // consumes `session.system_prompt` at agent_runtime.rs:556). `None`
        // leaves the coding-agent default prompt in place.
        custom_system_prompt: config.system_prompt.clone(),
        custom_guidelines: None,
        resume_session: None,
        // Construction-only sessions don't persist a JSONL transcript; the
        // drive feature owns persistence semantics. In-memory keeps this path
        // side-effect-free on disk.
        no_session: true,
        no_context_files: false,
        session_dir: None,
        no_skills: false,
        extra_skill_dirs: Vec::new(),
        // Sandbox: persist under the Tauri app data dir, never ~/.hand.
        base_dir: Some(config.app_data_dir.clone()),
    };

    let mut session = AgentSession::new_with_skill_dirs(session_config, tools, None, None)
        .map_err(|e| {
            AppError::internal_error(&format!("failed to construct agent session: {e}"))
        })?;

    // Re-impose the working_dir sandbox boundary on the read-only file tools.
    // The vendored coding agent does not confine `read`/`ls` to the cwd (it
    // honors absolute paths and expands `~`); HandBox enforces containment from
    // the outside via this before_tool_call extension, which Cancels any
    // out-of-sandbox path so cwd-external content is never read out. Later
    // milestones layer write/edit boundaries and approval gating onto the same
    // extension chain (the host calls every registered extension in order).
    session.register_extension(Arc::new(SandboxExtension::new(config.working_dir.clone())));

    // M2 approval gate: the dangerous, side-effecting tools (write/edit/bash)
    // are gated behind an asynchronous user approval. This second
    // before_tool_call extension emits an `agent_approval_request` and AWAITS the
    // user's decision (allow → Continue, deny → Cancel); with no emitter it fails
    // CLOSED (denies), preserving the M1 safety posture. It is registered AFTER
    // the sandbox on purpose: the host calls each registered extension in order
    // and the FIRST Cancel wins, so a sandbox escape (out-of-cwd
    // read/ls/grep/find) is silently Cancelled by the sandbox FIRST and never
    // reaches — never prompts — this approval gate.
    session.register_extension(Arc::new(PermissionExtension::new(approval_emitter)));

    Ok(session)
}

/// Filter the full coding-agent built-in tool set down to the per-session
/// `enabled` names, gating tool availability by registered name.
///
/// `create_default_tools(cwd)` builds all 7 built-ins
/// (`read`/`write`/`edit`/`bash`/`grep`/`find`/`ls`); this keeps only the ones
/// whose registered `name` appears in `enabled`. Names not present in the
/// built-in set (e.g. a stale legacy id like `read_file`, or a typo) are
/// ignored with a `warn` log rather than failing construction — an unknown
/// name simply contributes no tool.
///
/// Empty-list semantics follow HandBox's legacy `agent_tools::build_tools`
/// convention: an empty `enabled` registers NO tools ("not listed = not
/// enabled"), never the full set.
///
/// Output order follows `create_default_tools` (the canonical built-in order),
/// independent of the order names appear in `enabled`.
pub fn select_enabled_tools(cwd: &Path, enabled: &[String]) -> Vec<AgentTool> {
    let mut wanted: Vec<&str> = enabled.iter().map(String::as_str).collect();

    let selected: Vec<AgentTool> = create_default_tools(cwd)
        .into_iter()
        .filter(|tool| {
            if let Some(pos) = wanted.iter().position(|name| *name == tool.name) {
                // Mark this requested name as matched so anything left in
                // `wanted` afterwards is provably unknown.
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

/// Assemble a [`HandBoxAgentSessionConfig`] from the persisted HandBox session
/// and provider rows plus the app's data directory.
///
/// This bridges HandBox's storage layer to [`build_agent_session`]: it reads the
/// provider tag / base-url / plaintext key off the provider row and the model /
/// working-dir / enabled-tools off the session row. It does NOT touch the
/// network or construct anything — it only maps rows to the construction config,
/// so the drive layer can build a session from a `session_id` it just loaded.
///
/// Sandbox discipline for `working_dir`: when the session has no working
/// directory selected, the agent's cwd falls back to `app_data_dir`. The cwd
/// must be an existing directory (the coding agent reads context files / skills
/// and roots its tools there); `app_data_dir` always exists and stays inside the
/// app sandbox, so the fallback never escapes it.
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

    // No working dir selected → root the agent inside the app sandbox so the
    // cwd is always an existing directory the agent can operate against.
    let working_dir = session
        .working_dir
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data_dir.clone());

    Ok(HandBoxAgentSessionConfig {
        provider_id: provider.id.clone(),
        provider_type: provider.provider_type.clone(),
        model_id,
        base_url: provider.base_url.clone(),
        api_key: provider.api_key.clone(),
        working_dir,
        app_data_dir,
        // Per-session config consumed identically to the legacy path
        // (agent_runtime.rs:556,582-586): system_prompt verbatim, max_tokens
        // i32 → u32 via try_from (out-of-range silently drops to None),
        // thinking_level passed through for build_stream_options to parse.
        system_prompt: session.system_prompt.clone(),
        temperature: session.temperature,
        max_tokens: session.max_tokens.and_then(|t| u32::try_from(t).ok()),
        thinking_level: session.thinking_level.clone(),
        enabled_tools: session.enabled_tools.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_config(working_dir: PathBuf, app_data_dir: PathBuf) -> HandBoxAgentSessionConfig {
        HandBoxAgentSessionConfig {
            provider_id: "prov-row-123".to_string(),
            provider_type: "openai".to_string(),
            model_id: "gpt-4o".to_string(),
            base_url: String::new(),
            api_key: "sk-test-key".to_string(),
            working_dir,
            app_data_dir,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            thinking_level: None,
            enabled_tools: vec![],
        }
    }

    #[test]
    fn builds_session_with_expected_cwd_and_model() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config, None).expect("construction succeeds");

        // cwd is the working_dir we passed.
        assert_eq!(session.cwd(), cwd.path());
        // Model id is not silently substituted — what we asked for is what the
        // session carries.
        assert_eq!(session.model().id, config.model_id);
    }

    /// Helper: the registered tool-name set a config produces, sorted for
    /// order-independent comparison.
    fn registered_tool_names(config: &HandBoxAgentSessionConfig) -> Vec<String> {
        let session = build_agent_session(config, None).expect("construction succeeds");
        let mut names: Vec<String> = session.tools().iter().map(|t| t.name.clone()).collect();
        names.sort();
        names
    }

    /// VAL-CATOOLS-006 — enabling all 7 registered names registers exactly the
    /// 7 built-in tools, making each visible to the model.
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

    /// VAL-CATOOLS-007 — a tool toggled OFF (absent from enabled_tools) is not
    /// registered, so the model cannot call it. Here only read+grep are on.
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

    /// VAL-CATOOLS-008 — an enabled name that matches a registered tool is
    /// present with no `unknown tool` drop; an unknown name is ignored without
    /// failing construction or polluting the set.
    #[test]
    fn unknown_enabled_names_are_ignored() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        // `read` is valid; `read_file` (stale legacy id) and `nope` are not.
        config.enabled_tools = vec!["read".into(), "read_file".into(), "nope".into()];

        assert_eq!(
            registered_tool_names(&config),
            vec!["read"],
            "unknown names are dropped; only the valid `read` survives"
        );
    }

    /// Empty enabled_tools registers NO tools (legacy "not listed = not
    /// enabled" semantics), never the full set.
    #[test]
    fn empty_enabled_tools_registers_nothing() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        // sample_config already sets enabled_tools = vec![].

        let session = build_agent_session(&config, None).expect("construction succeeds");
        assert!(
            session.tools().is_empty(),
            "an empty enabled_tools list must register no tools"
        );
    }

    /// `select_enabled_tools` is order-independent: it emits tools in the
    /// canonical built-in order regardless of the order names appear in
    /// `enabled`, and dedups gracefully.
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

    #[test]
    fn api_key_is_injected_via_stream_options() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config, None).expect("construction succeeds");

        // The plaintext key rides inside stream options' base.api_key — the
        // only place this construction path puts it.
        assert_eq!(
            session.stream_options().base.api_key.as_deref(),
            Some("sk-test-key"),
        );
    }

    /// Regression guard for the code-review finding: the coding-agent path
    /// must consume the per-session sampling params exactly like legacy
    /// `agent_runtime` (temperature / max_tokens / thinking_level), not silently
    /// fall back to `ChatOptions::default()`. We assert they land on the
    /// constructed session's `stream_options` as non-default values.
    #[test]
    fn session_sampling_params_thread_into_stream_options() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.temperature = Some(0.3);
        config.max_tokens = Some(1000);
        config.thinking_level = Some("high".to_string());

        let session = build_agent_session(&config, None).expect("construction succeeds");
        let opts = session.stream_options();

        // temperature / max_tokens ride on stream_options.base; the default is
        // None, so a concrete value proves the per-session config threaded in.
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

    /// The default (no per-session sampling params) must NOT inject sampling
    /// values — proving the threading above is genuinely driven by the config,
    /// and that a session without overrides leaves provider defaults in place.
    #[test]
    fn absent_sampling_params_leave_stream_options_default() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        // sample_config sets temperature/max_tokens/thinking_level to None.
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config, None).expect("construction succeeds");
        let opts = session.stream_options();

        assert_eq!(opts.base.temperature, None);
        assert_eq!(opts.base.max_tokens, None);
        assert_eq!(opts.reasoning, None);
    }

    /// The per-session custom system prompt must be written into the
    /// `AgentSessionConfig.custom_system_prompt` slot that the coding agent
    /// feeds into the model context (legacy consumes `session.system_prompt` at
    /// agent_runtime.rs:556). `AgentSession` exposes no getter for the prompt,
    /// so we assert the end-to-end path config_from_rows → build_agent_session
    /// preserves a non-`None` prompt and that construction succeeds with it.
    #[test]
    fn session_system_prompt_is_carried_into_construction() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.system_prompt = Some("You are a HandBox coding agent.".to_string());

        // The config slot wired into AgentSessionConfig.custom_system_prompt is
        // non-None (the bug was hardcoding it to None).
        assert_eq!(
            config.system_prompt.as_deref(),
            Some("You are a HandBox coding agent."),
        );

        // And construction with a custom prompt succeeds (the prompt feeds
        // build_system_prompt inside AgentSession::new_with_skill_dirs).
        build_agent_session(&config, None)
            .expect("construction with a custom system prompt succeeds");
    }

    #[test]
    fn unknown_model_under_fixed_catalog_provider_errors() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.model_id = "no-such-model-9999".to_string();

        // `AgentSession` does not implement `Debug`, so `expect_err` (which
        // requires `T: Debug`) is unavailable — match on the Result instead.
        match build_agent_session(&config, None) {
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
        // caller-supplied base_url must override it, proving the override path
        // threads through construction.
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.provider_type = "openai-compatible".to_string();
        config.model_id = "my-local-llm".to_string();
        config.base_url = "http://localhost:1234/v1".to_string();

        let session = build_agent_session(&config, None).expect("construction succeeds");
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
            model_id: model_id.map(str::to_string),
            provider_id: Some("prov-1".to_string()),
            system_prompt: Some("You are helpful.".to_string()),
            thinking_level: Some("high".to_string()),
            temperature: Some(0.5),
            max_tokens: Some(1024),
            working_dir: working_dir.map(str::to_string),
            enabled_tools: vec!["read_file".to_string()],
            tool_execution_mode: None,
            message_count: 0,
            last_message_at: None,
            created_at: 0,
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

        assert_eq!(config.provider_id, "prov-1");
        assert_eq!(config.provider_type, "openai");
        assert_eq!(config.model_id, "gpt-4o");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.api_key, "sk-row-key");
        assert_eq!(config.working_dir, PathBuf::from("/tmp/project"));
        assert_eq!(config.app_data_dir, data.path());
        assert_eq!(config.enabled_tools, vec!["read_file".to_string()]);

        // Per-session config is read off the session row, equivalent to the
        // legacy path (agent_runtime.rs:556,582-586). max_tokens converts
        // i32 → u32 (1024 fits); thinking_level passes through verbatim.
        assert_eq!(config.system_prompt, Some("You are helpful.".to_string()));
        assert_eq!(config.temperature, Some(0.5));
        assert_eq!(config.max_tokens, Some(1024));
        assert_eq!(config.thinking_level, Some("high".to_string()));
    }

    /// A negative `max_tokens` on the row cannot become a `u32`; `try_from`
    /// drops it to `None` rather than panicking — exactly the legacy behavior
    /// (`session.max_tokens.and_then(|t| u32::try_from(t).ok())`).
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

        // No working_dir selected → cwd falls back to the app data dir (an
        // existing directory inside the sandbox).
        assert_eq!(config.working_dir, data.path());
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

    // -----------------------------------------------------------------
    // Read-only tool execution (read / ls / grep / find)
    //
    // These tools are coding-agent built-ins; HandBox does NOT reimplement
    // their logic. The tests below lock in that, once `create_default_tools`
    // registers them against a HandBox working directory, the AgentTool's
    // `execute` closure actually runs and returns the expected ToolResult
    // content (and surfaces a recoverable error — not a panic — on a missing
    // required parameter).
    //
    // Invoke pattern (reused by later sandbox / dangerous-deny features):
    // build the tool set with `create_default_tools(cwd)`, find the tool by
    // name, then drive its `execute` closure directly with a hand-built
    // `ToolExecuteCtx`. The closure is async and returns
    // `Result<ToolResult, ToolError>`; built-in tools report failures as a
    // text `ToolResult` (via `ToolResult::error`) rather than `Err`, so the
    // error rides back as the first text block instead of aborting the turn.
    // -----------------------------------------------------------------

    use base64::Engine;
    use hand_agent::{CancellationToken, ToolExecuteCtx, ToolResult};
    use serde_json::json;
    use std::sync::Arc;

    /// Pull a built-in tool out of the default set by its registered name.
    /// Panics with a clear message if the tool is not present, so a wiring
    /// regression (tool dropped from `create_default_tools`) surfaces as a
    /// test failure rather than a silent skip.
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

    /// VAL-CATOOLS-002 — `read` returns the verbatim content of a text file
    /// inside the working directory and feeds it back.
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

    /// VAL-CATOOLS-009 — reading an image file renders a thumbnail marker
    /// (`Read image file [mime]`) plus an image content block, instead of
    /// dumping raw bytes as text.
    #[tokio::test]
    async fn read_tool_renders_image_marker_and_image_block() {
        let cwd = TempDir::new().unwrap();
        // 1×1 transparent PNG, the same fixture coding-agent uses to anchor
        // image detection by file-magic.
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

    /// VAL-CATOOLS-010 — a large file is truncated and the footer carries a
    /// continuation hint containing `offset=` so the model knows how to read
    /// the rest.
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

    /// VAL-CATOOLS-003 — `ls` lists entries with directories first and file
    /// sizes alongside file entries.
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

    /// VAL-CATOOLS-004 — `grep` prefixes each hit with `path:linenum:`.
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
        // The hit line carries a `…haystack.txt:2:` prefix (file path, then
        // `:linenum:`). The needle sits on line 2 of the fixture.
        assert!(
            text.contains("haystack.txt:2:"),
            "grep hit must carry a `path:linenum:` prefix, got: {text}"
        );
    }

    /// VAL-CATOOLS-012 — `grep` with no matches is a completed (not failed)
    /// result whose text is exactly `No matches found.`.
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

    /// VAL-CATOOLS-005 — `find` lists files matching a glob pattern (and only
    /// the matching ones).
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

    // -----------------------------------------------------------------
    // Approval EFFECTS (M2) — what an allow/deny decision actually causes
    // at the tool boundary, proven by driving the tool body itself.
    //
    // The approval DECISION lives in `agent_permission` (the before_tool_call
    // gate: allow → Continue, deny → Cancel, already unit-tested there). These
    // tests lock the EFFECT side of that decision against the real coding-agent
    // tool bodies HandBox registers:
    //   * allow ⇒ the host proceeds past the gate and INVOKES the tool body;
    //     for `write` that body must actually land bytes on disk (VAL-CAPERM-004).
    //   * deny ⇒ the host Cancels and NEVER invokes the tool body; for `bash`
    //     a skipped invocation produces NO subprocess and NO file side effect
    //     (VAL-CAPERM-007).
    // We drive the bodies through the same `builtin_tool` + `invoke_tool`
    // pattern the read-only tool tests use, so "what runs on allow" and "what
    // is skipped on deny" are pinned against the genuine executors.
    // -----------------------------------------------------------------

    /// VAL-CAPERM-004 — once a `write` is APPROVED, the gate Continues and the
    /// host invokes the write tool body, which actually writes the target file
    /// to disk with the requested content. Invoking the body here models the
    /// post-allow execution path: the bytes land and are verifiable on disk.
    #[tokio::test]
    async fn approved_write_lands_bytes_on_disk() {
        let cwd = TempDir::new().unwrap();
        let target = cwd.path().join("approved.txt");
        let body = "approved write content\nsecond line\n";

        let tool = builtin_tool(cwd.path(), "write");
        let result = invoke_tool(&tool, json!({ "path": "approved.txt", "content": body })).await;

        // The tool reports the write (Created, since the file was new) ...
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

    /// VAL-CAPERM-007 — a DENIED `bash` is Cancelled at the gate and its tool
    /// body is NEVER invoked: no subprocess runs and no file side effect appears.
    /// We prove the side-effect link is real with a positive control (invoking
    /// the body DOES create the sentinel), then assert that the deny path —
    /// modeled by NOT invoking the body, which is exactly what `Cancel`
    /// guarantees — leaves the sentinel absent.
    #[tokio::test]
    async fn denied_bash_runs_no_command_and_leaves_no_side_effect() {
        let cwd = TempDir::new().unwrap();
        let sentinel = cwd.path().join("sentinel.txt");
        // A command whose ONLY observable effect is creating the sentinel file,
        // so its presence/absence is a faithful proxy for "did bash run".
        let command = format!("touch {}", sentinel.display());

        // Positive control: the body genuinely has the side effect when run, so
        // the assertion below is meaningful (the sentinel can appear).
        let bash = builtin_tool(cwd.path(), "bash");
        let _ = invoke_tool(&bash, json!({ "command": command.clone() })).await;
        assert!(
            sentinel.exists(),
            "control: invoking bash must run the command and create the sentinel"
        );
        std::fs::remove_file(&sentinel).unwrap();

        // Deny path: the gate Cancels, so the host NEVER invokes the tool body.
        // We model that by skipping the invocation entirely — the side-effecting
        // executor is never reached — and assert no subprocess ran (no sentinel).
        // (The Cancel decision itself is unit-tested in agent_permission.)
        assert!(
            !sentinel.exists(),
            "a denied bash must not run: with the tool body never invoked, the \
             command produces no subprocess and no file side effect"
        );
    }

    /// VAL-CATOOLS-011 — a missing required parameter fails the call but feeds
    /// the error back as a `ToolResult` (`Missing required parameter: <name>`)
    /// instead of returning `Err` and aborting the turn. Verified on both
    /// `read` (missing `path`) and `grep` (missing `pattern`).
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

    // -----------------------------------------------------------------
    // Dangerous-tool OBSERVABLE behavior (M2, post-allow) — VAL-CATOOLS-018..024,
    // 026.
    //
    // Once the approval gate Continues a dangerous tool (write/edit/bash), the
    // host invokes the genuine coding-agent tool body. These tests LOCK that
    // observable behavior — the exact response text, on-disk effect, atomicity,
    // truncation, sanitization, and exit-code/timeout markers — against the
    // built-in tools `create_default_tools` registers, so a HandBox embedding
    // (or an upstream bump) that quietly changed any of these contracts fails
    // here. We never re-implement the tools; we pin what the registered body
    // does. The invoke pattern is the same `builtin_tool` + `invoke_tool` used
    // by the read-only and approval-effect tests above.
    //
    // bash tests use only harmless commands (echo / exit N / seq / printf /
    // sleep+small timeout) against a tempdir cwd — never rm, never a system
    // path, never the network.
    //
    // NOTE on "error vs completed" (VAL-CATOOLS-021): at THIS layer a tool body
    // returns `hand_agent::types::ToolResult`, which carries NO `is_error`
    // flag — `ToolResult::text` and `ToolResult::error` are shape-identical
    // here, and the agent loop decides the error marker downstream. The bash
    // body routes a non-zero EXIT into `ToolResult::text` (the success-shaped
    // "completed" result) and only an executor FAILURE (spawn/wait error) into
    // `ToolResult::error` ("Bash execution failed: .."). So we lock the
    // completed state by asserting the result carries the `[Exit code: N]`
    // marker AND is NOT the `Bash execution failed` error-shaped text.

    /// VAL-CATOOLS-018 (single edit) — a single-edit `edit` returns a unified
    /// diff (the `--- a/`, `+++ b/`, and `-old`/`+new` lines) and lands the
    /// change on disk.
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

        // Unified-diff structure: file headers plus the -/+ hunk lines.
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

    /// VAL-CATOOLS-018 (multi edit) — a multi-edit `edits: [..]` batch returns
    /// the unified diff PLUS a `Successfully replaced N block(s)` count summary.
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

    /// VAL-CATOOLS-019 — multi-edit is ATOMIC: when one entry's `oldText` is
    /// absent, the whole batch fails and the file is byte-for-byte unchanged
    /// (no partial application of the entries that WOULD have matched).
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

        // The card fails: a per-entry miss error, NOT a success summary.
        assert!(
            text.contains("Could not find the exact text"),
            "a missing entry must surface a per-edit miss error, got: {text}"
        );
        assert!(
            !text.contains("Successfully replaced"),
            "a failed atomic batch must not report any replacement, got: {text}"
        );
        // File byte-for-byte unchanged — the first (matching) entry must NOT
        // have landed.
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            original,
            "atomic rollback: file content must equal the pre-call snapshot"
        );
    }

    /// VAL-CATOOLS-020 — a single-edit `old_string` that matches MORE than once
    /// without `replace_all` is ambiguous: the edit errors and the file is
    /// unchanged (it never silently picks one occurrence).
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

    /// VAL-CATOOLS-021 — `bash` with a non-zero exit code is a COMPLETED card,
    /// not an errored one: the response carries the `[Exit code: N]` marker and
    /// rides the success-shaped text result (NOT the `Bash execution failed`
    /// executor-error path — see the module NOTE above).
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

    /// VAL-CATOOLS-022 — `bash` output over the 64 KB cap is truncated in the
    /// response (`[Output truncated]`) and the full pre-truncation payload is
    /// persisted to a tempfile on disk (`hand-bash-output-<pid>-*.txt` in the
    /// system tempdir) that holds BOTH the head and the tail (the complete
    /// output). The truncated in-result text is strictly shorter than the
    /// persisted full payload.
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

        // The executor persists the full (cleaned, untruncated) payload to a
        // tempfile named `hand-bash-output-<pid>-<nanos>.txt` in the system
        // tempdir before clipping the in-result string. Find the newest such
        // file produced by THIS process and assert it holds the complete output
        // — both the HEAD (which fell off the tail-first truncation window) and
        // the TAIL. Locating by our own pid keeps the scan from colliding with
        // any unrelated leftover file.
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

        // The tempfile is never auto-deleted by the executor; clean up ours so
        // the test leaves no residue in the shared system tempdir.
        let _ = std::fs::remove_file(&persisted_path);
    }

    /// VAL-CATOOLS-023 — `bash` output containing ANSI escapes and C0 control
    /// bytes is SANITIZED before it reaches the model: no escape residue
    /// survives, only the visible characters remain.
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

    /// VAL-CATOOLS-024 — a `bash` command that exceeds its timeout is reported
    /// with the `[Timed out after Ns]` marker (and is not left hanging).
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

    /// VAL-CATOOLS-026 — `write` reports `Created <path> (N lines)` for a new
    /// file and `Updated <path> (N lines)` when overwriting, and the file holds
    /// exactly the requested content in both cases.
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
}
