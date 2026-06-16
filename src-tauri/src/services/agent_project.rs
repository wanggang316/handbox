// Agent Project 服务实现
//
// Agent 模式项目（按工作目录分组会话）的业务逻辑层，建立在
// `AgentProjectRepository` 之上，与 Chat 模式的 `SessionService` /
// `/agents` 预设的 `AgentService` 完全独立。
//
// path 校验与 `AgentSessionService::validate_working_dir` 同等严格，但语义
// 不同：session 的 working_dir 允许 None / 空串（归一为 null），而 project
// 的 path 是身份标识，**必须**非空。两者因此各自独立实现，不共享 helper，
// 避免「空值放行」被误复用。

use crate::models::AppError;
use crate::services::{AgentRuntime, Database};
use crate::storage::types::{AgentProject, CreateAgentProjectRequest, UUID};
use crate::storage::AgentProjectRepository;
use sqlx::Row;
use std::sync::Arc;

/// Agent Project 服务
#[derive(Clone)]
pub struct AgentProjectService {
    /// 直接持有 db 以查询项目下的 session id 集合（delete 前逐个 abort 用）；
    /// `AgentProjectRepository` / `AgentSessionRepository` 均未暴露该查询。
    db: Arc<Database>,
    repository: AgentProjectRepository,
}

impl AgentProjectService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: AgentProjectRepository::new(Arc::clone(&db)),
            db,
        }
    }

    /// 创建 Agent Project（get-or-create by canonical path）。
    ///
    /// `path` 必须是 **非空** 的 **绝对路径**，且能 canonicalize 到一个
    /// **已存在的目录**（symlink-to-dir 解析为其 canonical 目标）。存储
    /// canonical 绝对路径；默认 name 取 canonical path 的 basename，
    /// basename 为空（如根路径 `/`）时回退为完整 canonical path。
    ///
    /// 同 path（含 symlink 别名解析后）命中已有项目时原样返回，不改写其
    /// name / created_at / updated_at。空串 / 相对路径 / 指向文件 / 磁盘不
    /// 存在的路径一律 `VALIDATION_ERROR`，且不写入任何行。
    pub async fn create_project(&self, path: String) -> Result<AgentProject, AppError> {
        let canonical = Self::validate_project_path(&path)?;
        let name = default_project_name(&canonical);
        self.repository
            .create_project(&CreateAgentProjectRequest {
                path: canonical,
                name,
            })
            .await
    }

    /// 获取全部 Agent Project
    pub async fn list_projects(&self) -> Result<Vec<AgentProject>, AppError> {
        self.repository.list_projects().await
    }

    /// 获取 Agent Project 详情
    pub async fn get_project(&self, project_id: UUID) -> Result<AgentProject, AppError> {
        match self.repository.get_project_by_id(&project_id).await? {
            Some(project) => Ok(project),
            None => Err(AppError::not_found(&format!(
                "Agent project not found: {}",
                project_id
            ))),
        }
    }

    /// 重命名 Agent Project。
    ///
    /// trim 后为空白 -> `VALIDATION_ERROR`：项目名是分组侧栏的组头，空白名
    /// 会产生不可辨识的分组，故 trim 后拒空（session rename 无此约束）；
    /// 存储 trim 后的 name；项目不存在时透传仓储层的 `NOT_FOUND`。
    pub async fn rename_project(
        &self,
        project_id: UUID,
        name: String,
    ) -> Result<AgentProject, AppError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(AppError::validation_error(
                "Agent project name must not be blank",
            ));
        }
        self.repository.rename_project(&project_id, trimmed).await?;
        self.get_project(project_id).await
    }

    /// 删除 Agent Project（先 abort 后级联）。
    ///
    /// 先列出该项目全部 session id，逐个调用 `runtime.abort`（对无活跃 run
    /// 的 session 是干净的 no-op，对齐 `agent_session_delete` 先 abort 再删
    /// 的写法），再调仓储层在单事务内级联删除 messages / sessions / project。
    /// 项目不存在时透传 `NOT_FOUND`。
    pub async fn delete_project(
        &self,
        project_id: UUID,
        runtime: &AgentRuntime,
    ) -> Result<(), AppError> {
        self.delete_project_with_abort(project_id, |session_id| async move {
            runtime.abort(&session_id).await;
        })
        .await
    }

    /// `delete_project` 的实现体：abort 解耦为可注入闭包。
    ///
    /// 拆出这一层是为了在单测中无需真实启动 run（AgentRuntime 的 run 注册表
    /// 对外不可见、测试 seed 设施私有于 agent_runtime 模块），即可断言
    /// 「每个 session 先被 abort、且 abort 发生在级联删除之前」。
    async fn delete_project_with_abort<F, Fut>(
        &self,
        project_id: UUID,
        abort: F,
    ) -> Result<(), AppError>
    where
        F: Fn(UUID) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let session_ids = self.list_session_ids(&project_id).await?;
        for session_id in session_ids {
            abort(session_id).await;
        }
        self.repository.delete_project(&project_id).await
    }

    /// 列出某项目下全部 session id（项目不存在时为空集合，由后续仓储层
    /// delete 报 `NOT_FOUND`）。
    async fn list_session_ids(&self, project_id: &UUID) -> Result<Vec<UUID>, AppError> {
        let rows = sqlx::query("SELECT id FROM agent_sessions WHERE project_id = $1")
            .bind(project_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list project session ids: {}", e))
            })?;

        rows.into_iter()
            .map(|row| Ok(row.try_get::<String, _>("id")?))
            .collect()
    }

    /// 校验并规范化 project path。
    ///
    /// - 空字符串 -> `Err`（project path 是身份标识，不允许缺省）。
    /// - 非绝对路径 -> `Err`（即使能相对于 cwd 解析，也必须拒绝）。
    /// - canonicalize 失败（不存在）-> `Err`。
    /// - canonical 目标不是目录（如指向文件）-> `Err`。
    /// - 否则 -> `Ok(canonical_absolute_path)`。
    fn validate_project_path(raw: &str) -> Result<String, AppError> {
        if raw.is_empty() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                "project path must not be empty",
                "请提供一个已存在目录的绝对路径",
            ));
        }

        let path = std::path::Path::new(raw);

        // 必须是绝对路径：相对路径即便能相对 cwd canonicalize 也一律拒绝，保持确定性。
        if !path.is_absolute() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("project path must be an absolute path: {}", raw),
                "请提供一个已存在目录的绝对路径",
            ));
        }

        // canonicalize 会解析 symlink 并要求路径存在；失败即视为不存在。
        let canonical = std::fs::canonicalize(path).map_err(|_| {
            AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("project path does not exist: {}", raw),
                "请提供一个已存在目录的绝对路径",
            )
        })?;

        if !canonical.is_dir() {
            return Err(AppError::with_hint(
                "VALIDATION_ERROR",
                &format!("project path is not a directory: {}", raw),
                "project path 必须指向一个目录而非文件",
            ));
        }

        Ok(canonical.to_string_lossy().into_owned())
    }
}

