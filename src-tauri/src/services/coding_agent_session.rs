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
use crate::services::agent_permission::{DangerousDenyExtension, SandboxExtension};
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
/// 2. Build stream options carrying the plaintext api key (no auth.json, no
///    env vars).
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
/// Returns `AppError` when model resolution fails (unknown provider/model) or
/// the coding-agent session cannot be initialized.
pub fn build_agent_session(config: &HandBoxAgentSessionConfig) -> Result<AgentSession, AppError> {
    let model =
        chat_engine::resolve_model(&config.provider_type, &config.model_id, &config.base_url)?;

    // api key flows in via stream options only. ChatOptions defaults are fine:
    // construction does not carry per-turn temperature / max_tokens / signal —
    // those are applied by the drive feature when it actually streams.
    let stream_options: SimpleStreamOptions =
        chat_engine::build_stream_options(&ChatOptions::default(), &config.api_key);

    let tools = select_enabled_tools(&config.working_dir, &config.enabled_tools);

    let session_config = AgentSessionConfig {
        cwd: config.working_dir.clone(),
        model,
        stream_options,
        custom_system_prompt: None,
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

    // M1 deny stub: the dangerous, side-effecting tools (write/edit/bash) have
    // no approval surface yet, so this second before_tool_call extension Cancels
    // every call to them — the tool never runs (no file mutation, no subprocess)
    // and the model gets an error result instead. It composes alongside the
    // sandbox above: the host calls each registered extension in order and the
    // first Cancel wins. M2 REPLACES this stub with an approval extension that
    // awaits a user decision rather than denying unconditionally.
    session.register_extension(Arc::new(DangerousDenyExtension::new()));

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
            enabled_tools: vec![],
        }
    }

    #[test]
    fn builds_session_with_expected_cwd_and_model() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config).expect("construction succeeds");

        // cwd is the working_dir we passed.
        assert_eq!(session.cwd(), cwd.path());
        // Model id is not silently substituted — what we asked for is what the
        // session carries.
        assert_eq!(session.model().id, config.model_id);
    }

    /// Helper: the registered tool-name set a config produces, sorted for
    /// order-independent comparison.
    fn registered_tool_names(config: &HandBoxAgentSessionConfig) -> Vec<String> {
        let session = build_agent_session(config).expect("construction succeeds");
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

        let session = build_agent_session(&config).expect("construction succeeds");
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

        let session = build_agent_session(&config).expect("construction succeeds");

        // The plaintext key rides inside stream options' base.api_key — the
        // only place this construction path puts it.
        assert_eq!(
            session.stream_options().base.api_key.as_deref(),
            Some("sk-test-key"),
        );
    }

    #[test]
    fn unknown_model_under_fixed_catalog_provider_errors() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let mut config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());
        config.model_id = "no-such-model-9999".to_string();

        // `AgentSession` does not implement `Debug`, so `expect_err` (which
        // requires `T: Debug`) is unavailable — match on the Result instead.
        match build_agent_session(&config) {
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

        let session = build_agent_session(&config).expect("construction succeeds");
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
            thinking_level: None,
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
}
