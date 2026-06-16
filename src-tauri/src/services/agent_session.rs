// Agent Session 服务实现
//
// Agent 模式会话的 CRUD 业务逻辑层，建立在 `AgentSessionRepository` 之上，
// 与 Chat 模式的 `SessionService` / 预设 `AgentService` 完全独立。
// 仅负责会话 CRUD 与 transcript 读取；runtime / run / streaming / tools
// 属于后续 feature，不在此层实现。

use crate::models::AppError;
use crate::services::Database;
use crate::storage::types::{AgentSession, AgentSessionMessage, CreateAgentSessionRequest, UUID};
use crate::storage::{AgentProjectRepository, AgentSessionRepository};
use std::sync::Arc;

/// Agent Session 可更新参数类型（镜像 `AgentParameter`，按字段更新）
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
    ToolExecutionMode(Option<String>),
}

/// Agent Session 服务
#[derive(Clone)]
pub struct AgentSessionService {
    repository: AgentSessionRepository,
    /// 直接持有 project 仓储层（而非 `AgentProjectService`）：create 挂靠
    /// project 时只需按 id 解析一行，轻依赖即可，避免 service 间环状耦合。
    projects: AgentProjectRepository,
}

impl AgentSessionService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: AgentSessionRepository::new(Arc::clone(&db)),
            projects: AgentProjectRepository::new(db),
        }
    }

    /// 创建 Agent Session
    ///
    /// `project_id` 挂靠：若提供（空字符串视为未设置），则按 id 解析 project
    /// （不存在 -> `NOT_FOUND`），并要求 `project.path` 当前仍 canonicalize
    /// 回它自己且是目录——目录未被删除、也未被换成 symlink（否则
    /// `VALIDATION_ERROR`）。校验失败一律不写入任何行。
    /// 通过后把 `project.path`（创建时已 canonical）复制进 `working_dir`，
    /// **覆盖** 请求中的 working_dir——project 优先，runtime 的 working_dir
    /// 消费点因此零改动。
    ///
    /// 无 `project_id` 时行为不变：`working_dir` 若提供，则必须是一个
    /// **绝对路径** 且能 canonicalize 到一个 **已存在的目录**（symlink-to-dir
    /// 解析为其 canonical 目标后被接受）。存储 canonical 绝对路径。非绝对路径 /
    /// 不存在的路径 / 指向文件（非目录）的路径一律以 `AppError` 拒绝，且不写入
    /// 任何行。空字符串 / None 视为未设置，存储为 null。
    pub async fn create_session(
        &self,
        request: CreateAgentSessionRequest,
    ) -> Result<AgentSession, AppError> {
        let requested_project_id = request
            .project_id
            .as_deref()
            .filter(|id| !id.is_empty())
            .map(str::to_owned);

        let (project_id, working_dir) = match requested_project_id {
            Some(pid) => {
                let project = self
                    .projects
                    .get_project_by_id(&pid)
                    .await?
                    .ok_or_else(|| {
                        AppError::not_found(&format!("Agent project not found: {}", pid))
                    })?;

                // project.path 创建时已 canonical；此处复核它当前仍 canonicalize
                // 回它自己且是目录：目录可能在创建 project 后被删除，或被换成
                // 指向别处的 symlink（canonicalize 结果将不再等于自身）。
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

                (Some(project.id), Some(project.path))
            }
            None => (
                None,
                Self::validate_working_dir(request.working_dir.as_deref())?,
            ),
        };

        let now = Self::current_timestamp();
        let session = AgentSession {
            id: uuid::Uuid::new_v4().to_string(),
            name: request.name,
            project_id,
            model_id: request.model_id,
            provider_id: request.provider_id,
            system_prompt: request.system_prompt,
            thinking_level: request.thinking_level,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            working_dir,
            enabled_tools: request.enabled_tools.unwrap_or_default(),
            tool_execution_mode: request.tool_execution_mode,
            message_count: 0,
            last_message_at: None,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_session(&session).await?;
        Ok(session)
    }

    /// 获取 Agent Session 列表（按 updated_at 降序）
    ///
    /// 不传 `limit` 即全量返回：前端分组侧栏按 project 分组消费完整列表，
    /// 默认值绝不能静默截断（`i32::MAX` 对 SQLite 的 LIMIT 等效于无上限）。
    pub async fn list_sessions(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<AgentSession>, AppError> {
        let limit = limit.unwrap_or(i32::MAX);
        let offset = offset.unwrap_or(0);
        self.repository.list_sessions(limit, offset).await
    }

    /// 获取 Agent Session 详情
    pub async fn get_session(&self, session_id: UUID) -> Result<AgentSession, AppError> {
        match self.repository.get_session_by_id(&session_id).await? {
            Some(session) => Ok(session),
            None => Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            ))),
        }
    }

    /// 重命名 Agent Session
    pub async fn rename_session(
        &self,
        session_id: UUID,
        name: String,
    ) -> Result<AgentSession, AppError> {
        self.repository.rename_session(&session_id, &name).await?;
        self.get_session(session_id).await
    }

    /// 统一的单字段更新方法（镜像 `agent_update_field`）
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
                // 复用与 create 一致的校验：保证存储的总是 canonical 绝对目录或 null。
                session.working_dir = Self::validate_working_dir(working_dir.as_deref())?;
            }
            AgentSessionParameter::EnabledTools(tools) => session.enabled_tools = tools,
            AgentSessionParameter::ToolExecutionMode(mode) => session.tool_execution_mode = mode,
        }

        session.updated_at = Self::current_timestamp();
        self.repository.update_session(&session).await?;
        Ok(session)
    }

    /// 删除 Agent Session（仓储层显式级联删除其 transcript）
    pub async fn delete_session(&self, session_id: UUID) -> Result<(), AppError> {
        self.repository.delete_session(&session_id).await
    }

    /// 获取某个会话的全部 transcript（按 seq 升序）
    pub async fn list_messages(
        &self,
        session_id: UUID,
    ) -> Result<Vec<AgentSessionMessage>, AppError> {
        self.repository.list_messages(&session_id).await
    }

    /// 校验并规范化 `working_dir`。
    ///
    /// - `None` 或空字符串 -> `Ok(None)`（存储 null）。
    /// - 非绝对路径 -> `Err`（即使能相对于 cwd 解析，也必须拒绝）。
    /// - canonicalize 失败（不存在）-> `Err`。
    /// - canonical 目标不是目录（如指向文件）-> `Err`。
    /// - 否则 -> `Ok(Some(canonical_absolute_path))`。
    fn validate_working_dir(working_dir: Option<&str>) -> Result<Option<String>, AppError> {
        let raw = match working_dir {
            None | Some("") => return Ok(None),
            Some(s) => s,
        };

        let path = std::path::Path::new(raw);

        // 必须是绝对路径：相对路径即便能相对 cwd canonicalize 也一律拒绝，保持确定性。
        if !path.is_absolute() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("working_dir must be an absolute path: {}", raw),
                "请提供一个已存在目录的绝对路径",
            ));
        }

        // canonicalize 会解析 symlink 并要求路径存在；失败即视为不存在。
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

    /// 测试用数据库（持有 TempDir 以保证文件存活）
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
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: None,
            thinking_level: None,
            temperature: None,
            max_tokens: None,
            working_dir: None,
            enabled_tools: None,
            tool_execution_mode: None,
        }
    }

    async fn count_rows(db: &Database, table: &str) -> i64 {
        let row = sqlx::query(&format!("SELECT COUNT(*) AS count FROM {}", table))
            .fetch_one(db.pool())
            .await
            .unwrap();
        row.try_get::<i64, _>("count").unwrap()
    }

    // --- VAL-SESSION-003: valid existing absolute dir is stored canonicalized ---

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

    // --- VAL-SESSION-004: invalid working_dir rejected, no row written ---

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

    // --- VAL-CREATE-010 + project attach: create_session with project_id ---

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

    /// VAL-CASESS-006 — two sessions whose working directories canonicalize to
    /// the SAME path are grouped under one project: project get-or-create keys
    /// off the canonical path, so a second create for the same directory (here
    /// reached via a symlink alias) returns the same `project_id`, and both
    /// sessions therefore carry the same `project_id` the sidebar groups by.
    /// This is the data-layer geology under the frontend `groupSessions`
    /// grouping; the grouping itself depends only on the SQLite `project_id` and
    /// is unaffected by JSONL persistence.
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

    // --- sidebar consumer: default list must not silently truncate ---

    #[tokio::test]
    async fn list_sessions_default_limit_does_not_truncate() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        // 60 sessions exceeds the previous default limit of 50.
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

    // --- CRUD roundtrip via the service ---

    #[tokio::test]
    async fn service_crud_roundtrip() {
        let (db, _guard) = create_test_database().await;
        let service = AgentSessionService::new(db);

        let created = service
            .create_session(base_request("Roundtrip"))
            .await
            .unwrap();

        // List
        let listed = service.list_sessions(Some(10), Some(0)).await.unwrap();
        assert_eq!(listed.len(), 1);

        // Get
        let got = service.get_session(created.id.clone()).await.unwrap();
        assert_eq!(got.name, "Roundtrip");

        // Rename
        let renamed = service
            .rename_session(created.id.clone(), "Renamed".to_string())
            .await
            .unwrap();
        assert_eq!(renamed.name, "Renamed");

        // Update field
        let updated = service
            .update_session_field(
                created.id.clone(),
                AgentSessionParameter::ThinkingLevel(Some("high".to_string())),
            )
            .await
            .unwrap();
        assert_eq!(updated.thinking_level, Some("high".to_string()));

        // Messages (empty transcript)
        let msgs = service.list_messages(created.id.clone()).await.unwrap();
        assert!(msgs.is_empty());

        // Delete
        service.delete_session(created.id.clone()).await.unwrap();
        let err = service.get_session(created.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
    }

    /// VAL-DEPRECATE-008 / VAL-DEPRECATE-009: a create request still carrying
    /// the deprecated enabledSkills key succeeds (serde ignores unknown keys)
    /// and the deactivated DB column stays NULL.
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

    /// VAL-DEPRECATE-003: removing the EnabledSkills variant leaves every other
    /// field mapping intact — thinkingLevel / enabledTools / workingDir /
    /// modelId still persist through update_session_field.
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

    // --- VAL-SESSION-011 + VAL-SESSION-012 ---
    //
    // VAL-SESSION-011 (structural): the service holds ONLY an
    // `AgentSessionRepository` — it cannot reach chat `SessionService` /
    // preset `AgentService` / chat/preset repos. This is enforced by the
    // single-field struct above and is asserted here by exercising the full
    // create+delete path through the public API (no chat/preset surface
    // exists on this type to invoke).
    //
    // VAL-SESSION-012 (data): a create+delete cycle against a DB that already
    // contains agents / sessions / messages rows leaves all three table COUNTs
    // unchanged.
    #[tokio::test]
    async fn create_delete_cycle_leaves_chat_and_preset_tables_unchanged() {
        let (db, _guard) = create_test_database().await;

        // Seed the chat/preset tables directly (no chat/preset service involved)
        // so we can prove the agent_session path never touches them.
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

        sqlx::query(
            "INSERT INTO sessions (id, name, message_count, mcp_servers, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind("session-seed")
        .bind("Seed Session")
        .bind(0)
        .bind("[]")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO messages (id, session_id, role, content, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind("message-seed")
        .bind("session-seed")
        .bind("user")
        .bind("hello")
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        let agents_before = count_rows(&db, "agents").await;
        let sessions_before = count_rows(&db, "sessions").await;
        let messages_before = count_rows(&db, "messages").await;
        assert_eq!((agents_before, sessions_before, messages_before), (1, 1, 1));

        // Exercise the agent_session create+delete cycle ONLY.
        let service = AgentSessionService::new(db.clone());
        let created = service
            .create_session(base_request("Isolated"))
            .await
            .unwrap();
        service.delete_session(created.id).await.unwrap();

        // The three chat/preset tables are untouched.
        assert_eq!(count_rows(&db, "agents").await, agents_before);
        assert_eq!(count_rows(&db, "sessions").await, sessions_before);
        assert_eq!(count_rows(&db, "messages").await, messages_before);
    }
}
