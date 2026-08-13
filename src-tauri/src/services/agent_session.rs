// Agent-mode session CRUD service on top of `AgentSessionRepository`, independent
// of the chat-mode `SessionService` and the preset `AgentService`. Only session
// CRUD and transcript reads live here; runtime/run/streaming/tools live elsewhere.

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{
    Agent, AgentSession, AgentSessionMessage, CreateAgentSessionRequest,
    InstantiateAgentSessionRequest, McpServerConfig, UUID,
};
use crate::storage::{AgentProjectRepository, AgentRepository, AgentSessionRepository};
use std::sync::Arc;

/// Updatable session fields (mirrors `AgentParameter`, one field per update).
pub enum AgentSessionParameter {
    Name(String),
    ModelId(Option<String>),
    ProviderId(Option<String>),
    SystemPrompt(Option<String>),
    ThinkingLevel(Option<String>),
    Temperature(Option<f32>),
    MaxTokens(Option<i32>),
    WorkingDir(Option<String>),
    EnabledTools(Vec<String>),
    McpServers(Vec<McpServerConfig>),
    ToolExecutionMode(Option<String>),
}

#[derive(Clone)]
pub struct AgentSessionService {
    repository: AgentSessionRepository,
    /// Repository rather than `AgentProjectService`: create only resolves one
    /// project row by id, keeping the dependency light and avoiding service coupling.
    projects: AgentProjectRepository,
    /// Repository rather than `AgentService`: instantiation only resolves one
    /// definition row by id.
    definitions: AgentRepository,
}