/// 默认项目名：canonical path 的 basename；basename 为空（如根路径 `/`）
/// 时回退为完整 canonical path。
///
/// Hoisted out of `AgentProjectService` to a free `pub fn` so the SQLite→JSONL
/// migration can derive a JSONL session's project group name (from its
/// `header.cwd`, canonicalized first) with the SAME algorithm `create_project`
/// uses for `agent_projects.name` — guaranteeing a session keeps its project
/// group across the new (JSONL) and legacy (SQLite) sources (VAL-CASESS-024).
/// The algorithm is unchanged from the previous private method.
pub fn default_project_name(canonical: &str) -> String {
    std::path::Path::new(canonical)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| canonical.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

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

    async fn count_rows(db: &Database, table: &str) -> i64 {
        let row = sqlx::query(&format!("SELECT COUNT(*) AS count FROM {}", table))
            .fetch_one(db.pool())
            .await
            .unwrap();
        row.try_get::<i64, _>("count").unwrap()
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    /// 直接插入一条 agent_sessions 行（可选挂到某个项目），返回 session id。
    async fn insert_session(db: &Database, project_id: Option<&str>, name: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, name, project_id, message_count, created_at, updated_at)
            VALUES ($1, $2, $3, 0, $4, $5)
        "#,
        )
        .bind(&id)
        .bind(name)
        .bind(project_id)
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();
        id
    }

    /// 直接插入一条 transcript 行。
    async fn insert_message(db: &Database, session_id: &str, seq: i64) {
        sqlx::query(
            r#"
            INSERT INTO agent_session_messages (id, session_id, seq, role, payload, created_at)
            VALUES ($1, $2, $3, 'user', '{"text":"hi"}', $4)
        "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(session_id)
        .bind(seq)
        .bind(now_ms())
        .execute(db.pool())
        .await
        .unwrap();
    }

    // --- VAL-PROJ-010: invalid path rejected with VALIDATION_ERROR, no row ---

    #[tokio::test]
    async fn create_project_rejects_empty_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let err = service
            .create_project(String::new())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_rejects_relative_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        // A relative path that may well exist relative to cwd, yet must be rejected.
        let err = service
            .create_project("src".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_rejects_file_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        // An existing FILE (not a dir).
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("a-file.txt");
        std::fs::write(&file_path, b"hello").unwrap();

        let err = service
            .create_project(file_path.to_string_lossy().into_owned())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    #[tokio::test]
    async fn create_project_rejects_missing_path_and_writes_no_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let err = service
            .create_project("/this/path/should/not/exist/handbox-agent-project".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert_eq!(count_rows(&db, "agent_projects").await, 0);
    }

    // --- get-or-create: same dir twice (incl. symlink alias) -> single row ---

    #[tokio::test]
    async fn create_project_stores_canonical_path_and_basename_name() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        let work_dir = TempDir::new().unwrap();
        let sub_dir = work_dir.path().join("alpha");
        std::fs::create_dir(&sub_dir).unwrap();
        let expected_canonical = std::fs::canonicalize(&sub_dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let created = service
            .create_project(sub_dir.to_string_lossy().into_owned())
            .await
            .expect("create failed");
        assert_eq!(created.path, expected_canonical);
        assert_eq!(created.name, "alpha");
    }

    #[tokio::test]
    async fn create_project_twice_same_dir_returns_existing_single_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let work_dir = TempDir::new().unwrap();
        let raw = work_dir.path().to_string_lossy().into_owned();

        let first = service.create_project(raw.clone()).await.unwrap();

        // Rename so a second create proves "returns existing unchanged".
        let renamed = service
            .rename_project(first.id.clone(), "Custom Name".to_string())
            .await
            .unwrap();

        let second = service.create_project(raw).await.unwrap();
        assert_eq!(second.id, first.id);
        assert_eq!(second.name, "Custom Name");
        assert_eq!(second.created_at, first.created_at);
        assert_eq!(second.updated_at, renamed.updated_at);
        assert_eq!(count_rows(&db, "agent_projects").await, 1);
    }

    #[tokio::test]
    async fn create_project_via_symlink_alias_returns_existing_single_row() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

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

        let direct = service
            .create_project(target.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(direct.path, canonical_target);

        // The symlink alias resolves to the same canonical path -> same project.
        let via_link = service
            .create_project(link.to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(via_link.id, direct.id);
        assert_eq!(via_link.path, canonical_target);
        assert_eq!(via_link.name, direct.name);
        assert_eq!(count_rows(&db, "agent_projects").await, 1);
    }

    #[tokio::test]
    async fn create_project_root_path_falls_back_to_full_path_name() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        // Root path: basename is empty -> name falls back to the full path.
        let created = service.create_project("/".to_string()).await.unwrap();
        assert_eq!(created.path, "/");
        assert_eq!(created.name, "/");
    }

    // --- list / get ---

    #[tokio::test]
    async fn list_and_get_project_roundtrip() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        let work_dir = TempDir::new().unwrap();
        let created = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        let listed = service.list_projects().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let got = service.get_project(created.id.clone()).await.unwrap();
        assert_eq!(got.path, created.path);

        let err = service
            .get_project("nonexistent".to_string())
            .await
            .expect_err("expected error");
        assert_eq!(err.code, "NOT_FOUND");
    }

    // --- rename: blank rejected, trimmed stored, NOT_FOUND passthrough ---

    #[tokio::test]
    async fn rename_project_rejects_blank_and_trims_valid_name() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db);

        let work_dir = TempDir::new().unwrap();
        let created = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        // Blank (whitespace-only) name -> VALIDATION_ERROR, nothing changed.
        let err = service
            .rename_project(created.id.clone(), "   ".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "VALIDATION_ERROR");
        let unchanged = service.get_project(created.id.clone()).await.unwrap();
        assert_eq!(unchanged.name, created.name);
        assert_eq!(unchanged.updated_at, created.updated_at);

        // Valid name is trimmed before storage.
        let renamed = service
            .rename_project(created.id.clone(), "  New Name  ".to_string())
            .await
            .unwrap();
        assert_eq!(renamed.name, "New Name");

        // Missing id -> NOT_FOUND passthrough.
        let err = service
            .rename_project("missing".to_string(), "x".to_string())
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "NOT_FOUND");
    }

    // --- delete: abort each session BEFORE cascade, NOT_FOUND passthrough ---

    #[tokio::test]
    async fn delete_project_aborts_each_session_before_cascade() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());

        let work_dir = TempDir::new().unwrap();
        let project = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();

        let s1 = insert_session(&db, Some(&project.id), "s1").await;
        let s2 = insert_session(&db, Some(&project.id), "s2").await;
        insert_message(&db, &s1, 0).await;
        insert_message(&db, &s2, 0).await;
        // A bystander session outside the project must NOT be aborted.
        let outsider = insert_session(&db, None, "outsider").await;

        // Recording abort: capture (session_id, rows still present at abort
        // time) to prove abort happens BEFORE the cascade delete.
        let abort_log: Arc<Mutex<Vec<(String, i64)>>> = Arc::new(Mutex::new(Vec::new()));
        let db_for_abort = db.clone();
        let log_for_abort = Arc::clone(&abort_log);
        let abort = move |session_id: String| {
            let db = db_for_abort.clone();
            let log = Arc::clone(&log_for_abort);
            async move {
                let still_present: i64 =
                    sqlx::query("SELECT COUNT(*) AS count FROM agent_sessions WHERE id = $1")
                        .bind(&session_id)
                        .fetch_one(db.pool())
                        .await
                        .unwrap()
                        .try_get("count")
                        .unwrap();
                log.lock().await.push((session_id, still_present));
            }
        };

        service
            .delete_project_with_abort(project.id.clone(), abort)
            .await
            .unwrap();

        // Both project sessions were aborted while their rows still existed.
        let log = abort_log.lock().await;
        assert_eq!(log.len(), 2);
        let aborted: Vec<&str> = log.iter().map(|(id, _)| id.as_str()).collect();
        assert!(aborted.contains(&s1.as_str()));
        assert!(aborted.contains(&s2.as_str()));
        assert!(!aborted.contains(&outsider.as_str()));
        assert!(log.iter().all(|(_, present)| *present == 1));

        // Cascade: project, its sessions and their messages are gone.
        let err = service.get_project(project.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_session_messages").await, 0);
        // Only the ungrouped bystander session remains.
        assert_eq!(count_rows(&db, "agent_sessions").await, 1);
    }

    #[tokio::test]
    async fn delete_project_with_runtime_cascades_and_passes_through_not_found() {
        let (db, _guard) = create_test_database().await;
        let service = AgentProjectService::new(db.clone());
        // A real runtime: abort on sessions without an active run is a clean
        // no-op, so the full public path is exercised end to end.
        let runtime = AgentRuntime::new(db.clone());

        let work_dir = TempDir::new().unwrap();
        let project = service
            .create_project(work_dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        let session = insert_session(&db, Some(&project.id), "s1").await;
        insert_message(&db, &session, 0).await;

        service
            .delete_project(project.id.clone(), &runtime)
            .await
            .unwrap();
        let err = service.get_project(project.id).await.expect_err("gone");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(count_rows(&db, "agent_sessions").await, 0);
        assert_eq!(count_rows(&db, "agent_session_messages").await, 0);

        // Missing id -> NOT_FOUND passthrough (and no panic from abort phase).
        let err = service
            .delete_project("missing".to_string(), &runtime)
            .await
            .expect_err("should reject");
        assert_eq!(err.code, "NOT_FOUND");
    }
}
