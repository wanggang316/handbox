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

use std::path::PathBuf;

use hand_ai_model::SimpleStreamOptions;
use hand_coding_agent::tools::create_default_tools;
use hand_coding_agent::{AgentSession, AgentSessionConfig};

use crate::models::AppError;
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
    /// HandBox's per-session enabled-tool list. Carried for forward
    /// compatibility with the tool-registration feature; this construction
    /// feature registers the full built-in set (see [`build_agent_session`]).
    pub enabled_tools: Vec<String>,
}

/// Construct a coding-agent [`AgentSession`] from a HandBox configuration.
///
/// Steps:
/// 1. Resolve the model through `chat_engine` (no silent substitution — the
///    returned `model.id` equals the requested `model_id`).
/// 2. Build stream options carrying the plaintext api key (no auth.json, no
///    env vars).
/// 3. Register the full built-in tool set against `working_dir`.
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

    let tools = create_default_tools(&config.working_dir);

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

    AgentSession::new_with_skill_dirs(session_config, tools, None, None)
        .map_err(|e| AppError::internal_error(&format!("failed to construct agent session: {e}")))
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

    #[test]
    fn registers_full_builtin_tool_set() {
        let cwd = TempDir::new().unwrap();
        let data = TempDir::new().unwrap();
        let config = sample_config(cwd.path().to_path_buf(), data.path().to_path_buf());

        let session = build_agent_session(&config).expect("construction succeeds");

        // create_default_tools registers the 7 built-in tools.
        assert_eq!(
            session.tools().len(),
            7,
            "full built-in tool set must be registered"
        );
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
}