impl AgentSessionService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: AgentSessionRepository::new(Arc::clone(&db)),
            projects: AgentProjectRepository::new(Arc::clone(&db)),
            definitions: AgentRepository::new(db),
        }
    }

    /// Project attach wins: resolve `project_id` (empty = unset; `NOT_FOUND` if
    /// missing), require its path to still canonicalize to itself as a directory,
    /// and copy it into `working_dir`, overriding the request; otherwise validate
    /// `working_dir` directly. Any validation failure writes no row.
    pub async fn create_session(
        &self,
        request: CreateAgentSessionRequest,
    ) -> Result<AgentSession, AppError> {
        let (project_id, working_dir) = self
            .resolve_project_and_working_dir(request.project_id.clone(), request.working_dir.clone())
            .await?;

        let now = Self::current_timestamp();
        let session = AgentSession {
            id: uuid::Uuid::new_v4().to_string(),
            name: request.name,
            project_id,
            agent_definition_id: request.agent_definition_id,
            model_id: request.model_id,
            provider_id: request.provider_id,
            system_prompt: request.system_prompt,
            thinking_level: request.thinking_level,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            working_dir,
            enabled_tools: request.enabled_tools.unwrap_or_default(),
            mcp_servers: request.mcp_servers.unwrap_or_default(),
            tool_execution_mode: request.tool_execution_mode,
            message_count: 0,
            last_message_at: None,
            pinned: false,
            archived: false,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_session(&session).await?;
        Ok(session)
    }

    /// Instantiate a session from an AgentDefinition: snapshot its capability set
    /// (`enabled_tools` ← `builtin_tools`) and defaults, apply `overrides`
    /// (name/model/provider win), let `working_dir_mode` arbitrate the working dir,
    /// then delegate to [`create_session`] with an `agent_definition_id` back-link.
    pub async fn create_session_from_definition(
        &self,
        definition_id: UUID,
        overrides: InstantiateAgentSessionRequest,
    ) -> Result<AgentSession, AppError> {
        let definition = self
            .definitions
            .get_agent_by_id(&definition_id)
            .await?
            .ok_or_else(|| {
                AppError::not_found(&format!("Agent definition not found: {}", definition_id))
            })?;

        let request = Self::build_instantiation_request(&definition, overrides)?;
        self.create_session(request).await
    }

    /// Re-point an existing session at another AgentDefinition in place, without
    /// creating a row. Callers only take this path while the session has no
    /// messages, so id / created_at / transcript are preserved while the
    /// capability set, defaults and `agent_definition_id` are re-snapshotted —
    /// the only sanctioned rewrite of that provenance link outside create.
    ///
    /// Model/provider fall back to the session's current values when the new
    /// definition pins none; the working dir is inherited and then arbitrated by
    /// the new definition's `working_dir_mode`.
    pub async fn reinstantiate_from_definition(
        &self,
        session_id: UUID,
        definition_id: UUID,
        overrides: InstantiateAgentSessionRequest,
    ) -> Result<AgentSession, AppError> {
        let mut session = self.get_session(session_id).await?;

        let definition = self
            .definitions
            .get_agent_by_id(&definition_id)
            .await?
            .ok_or_else(|| {
                AppError::not_found(&format!("Agent definition not found: {}", definition_id))
            })?;

        // Absent an explicit override, inherit the session's current attachment;
        // build_instantiation_request then applies the new working_dir_mode.
        let overrides = InstantiateAgentSessionRequest {
            project_id: overrides.project_id.or_else(|| session.project_id.clone()),
            working_dir: overrides.working_dir.or_else(|| session.working_dir.clone()),
            ..overrides
        };
        let request = Self::build_instantiation_request(&definition, overrides)?;
        let (project_id, working_dir) = self
            .resolve_project_and_working_dir(request.project_id, request.working_dir)
            .await?;

        session.agent_definition_id = Some(definition.id.clone());
        session.name = request.name;
        session.project_id = project_id;
        session.working_dir = working_dir;
        // Keep the session's model/provider when the definition pins none. Those
        // columns can be "" rather than NULL, so an empty string must count as
        // unset or a real selected model would be overwritten with nothing.
        let non_empty = |v: Option<String>| v.filter(|s| !s.is_empty());
        session.model_id = non_empty(request.model_id).or(session.model_id);
        session.provider_id = non_empty(request.provider_id).or(session.provider_id);
        session.system_prompt = request.system_prompt;
        session.thinking_level = request.thinking_level;
        session.temperature = request.temperature;
        session.max_tokens = request.max_tokens;
        session.enabled_tools = request.enabled_tools.unwrap_or_default();
        session.mcp_servers = request.mcp_servers.unwrap_or_default();
        session.tool_execution_mode = request.tool_execution_mode;
        session.updated_at = Self::current_timestamp();

        self.repository.reinstantiate_session(&session).await?;
        Ok(session)
    }

    /// Assemble a `CreateAgentSessionRequest` from a definition plus overrides.
    ///
    /// `working_dir_mode` arbitrates the directory: `"none"` forces null,
    /// `"required"` demands a project or working_dir (else `VALIDATION_ERROR`),
    /// `"optional"`/NULL passes through. Shared by create and reinstantiate so
    /// the two paths cannot drift.
    fn build_instantiation_request(
        definition: &Agent,
        overrides: InstantiateAgentSessionRequest,
    ) -> Result<CreateAgentSessionRequest, AppError> {
        // An empty string counts as "not provided", matching create's validation.
        let is_set = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
        let (project_id, working_dir) =
            match definition.working_dir_mode.as_deref().unwrap_or("optional") {
                "none" => (None, None),
                "required" => {
                    if !is_set(&overrides.project_id) && !is_set(&overrides.working_dir) {
                        return Err(AppError::with_hint(
                            "VALIDATION_ERROR",
                            "this agent definition requires a working directory",
                            "该 Agent 需要选择工作目录或项目后才能创建会话",
                        ));
                    }
                    (overrides.project_id, overrides.working_dir)
                }
                _ => (overrides.project_id, overrides.working_dir),
            };

        let name = overrides
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| definition.name.clone());

        Ok(CreateAgentSessionRequest {
            name,
            project_id,
            agent_definition_id: Some(definition.id.clone()),
            // Model is decoupled from the definition: it comes only from the
            // overrides (picked in the UI). Provider may still take a default.
            model_id: overrides.model_id,
            provider_id: overrides
                .provider_id
                .or_else(|| definition.provider_id.clone()),
            system_prompt: definition.system_prompt.clone(),
            thinking_level: definition.thinking_level.clone(),
            temperature: definition.temperature,
            max_tokens: definition.max_tokens,
            working_dir,
            enabled_tools: Some(definition.builtin_tools.clone()),
            mcp_servers: Some(definition.mcp_servers.clone()),
            tool_execution_mode: definition.tool_execution_mode.clone(),
        })
    }

    /// Resolve `(project_id, working_dir)` in canonical form. A project wins: it
    /// is looked up by id and its path re-checked as a canonical directory;
    /// otherwise `working_dir` is validated directly. Empty string / None mean
    /// unset. Shared by create and reinstantiate so the two cannot drift.
    async fn resolve_project_and_working_dir(
        &self,
        project_id: Option<String>,
        working_dir: Option<String>,
    ) -> Result<(Option<String>, Option<String>), AppError> {
        let requested_project_id = project_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(str::to_owned);

        match requested_project_id {
            Some(pid) => {
                let project = self
                    .projects
                    .get_project_by_id(&pid)
                    .await?
                    .ok_or_else(|| {
                        AppError::not_found(&format!("Agent project not found: {}", pid))
                    })?;

                // project.path was canonical when stored; re-check that it still
                // canonicalizes to itself and is a directory — it may have been
                // deleted or replaced by a symlink pointing elsewhere.
                let still_canonical = std::fs::canonicalize(&project.path)
                    .map(|c| c == std::path::Path::new(&project.path) && c.is_dir())
                    .unwrap_or(false);
                if !still_canonical {
                    return Err(AppError::with_hint(
                        "VALIDATION_ERROR",
                        &format!(
                            "project path is no longer a canonical existing directory: {}",
                            project.path
                        ),
                        "项目目录已不存在或已被替换，请重新选择项目",
                    ));
                }

                Ok((Some(project.id), Some(project.path)))
            }
            None => Ok((None, Self::validate_working_dir(working_dir.as_deref())?)),
        }
    }

    /// Sessions ordered by updated_at desc. Omitting `limit` returns everything:
    /// the sidebar groups the full list by project, so the default must never
    /// truncate silently (`i32::MAX` is effectively unbounded for SQLite).
    pub async fn list_sessions(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<AgentSession>, AppError> {
        let limit = limit.unwrap_or(i32::MAX);
        let offset = offset.unwrap_or(0);
        self.repository.list_sessions(limit, offset).await
    }

    pub async fn get_session(&self, session_id: UUID) -> Result<AgentSession, AppError> {
        match self.repository.get_session_by_id(&session_id).await? {
            Some(session) => Ok(session),
            None => Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            ))),
        }
    }

    pub async fn rename_session(
        &self,
        session_id: UUID,
        name: String,
    ) -> Result<AgentSession, AppError> {
        self.repository.rename_session(&session_id, &name).await?;
        self.get_session(session_id).await
    }

    /// Toggles the sidebar pin. Kept off `update_session_field` on purpose: the
    /// flag has no place in the generic read-modify-write path (see
    /// [`AgentSessionRepository::set_session_pinned`]).
    pub async fn set_session_pinned(
        &self,
        session_id: UUID,
        pinned: bool,
    ) -> Result<AgentSession, AppError> {
        self.repository
            .set_session_pinned(&session_id, pinned)
            .await?;
        self.get_session(session_id).await
    }

    /// Toggles the archive flag; same rationale as [`set_session_pinned`].
    pub async fn set_session_archived(
        &self,
        session_id: UUID,
        archived: bool,
    ) -> Result<AgentSession, AppError> {
        self.repository
            .set_session_archived(&session_id, archived)
            .await?;
        self.get_session(session_id).await
    }

    /// Single entry point for updating one session field (mirrors
    /// `agent_update_field`).
    pub async fn update_session_field(
        &self,
        session_id: UUID,
        parameter: AgentSessionParameter,
    ) -> Result<AgentSession, AppError> {
        let mut session = self.get_session(session_id).await?;

        match parameter {
            AgentSessionParameter::Name(name) => session.name = name,
            AgentSessionParameter::ModelId(model_id) => session.model_id = model_id,
            AgentSessionParameter::ProviderId(provider_id) => session.provider_id = provider_id,
            AgentSessionParameter::SystemPrompt(prompt) => session.system_prompt = prompt,
            AgentSessionParameter::ThinkingLevel(level) => session.thinking_level = level,
            AgentSessionParameter::Temperature(temp) => session.temperature = temp,
            AgentSessionParameter::MaxTokens(max_tokens) => session.max_tokens = max_tokens,
            AgentSessionParameter::WorkingDir(working_dir) => {
                // Same validation as create, so storage always holds a canonical
                // absolute directory or null.
                session.working_dir = Self::validate_working_dir(working_dir.as_deref())?;
            }
            AgentSessionParameter::EnabledTools(tools) => session.enabled_tools = tools,
            AgentSessionParameter::McpServers(servers) => session.mcp_servers = servers,
            AgentSessionParameter::ToolExecutionMode(mode) => session.tool_execution_mode = mode,
        }

        session.updated_at = Self::current_timestamp();
        self.repository.update_session(&session).await?;
        Ok(session)
    }

    /// Deletes the session; the repository cascades its transcript.
    pub async fn delete_session(&self, session_id: UUID) -> Result<(), AppError> {
        self.repository.delete_session(&session_id).await
    }

    /// Full transcript of a session, ordered by seq.
    pub async fn list_messages(
        &self,
        session_id: UUID,
    ) -> Result<Vec<AgentSessionMessage>, AppError> {
        self.repository.list_messages(&session_id).await
    }

    /// Validate and canonicalize `working_dir`. None / empty store as null;
    /// relative, missing, or non-directory paths are rejected; anything else
    /// yields its canonical absolute path.
    fn validate_working_dir(working_dir: Option<&str>) -> Result<Option<String>, AppError> {
        let raw = match working_dir {
            None | Some("") => return Ok(None),
            Some(s) => s,
        };

        let path = std::path::Path::new(raw);

        // Relative paths are rejected even when they resolve against cwd, so the
        // stored path stays deterministic.
        if !path.is_absolute() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("working_dir must be an absolute path: {}", raw),
                "请提供一个已存在目录的绝对路径",
            ));
        }

        // canonicalize resolves symlinks and requires existence; a failure here
        // means the path is not there.
        let canonical = std::fs::canonicalize(path).map_err(|_| {
            AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("working_dir does not exist: {}", raw),
                "请提供一个已存在目录的绝对路径",
            )
        })?;

        if !canonical.is_dir() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("working_dir is not a directory: {}", raw),
                "working_dir 必须指向一个目录而非文件",
            ));
        }

        Ok(Some(canonical.to_string_lossy().into_owned()))
    }

    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use sqlx::Row;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Test database; the returned TempDir must outlive it.
    async fn create_test_database() -> (Arc<Database>, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(
            Database::new(&db_path)
                .await
                .expect("Failed to create database"),
        );
        (db, temp_dir)
    }

    fn base_request(name: &str) -> CreateAgentSessionRequest {
        CreateAgentSessionRequest {
            name: name.to_string(),
            project_id: None,
            agent_definition_id: None,
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: None,
            thinking_level: None,
            temperature: None,
            max_tokens: None,
            working_dir: None,
            enabled_tools: None,
            mcp_servers: None,
            tool_execution_mode: None,
        }
    }

    async fn count_rows(db: &Database, table: &str) -> i64 {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) AS count FROM {}",
            table
        )))
            .fetch_one(db.pool())
            .await
            .unwrap();
        row.try_get::<i64, _>("count").unwrap()
    }

    /// builtin-coding instantiates with the full seven-tool capability set
    /// snapshotted into `enabled_tools`, its manual tool-execution policy, the
    /// provided working dir, and an `agent_definition_id` back-link.
    #[tokio::test]
    async fn from_definition_builtin_coding_snapshots_capability_set() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let work_dir = TempDir::new().unwrap();
        let canonical = std::fs::canonicalize(work_dir.path())
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let overrides = InstantiateAgentSessionRequest {
            working_dir: Some(work_dir.path().to_string_lossy().into_owned()),
            provider_id: Some("openai".to_string()),
            model_id: Some("gpt-4o".to_string()),
            ..Default::default()
        };
        let session = service
            .create_session_from_definition("builtin-coding".to_string(), overrides)
            .await
            .expect("instantiate builtin-coding");

        // The 7 coding built-ins plus `ask_question`, which migration 066 grants
        // to both builtin definitions so a coding agent can also ask before it acts.
        assert_eq!(
            session.enabled_tools,
            vec!["read", "write", "edit", "bash", "grep", "find", "ls", "ask_question"]
        );
        assert_eq!(session.tool_execution_mode.as_deref(), Some("manual"));
        assert_eq!(session.working_dir, Some(canonical));
        assert_eq!(
            session.agent_definition_id.as_deref(),
            Some("builtin-coding")
        );
        // name defaults to the definition name when no override is given.
        assert_eq!(session.name, "Coding");
    }

    /// builtin-coding is `working_dir_mode: "required"`: instantiating it without
    /// a working dir (and without a project) is a VALIDATION_ERROR and writes no row.
    #[tokio::test]
    async fn from_definition_required_mode_without_dir_is_rejected() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let err = service
            .create_session_from_definition(
                "builtin-coding".to_string(),
                InstantiateAgentSessionRequest::default(),
            )
            .await
            .expect_err("required mode must reject a missing working dir");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    /// builtin-chat is `working_dir_mode: "none"`: it degrades to pure dialog —
    /// zero builtin tools, and any supplied working dir is IGNORED (forced null).
    #[tokio::test]
    async fn from_definition_builtin_chat_is_pure_dialog() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        // Even a perfectly valid working dir is dropped for a "none"-mode definition.
        let work_dir = TempDir::new().unwrap();
        let overrides = InstantiateAgentSessionRequest {
            working_dir: Some(work_dir.path().to_string_lossy().into_owned()),
            provider_id: Some("openai".to_string()),
            model_id: Some("gpt-4o".to_string()),
            ..Default::default()
        };
        let session = service
            .create_session_from_definition("builtin-chat".to_string(), overrides)
            .await
            .expect("instantiate builtin-chat");

        // Chat registers no FILE/SHELL tools; `ask_question` is the one exception
        // (migration 066) — it needs no working dir and is what lets a plain chat
        // ask the user before guessing.
        assert_eq!(
            session.enabled_tools,
            vec!["ask_question"],
            "chat-class registers no builtin tools beyond ask_question"
        );
        assert_eq!(
            session.working_dir, None,
            "none-mode definition must ignore any provided working dir"
        );
        assert_eq!(session.agent_definition_id.as_deref(), Some("builtin-chat"));
    }

    /// `overrides` win over the definition defaults for the fields the UI fills in
    /// at instantiation: name, model, provider.
    #[tokio::test]
    async fn from_definition_overrides_take_precedence() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let overrides = InstantiateAgentSessionRequest {
            name: Some("My Chat".to_string()),
            model_id: Some("claude-opus-4-8".to_string()),
            provider_id: Some("anthropic".to_string()),
            ..Default::default()
        };
        let session = service
            .create_session_from_definition("builtin-chat".to_string(), overrides)
            .await
            .expect("instantiate with overrides");

        assert_eq!(session.name, "My Chat");
        assert_eq!(session.model_id.as_deref(), Some("claude-opus-4-8"));
        assert_eq!(session.provider_id.as_deref(), Some("anthropic"));
    }

    /// An unknown definition id is a NOT_FOUND, not a silent empty session.
    #[tokio::test]
    async fn from_definition_unknown_id_errors() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let err = service
            .create_session_from_definition(
                "does-not-exist".to_string(),
                InstantiateAgentSessionRequest::default(),
            )
            .await
            .expect_err("unknown definition must error");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    /// Re-pointing an existing session to another definition mutates it in place:
    /// same id, no new row, provenance + capability set re-snapshotted from the new
    /// definition. coding -> chat drops the seven tools and the working dir (chat is
    /// none-mode) while preserving the session's model/provider (chat pins none) —
    /// proving the row was reused, not recreated.
    #[tokio::test]
    async fn reinstantiate_repoints_in_place_from_coding_to_chat() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let work_dir = TempDir::new().unwrap();
        let coding = service
            .create_session_from_definition(
                "builtin-coding".to_string(),
                InstantiateAgentSessionRequest {
                    working_dir: Some(work_dir.path().to_string_lossy().into_owned()),
                    provider_id: Some("openai".to_string()),
                    model_id: Some("gpt-4o".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("instantiate builtin-coding");
        let session_id = coding.id.clone();

        let chat = service
            .reinstantiate_from_definition(
                session_id.clone(),
                "builtin-chat".to_string(),
                InstantiateAgentSessionRequest::default(),
            )
            .await
            .expect("reinstantiate to builtin-chat");

        // Same row, re-pointed — not a fresh session.
        assert_eq!(chat.id, session_id, "session id is preserved (in place)");
        assert_eq!(count_rows(&db, "agent_sessions").await, 1, "no new row");
        assert_eq!(chat.agent_definition_id.as_deref(), Some("builtin-chat"));
        // Capability set re-snapshotted from the new (chat) definition.
        assert_eq!(
            chat.enabled_tools,
            vec!["ask_question"],
            "chat clears the coding built-ins, keeping only ask_question"
        );
        assert_eq!(chat.working_dir, None, "none-mode chat drops working dir");
        assert_eq!(chat.name, "通用对话", "name adopts the new definition");
        // Model/provider preserved: builtin-chat pins none, so the session keeps its own.
        assert_eq!(chat.model_id.as_deref(), Some("gpt-4o"));
        assert_eq!(chat.provider_id.as_deref(), Some("openai"));

        // The persisted row matches the returned session (read-back).
        let reread = service.get_session(session_id).await.expect("get session");
        assert_eq!(reread.agent_definition_id.as_deref(), Some("builtin-chat"));
        assert_eq!(reread.enabled_tools, vec!["ask_question"]);
    }

    /// An unknown definition id on reinstantiate is a NOT_FOUND and leaves the
    /// existing session untouched (no partial re-point).
    #[tokio::test]
    async fn reinstantiate_unknown_definition_errors_and_preserves_session() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let session = service
            .create_session_from_definition(
                "builtin-chat".to_string(),
                InstantiateAgentSessionRequest {
                    provider_id: Some("openai".to_string()),
                    model_id: Some("gpt-4o".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("instantiate builtin-chat");

        let err = service
            .reinstantiate_from_definition(
                session.id.clone(),
                "does-not-exist".to_string(),
                InstantiateAgentSessionRequest::default(),
            )
            .await
            .expect_err("unknown definition must error");
        assert_eq!(err.code, "NOT_FOUND");

        let reread = service.get_session(session.id).await.expect("get session");
        assert_eq!(reread.agent_definition_id.as_deref(), Some("builtin-chat"));
        assert_eq!(count_rows(&db, "agent_sessions").await, 1);
    }

    /// Reinstantiating a session that doesn't exist is a NOT_FOUND — no ghost row.
    #[tokio::test]
    async fn reinstantiate_unknown_session_errors() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let err = service
            .reinstantiate_from_definition(
                "no-such-session".to_string(),
                "builtin-chat".to_string(),
                InstantiateAgentSessionRequest::default(),
            )
            .await
            .expect_err("unknown session must error");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    #[tokio::test]
    async fn create_session_accepts_existing_absolute_dir_and_stores_canonical() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        // A real, existing directory.
        let work_dir = TempDir::new().unwrap();
        let raw = work_dir.path().to_string_lossy().into_owned();
        let expected_canonical = std::fs::canonicalize(work_dir.path())
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let mut req = base_request("With WorkingDir");
        req.working_dir = Some(raw);

        let created = service.create_session(req).await.expect("create failed");
        assert_eq!(created.working_dir, Some(expected_canonical.clone()));

        // Persisted canonical path round-trips.
        let fetched = service.get_session(created.id.clone()).await.unwrap();
        assert_eq!(fetched.working_dir, Some(expected_canonical));
    }

    #[tokio::test]
    async fn create_session_resolves_symlink_to_dir_to_canonical_target() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        // Real target dir + a symlink pointing at it.
        let target = TempDir::new().unwrap();
        let link_parent = TempDir::new().unwrap();
        let link = link_parent.path().join("link-to-dir");

        #[cfg(unix)]
        std::os::unix::fs::symlink(target.path(), &link).unwrap();
        #[cfg(not(unix))]
        return; // symlink semantics differ; covered on unix CI

        let canonical_target = std::fs::canonicalize(target.path())
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let mut req = base_request("Symlink WorkingDir");
        req.working_dir = Some(link.to_string_lossy().into_owned());

        let created = service.create_session(req).await.expect("create failed");
        assert_eq!(created.working_dir, Some(canonical_target));
    }

    #[tokio::test]
    async fn create_session_rejects_missing_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let mut req = base_request("Missing Dir");
        req.working_dir = Some("/this/path/should/not/exist/handbox-xyz".to_string());

        let err = service
            .create_session(req)
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    #[tokio::test]
    async fn create_session_rejects_relative_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let mut req = base_request("Relative Dir");
        // A relative path that may well exist relative to cwd, yet must be rejected.
        req.working_dir = Some("src".to_string());

        let err = service
            .create_session(req)
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    #[tokio::test]
    async fn create_session_rejects_file_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        // An existing FILE (not a dir).
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("a-file.txt");
        std::fs::write(&file_path, b"hello").unwrap();

        let mut req = base_request("File Dir");
        req.working_dir = Some(file_path.to_string_lossy().into_owned());

        let err = service
            .create_session(req)
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    #[tokio::test]
    async fn create_session_allows_empty_and_none_working_dir_as_null() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        // None
        let created_none = service
            .create_session(base_request("No WorkingDir"))
            .await
            .expect("create failed");
        assert_eq!(created_none.working_dir, None);

        // Empty string -> stored as null
        let mut req_empty = base_request("Empty WorkingDir");
        req_empty.working_dir = Some(String::new());
        let created_empty = service
            .create_session(req_empty)
            .await
            .expect("create failed");
        assert_eq!(created_empty.working_dir, None);
    }

    #[tokio::test]
    async fn create_session_with_project_copies_path_and_overrides_working_dir() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());
        let projects = crate::services::AgentProjectService::new(db);

        let project_dir = TempDir::new().unwrap();
        let project = projects
            .create_project(project_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        // The request also carries a DIFFERENT (valid) working_dir: the project
        // path must win.
        let other_dir = TempDir::new().unwrap();
        let mut req = base_request("Attached Session");
        req.project_id = Some(project.id.clone());
        req.working_dir = Some(other_dir.path().to_string_lossy().into_owned());

        let created = service.create_session(req).await.expect("create failed");
        assert_eq!(created.project_id, Some(project.id.clone()));
        assert_eq!(created.working_dir, Some(project.path.clone()));

        // Round-trip via get and list: projectId survives persistence.
        let fetched = service.get_session(created.id.clone()).await.unwrap();
        assert_eq!(fetched.project_id, Some(project.id.clone()));
        assert_eq!(fetched.working_dir, Some(project.path));

        let listed = service.list_sessions(None, None).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].project_id, Some(project.id.clone()));

        // Wire shape for the sidebar consumer: camelCase `projectId`.
        let json = serde_json::to_string(&listed[0]).unwrap();
        assert!(json.contains(&format!("\"projectId\":\"{}\"", project.id)));
    }

    #[tokio::test]
    async fn create_session_with_project_skips_invalid_working_dir_entirely() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());
        let projects = crate::services::AgentProjectService::new(db);

        let project_dir = TempDir::new().unwrap();
        let project = projects
            .create_project(project_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        // A working_dir that would be REJECTED on its own (relative garbage):
        // with a project attached it is skipped entirely — never validated —
        // and the stored working_dir is the project path.
        let mut req = base_request("Project Beats Garbage WorkingDir");
        req.project_id = Some(project.id.clone());
        req.working_dir = Some("relative/garbage".to_string());

        let created = service.create_session(req).await.expect("create failed");
        assert_eq!(created.project_id, Some(project.id));
        assert_eq!(created.working_dir, Some(project.path));
    }

    /// Two sessions whose working directories canonicalize to the same path land
    /// in one project: get-or-create keys off the canonical path, so a symlink
    /// alias returns the same `project_id` the sidebar groups by.
    #[tokio::test]
    async fn sessions_in_same_canonical_dir_share_one_project_id() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());
        let projects = crate::services::AgentProjectService::new(db.clone());

        // One real directory, reachable two ways: directly and via a symlink.
        let target = TempDir::new().unwrap();
        let link_parent = TempDir::new().unwrap();
        let link = link_parent.path().join("alias");

        #[cfg(unix)]
        std::os::unix::fs::symlink(target.path(), &link).unwrap();
        #[cfg(not(unix))]
        return; // symlink semantics differ; covered on unix CI

        // get-or-create by canonical path: the direct path and the symlink alias
        // both resolve to one project row.
        let p_direct = projects
            .create_project(target.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        let p_alias = projects
            .create_project(link.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(
            p_alias.id, p_direct.id,
            "the canonical-path get-or-create must collapse both aliases to one project"
        );

        // A session created against each project id lands in the same group.
        let mut req1 = base_request("Via Direct");
        req1.project_id = Some(p_direct.id.clone());
        let s1 = service.create_session(req1).await.unwrap();

        let mut req2 = base_request("Via Alias");
        req2.project_id = Some(p_alias.id.clone());
        let s2 = service.create_session(req2).await.unwrap();

        assert_eq!(
            s1.project_id, s2.project_id,
            "two sessions in the same canonical dir must share one project_id (one group)"
        );
        assert_eq!(s1.project_id, Some(p_direct.id));
        // Exactly one project row backs the group — no per-alias duplication.
        assert_eq!(count_rows(&db, "agent_projects").await, 1);
    }

    /// Three byte-different but user-equivalent cwd forms of one directory —
    /// plain path, trailing slash, symlink alias — all collapse into a single
    /// project bucket, because `validate_project_path` canonicalizes first.
    #[tokio::test]
    async fn cwd_trailing_slash_and_symlink_forms_share_one_project_bucket() {
        let (db, _guard) = create_test_database().await;
        let projects = crate::services::AgentProjectService::new(db.clone());

        // One real directory. Canonicalize once so the trailing-slash string is
        // built from the post-canonical path (the writer canonicalizes too, so
        // both the plain and trailing-slash forms must land the same row).
        let target = TempDir::new().unwrap();
        let canonical = std::fs::canonicalize(target.path()).unwrap();

        // Form 1: plain canonical path, no trailing separator.
        let plain = canonical.to_string_lossy().into_owned();
        // Form 2: same path with a trailing slash appended.
        let trailing = format!("{}{}", plain, std::path::MAIN_SEPARATOR);
        assert_ne!(
            plain, trailing,
            "the two forms must differ byte-wise, else the test proves nothing"
        );

        let p_plain = projects.create_project(plain.clone()).await.unwrap();
        let p_trailing = projects.create_project(trailing).await.unwrap();
        assert_eq!(
            p_trailing.id, p_plain.id,
            "a trailing slash must canonicalize away → the same project row, not a second"
        );

        // Form 3: a symlink alias to the same directory resolves to the same row.
        let link_parent = TempDir::new().unwrap();
        let link = link_parent.path().join("alias");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target.path(), &link).unwrap();
            let p_link = projects
                .create_project(link.to_string_lossy().into_owned())
                .await
                .unwrap();
            assert_eq!(
                p_link.id, p_plain.id,
                "a symlink alias must canonicalize to the same project row"
            );
        }

        // Exactly one project row backs all the equivalent forms — one bucket.
        assert_eq!(
            count_rows(&db, "agent_projects").await,
            1,
            "all user-equivalent cwd forms must collapse to a single project row"
        );
    }

    #[tokio::test]
    async fn create_session_with_unknown_project_returns_not_found_and_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let mut req = base_request("Ghost Project");
        req.project_id = Some("nonexistent-project".to_string());

        let err = service
            .create_session(req)
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    #[tokio::test]
    async fn create_session_with_deleted_project_dir_rejects_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());
        let projects = crate::services::AgentProjectService::new(db.clone());

        // Create the project while the directory exists, then delete the
        // directory from disk before attaching a session.
        let project_dir = TempDir::new().unwrap();
        let project = projects
            .create_project(project_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        drop(project_dir); // removes the directory from disk

        let mut req = base_request("Stale Project Dir");
        req.project_id = Some(project.id);

        let err = service
            .create_session(req)
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
    }

    #[tokio::test]
    async fn create_session_empty_project_id_treated_as_unset() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let mut req = base_request("Empty ProjectId");
        req.project_id = Some(String::new());

        let created = service.create_session(req).await.expect("create failed");
        assert_eq!(created.project_id, None);
        assert_eq!(created.working_dir, None);
    }

    #[tokio::test]
    async fn list_sessions_default_limit_does_not_truncate() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        // Comfortably more than any plausible page-size default.
        let total = 60;
        for i in 0..total {
            service
                .create_session(base_request(&format!("Session {}", i)))
                .await
                .unwrap();
        }

        let listed = service.list_sessions(None, None).await.unwrap();
        assert_eq!(listed.len(), total, "default list must return all sessions");
    }

    #[tokio::test]
    async fn service_crud_roundtrip() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let created = service
            .create_session(base_request("Roundtrip"))
            .await
            .unwrap();

        let listed = service.list_sessions(Some(10), Some(0)).await.unwrap();
        assert_eq!(listed.len(), 1);

        let got = service.get_session(created.id.clone()).await.unwrap();
        assert_eq!(got.name, "Roundtrip");

        let renamed = service
            .rename_session(created.id.clone(), "Renamed".to_string())
            .await
            .unwrap();
        assert_eq!(renamed.name, "Renamed");

        let updated = service
            .update_session_field(
                created.id.clone(),
                AgentSessionParameter::ThinkingLevel(Some("high".to_string())),
            )
            .await
            .unwrap();
        assert_eq!(updated.thinking_level, Some("high".to_string()));

        let msgs = service.list_messages(created.id.clone()).await.unwrap();
        assert!(msgs.is_empty());

        service.delete_session(created.id.clone()).await.unwrap();
        let err = service.get_session(created.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
    }

    /// A create request still carrying the deprecated enabledSkills key succeeds
    /// (serde ignores unknown keys) and leaves the dead column NULL.
    #[tokio::test]
    async fn create_with_deprecated_enabled_skills_key_succeeds_and_column_stays_null() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db.clone());

        let req: CreateAgentSessionRequest =
            serde_json::from_str(r#"{"name": "Deprecated Key", "enabledSkills": ["pdf"]}"#)
                .expect("unknown enabledSkills key must be ignored by serde");
        let created = service.create_session(req).await.unwrap();
        assert_eq!(created.name, "Deprecated Key");

        let column: Option<String> =
            sqlx::query("SELECT enabled_skills FROM agent_sessions WHERE id = $1")
                .bind(&created.id)
                .fetch_one(db.pool())
                .await
                .unwrap()
                .try_get("enabled_skills")
                .unwrap();
        assert_eq!(column, None, "new sessions must leave enabled_skills NULL");
    }

    /// thinkingLevel / enabledTools / workingDir / modelId all persist through
    /// update_session_field.
    #[tokio::test]
    async fn update_field_other_parameters_persist_after_variant_removal() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let created = service
            .create_session(base_request("Field Mappings"))
            .await
            .unwrap();

        let work_dir = TempDir::new().unwrap();
        let canonical = std::fs::canonicalize(work_dir.path())
            .unwrap()
            .to_string_lossy()
            .into_owned();

        for parameter in [
            AgentSessionParameter::ThinkingLevel(Some("low".to_string())),
            AgentSessionParameter::EnabledTools(vec!["read".to_string()]),
            AgentSessionParameter::WorkingDir(Some(canonical.clone())),
            AgentSessionParameter::ModelId(Some("gpt-4.1".to_string())),
        ] {
            service
                .update_session_field(created.id.clone(), parameter)
                .await
                .unwrap();
        }

        let reloaded = service.get_session(created.id).await.unwrap();
        assert_eq!(reloaded.thinking_level, Some("low".to_string()));
        assert_eq!(reloaded.enabled_tools, vec!["read".to_string()]);
        assert_eq!(reloaded.working_dir, Some(canonical));
        assert_eq!(reloaded.model_id, Some("gpt-4.1".to_string()));
    }

    #[tokio::test]
    async fn get_session_returns_not_found() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let err = service
            .get_session("nonexistent".to_string())
            .await
            .expect_err("expected error");
        assert_eq!(err.code, "NOT_FOUND");
    }

    // A create+delete cycle against a DB that already holds preset `agents` rows
    // leaves that table's COUNT unchanged: this service reaches no preset
    // surface at all.
    #[tokio::test]
    async fn create_delete_cycle_leaves_preset_agents_table_unchanged() {
        let (db, _guard) = create_test_database().await;

        // Seed the preset table directly (no preset service involved) so we can
        // prove the agent_session path never touches it.
        let now = AgentSessionService::current_timestamp();
        sqlx::query(
            "INSERT INTO agents (id, name, mcp_servers, skills, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind("agent-seed")
        .bind("Seed Agent")
        .bind("[]")
        .bind("[]")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        // 1 user row + the 2 seeded builtin definitions (builtin-chat /
        // builtin-coding).
        let agents_before = count_rows(&db, "agents").await;
        assert_eq!(agents_before, 3);

        // Exercise the agent_session create+delete cycle ONLY.
        let service = AgentSessionService::new(db.clone());
        let created = service
            .create_session(base_request("Isolated"))
            .await
            .unwrap();
        service.delete_session(created.id).await.unwrap();

        // The preset table is untouched.
        assert_eq!(count_rows(&db, "agents").await, agents_before);
    }
}
