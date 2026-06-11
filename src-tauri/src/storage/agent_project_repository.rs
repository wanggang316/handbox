// Agent Project 数据访问层
//
// Agent 模式项目（按工作目录分组会话）的持久化层，建立在 `agent_projects`
// 表与 `agent_sessions.project_id` 列之上。与 Chat 模式的 `session_repository`
// 以及 `/agents` 预设（`agents` 表）完全独立。
//
// path 的 canonicalize 在 service 层完成；仓库层信任传入 path 已 canonical，
// get-or-create 的去重按字符串全等（数据库 UNIQUE 约束兜底）。

use crate::models::AppError;
use crate::storage::types::{AgentProject, CreateAgentProjectRequest, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Agent Project 仓储层
#[derive(Clone)]
pub struct AgentProjectRepository {
    db: Arc<Database>,
}

impl AgentProjectRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建 Agent Project（get-or-create by path）。
    ///
    /// 同 path 已存在时返回已有项目，绝不改写其 name / created_at /
    /// updated_at。并发同 path 创建时（UNIQUE 竞态），落败方不会向调用方
    /// 暴露约束冲突错误：`INSERT ... ON CONFLICT(path) DO NOTHING` 把冲突
    /// 静默吞掉，随后的 SELECT 取回胜出方写入的行。
    pub async fn create_project(
        &self,
        request: &CreateAgentProjectRequest,
    ) -> Result<AgentProject, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Self::now_ms();

        sqlx::query(
            r#"
            INSERT INTO agent_projects (id, path, name, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT(path) DO NOTHING
        "#,
        )
        .bind(&id)
        .bind(&request.path)
        .bind(&request.name)
        .bind(now)
        .bind(now)
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to create agent project: {}", e)))?;

        // 无论 INSERT 是否生效，统一按 path 取回当前行：覆盖「本次新建」与
        // 「已存在 / 并发胜出方写入」两种情况，调用方拿到的总是数据库中的真实行。
        self.get_project_by_path(&request.path)
            .await?
            .ok_or_else(|| {
                AppError::internal_error(&format!(
                    "Agent project disappeared after create: {}",
                    request.path
                ))
            })
    }

    /// 根据 ID 获取 Agent Project
    pub async fn get_project_by_id(
        &self,
        project_id: &UUID,
    ) -> Result<Option<AgentProject>, AppError> {
        let row = sqlx::query(
            "SELECT id, path, name, created_at, updated_at FROM agent_projects WHERE id = $1",
        )
        .bind(project_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get agent project: {}", e)))?;

        row.map(Self::row_to_project).transpose()
    }

    /// 根据 path 获取 Agent Project（字符串全等匹配）
    pub async fn get_project_by_path(&self, path: &str) -> Result<Option<AgentProject>, AppError> {
        let row = sqlx::query(
            "SELECT id, path, name, created_at, updated_at FROM agent_projects WHERE path = $1",
        )
        .bind(path)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to get agent project: {}", e)))?;

        row.map(Self::row_to_project).transpose()
    }

    /// 获取全部 Agent Project（created_at 降序、id 升序保证稳定；展示排序由前端做）
    pub async fn list_projects(&self) -> Result<Vec<AgentProject>, AppError> {
        let rows = sqlx::query(
            "SELECT id, path, name, created_at, updated_at FROM agent_projects ORDER BY created_at DESC, id ASC",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to list agent projects: {}", e)))?;

        rows.into_iter().map(Self::row_to_project).collect()
    }

    /// 重命名 Agent Project（同时刷新 updated_at）
    pub async fn rename_project(&self, project_id: &UUID, name: &str) -> Result<(), AppError> {
        let now = Self::now_ms();

        let result =
            sqlx::query("UPDATE agent_projects SET name = $1, updated_at = $2 WHERE id = $3")
                .bind(name)
                .bind(now)
                .bind(project_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to rename agent project: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent project not found: {}",
                project_id
            )));
        }

        Ok(())
    }

    /// 删除 Agent Project（显式级联删除其会话及 transcript）
    ///
    /// # 为什么显式删除而不依赖 `ON DELETE CASCADE`？
    ///
    /// 这里在同一个事务里依次删除该项目全部会话的 `agent_session_messages`、
    /// 该项目的 `agent_sessions` 行、最后是 `agent_projects` 行本身。显式级联
    /// 是一种防御性写法，与连接的 `PRAGMA foreign_keys` 状态无关：无论 FK
    /// 强制开启与否，三层数据都会被原子地清除，不留孤儿行。
    ///
    /// 两条删除均以 `project_id = $1` 为界：`project_id IS NULL` 的未分组
    /// 会话与兄弟项目的会话天然不在删除范围内。
    pub async fn delete_project(&self, project_id: &UUID) -> Result<(), AppError> {
        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to begin transaction: {}", e))
        })?;

        // 1. 先删除该项目全部会话的 transcript 行（以项目的会话集合为界）
        sqlx::query(
            r#"
            DELETE FROM agent_session_messages
            WHERE session_id IN (SELECT id FROM agent_sessions WHERE project_id = $1)
        "#,
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::internal_error(&format!(
                "Failed to delete agent project session messages: {}",
                e
            ))
        })?;

        // 2. 再删除该项目的全部会话行
        sqlx::query("DELETE FROM agent_sessions WHERE project_id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete agent project sessions: {}", e))
            })?;

        // 3. 最后删除项目行本身
        let result = sqlx::query("DELETE FROM agent_projects WHERE id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete agent project: {}", e))
            })?;

        if result.rows_affected() == 0 {
            // 项目不存在：回滚（合法数据下步骤 1/2 不会命中任何行；即使存在
            // 指向缺失项目的脏数据，回滚也保证零副作用），返回 NotFound。
            tx.rollback().await.map_err(|e| {
                AppError::internal_error(&format!("Failed to rollback transaction: {}", e))
            })?;
            return Err(AppError::not_found(&format!(
                "Agent project not found: {}",
                project_id
            )));
        }

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// 统计某项目下的会话数（项目不存在时返回 0）
    pub async fn session_count(&self, project_id: &UUID) -> Result<i64, AppError> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM agent_sessions WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to count project sessions: {}", e))
            })?;

        Ok(row.try_get("count")?)
    }

    /// 当前时间（毫秒）
    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    // 辅助方法：将数据库行转换为 AgentProject
    fn row_to_project(row: sqlx::sqlite::SqliteRow) -> Result<AgentProject, AppError> {
        Ok(AgentProject {
            id: row.try_get("id")?,
            path: row.try_get("path")?,
            name: row.try_get("name")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_service = Database::new(&db_path).await.unwrap();
        (db_service, temp_dir)
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn sample_request(path: &str, name: &str) -> CreateAgentProjectRequest {
        CreateAgentProjectRequest {
            path: path.to_string(),
            name: name.to_string(),
        }
    }

    /// 直接插入一条 agent_sessions 行（可选挂到某个项目），返回 session id。
    /// 不走 AgentSessionRepository：本仓库的测试只关心 project_id 维度的行为。
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

    async fn count_rows(db: &Database, query: &str, bind: &str) -> i64 {
        sqlx::query(query)
            .bind(bind)
            .fetch_one(db.pool())
            .await
            .unwrap()
            .try_get::<i64, _>("count")
            .unwrap()
    }

    async fn count_sessions(db: &Database, project_id: &str) -> i64 {
        count_rows(
            db,
            "SELECT COUNT(*) AS count FROM agent_sessions WHERE project_id = $1",
            project_id,
        )
        .await
    }

    async fn count_messages(db: &Database, session_id: &str) -> i64 {
        count_rows(
            db,
            "SELECT COUNT(*) AS count FROM agent_session_messages WHERE session_id = $1",
            session_id,
        )
        .await
    }

    async fn count_projects_by_path(db: &Database, path: &str) -> i64 {
        count_rows(
            db,
            "SELECT COUNT(*) AS count FROM agent_projects WHERE path = $1",
            path,
        )
        .await
    }

    #[tokio::test]
    async fn test_agent_project_crud_roundtrip() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentProjectRepository::new(db_arc.clone());

        // Create
        let created = repo
            .create_project(&sample_request("/tmp/workspace/alpha", "alpha"))
            .await
            .unwrap();
        assert_eq!(created.path, "/tmp/workspace/alpha");
        assert_eq!(created.name, "alpha");
        assert!(created.created_at > 0);
        assert_eq!(created.created_at, created.updated_at);

        // Get by ID / by path
        let by_id = repo.get_project_by_id(&created.id).await.unwrap().unwrap();
        assert_eq!(by_id.id, created.id);
        assert_eq!(by_id.path, created.path);

        let by_path = repo
            .get_project_by_path("/tmp/workspace/alpha")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_path.id, created.id);

        // Missing lookups return None (not errors).
        assert!(repo
            .get_project_by_id(&"missing".to_string())
            .await
            .unwrap()
            .is_none());
        assert!(repo.get_project_by_path("/nope").await.unwrap().is_none());

        // List: stable order is created_at DESC, then id ASC (as promised by
        // the list_projects doc comment). Sleep so created_at values differ.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = repo
            .create_project(&sample_request("/tmp/workspace/beta", "beta"))
            .await
            .unwrap();
        assert!(second.created_at > created.created_at);
        let projects = repo.list_projects().await.unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].id, second.id, "newest created_at lists first");
        assert_eq!(projects[1].id, created.id);

        // Equal created_at ties break by id ASC: seed two rows sharing one
        // (older) timestamp, in reverse id order, and expect id-ascending.
        for tie_id in ["tie-b", "tie-a"] {
            sqlx::query(
                "INSERT INTO agent_projects (id, path, name, created_at, updated_at) \
                 VALUES ($1, $2, $3, 1, 1)",
            )
            .bind(tie_id)
            .bind(format!("/tmp/workspace/{tie_id}"))
            .bind(tie_id)
            .execute(db_arc.pool())
            .await
            .unwrap();
        }
        let projects = repo.list_projects().await.unwrap();
        let ids: Vec<&str> = projects.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![second.id.as_str(), created.id.as_str(), "tie-a", "tie-b"],
            "created_at DESC overall, id ASC within equal created_at"
        );

        // Rename bumps updated_at and keeps created_at / path.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        repo.rename_project(&created.id, "alpha-renamed")
            .await
            .unwrap();
        let renamed = repo.get_project_by_id(&created.id).await.unwrap().unwrap();
        assert_eq!(renamed.name, "alpha-renamed");
        assert_eq!(renamed.path, created.path);
        assert_eq!(renamed.created_at, created.created_at);
        assert!(renamed.updated_at > created.updated_at);

        // Session count: 0 before linking, then reflects linked sessions only.
        assert_eq!(repo.session_count(&created.id).await.unwrap(), 0);
        insert_session(db_arc.as_ref(), Some(&created.id), "s1").await;
        insert_session(db_arc.as_ref(), Some(&created.id), "s2").await;
        insert_session(db_arc.as_ref(), None, "ungrouped").await;
        assert_eq!(repo.session_count(&created.id).await.unwrap(), 2);
        assert_eq!(repo.session_count(&second.id).await.unwrap(), 0);

        // Delete
        repo.delete_project(&created.id).await.unwrap();
        assert!(repo.get_project_by_id(&created.id).await.unwrap().is_none());
    }

    /// VAL-PROJ-016: cascade delete removes the project, its sessions and their
    /// messages atomically — while ungrouped (project_id NULL) sessions and
    /// sibling projects remain fully untouched, message layer included.
    #[tokio::test]
    async fn test_delete_project_cascade_with_isolation() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentProjectRepository::new(db_arc.clone());

        // Force FK enforcement OFF so SQL-level cascade cannot fire: this proves
        // the explicit repository cascade is what removes the three layers.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(db_arc.pool())
            .await
            .unwrap();

        let doomed = repo
            .create_project(&sample_request("/tmp/workspace/doomed", "doomed"))
            .await
            .unwrap();
        let sibling = repo
            .create_project(&sample_request("/tmp/workspace/sibling", "sibling"))
            .await
            .unwrap();

        // doomed: 2 sessions with 2 messages each.
        let doomed_s1 = insert_session(db_arc.as_ref(), Some(&doomed.id), "d1").await;
        let doomed_s2 = insert_session(db_arc.as_ref(), Some(&doomed.id), "d2").await;
        for (i, sid) in [&doomed_s1, &doomed_s2].iter().enumerate() {
            insert_message(db_arc.as_ref(), sid, i as i64).await;
            insert_message(db_arc.as_ref(), sid, i as i64 + 10).await;
        }

        // sibling: 1 session with 1 message.
        let sibling_s = insert_session(db_arc.as_ref(), Some(&sibling.id), "sib").await;
        insert_message(db_arc.as_ref(), &sibling_s, 0).await;

        // ungrouped: 1 session (project_id NULL) with 1 message.
        let ungrouped_s = insert_session(db_arc.as_ref(), None, "ungrouped").await;
        insert_message(db_arc.as_ref(), &ungrouped_s, 0).await;

        // Delete the doomed project.
        repo.delete_project(&doomed.id).await.unwrap();

        // Project row, its sessions, and their messages are all gone.
        assert!(repo.get_project_by_id(&doomed.id).await.unwrap().is_none());
        assert_eq!(count_sessions(db_arc.as_ref(), &doomed.id).await, 0);
        assert_eq!(count_messages(db_arc.as_ref(), &doomed_s1).await, 0);
        assert_eq!(count_messages(db_arc.as_ref(), &doomed_s2).await, 0);

        // Sibling project fully intact: row, session, message.
        let sibling_after = repo.get_project_by_id(&sibling.id).await.unwrap().unwrap();
        assert_eq!(sibling_after.name, "sibling");
        assert_eq!(count_sessions(db_arc.as_ref(), &sibling.id).await, 1);
        assert_eq!(count_messages(db_arc.as_ref(), &sibling_s).await, 1);

        // Ungrouped session (project_id NULL) and its message untouched.
        let ungrouped_count: i64 =
            sqlx::query("SELECT COUNT(*) AS count FROM agent_sessions WHERE project_id IS NULL")
                .fetch_one(db_arc.pool())
                .await
                .unwrap()
                .try_get("count")
                .unwrap();
        assert_eq!(ungrouped_count, 1);
        assert_eq!(count_messages(db_arc.as_ref(), &ungrouped_s).await, 1);
    }

    /// VAL-PROJ-014: delete / rename on a missing id is a clean NOT_FOUND with
    /// zero side effects — double delete included.
    #[tokio::test]
    async fn test_delete_and_rename_missing_id_clean_not_found() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentProjectRepository::new(db_arc.clone());

        let project = repo
            .create_project(&sample_request("/tmp/workspace/keep", "keep"))
            .await
            .unwrap();
        let session = insert_session(db_arc.as_ref(), Some(&project.id), "keep-s").await;
        insert_message(db_arc.as_ref(), &session, 0).await;

        // Rename a never-existed id: clean NOT_FOUND.
        let err = repo
            .rename_project(&"never-existed".to_string(), "x")
            .await
            .unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");

        // Delete a never-existed id: clean NOT_FOUND.
        let err = repo
            .delete_project(&"never-existed".to_string())
            .await
            .unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");

        // Double delete: first succeeds, second is a clean NOT_FOUND.
        let doomed = repo
            .create_project(&sample_request("/tmp/workspace/doomed", "doomed"))
            .await
            .unwrap();
        repo.delete_project(&doomed.id).await.unwrap();
        let err = repo.delete_project(&doomed.id).await.unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");

        // Zero side effects: the bystander project, its session and message
        // are exactly as before.
        let kept = repo.get_project_by_id(&project.id).await.unwrap().unwrap();
        assert_eq!(kept.name, "keep");
        assert_eq!(kept.updated_at, project.updated_at);
        assert_eq!(count_sessions(db_arc.as_ref(), &project.id).await, 1);
        assert_eq!(count_messages(db_arc.as_ref(), &session).await, 1);
    }

    /// VAL-PROJ-019: two concurrent creates with the same path yield exactly
    /// one row; both callers get the same project and no constraint error
    /// surfaces. A later create with the same path returns the existing row
    /// unchanged (name / created_at / updated_at not overwritten).
    #[tokio::test]
    async fn test_concurrent_create_same_path_single_row() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentProjectRepository::new(db_arc.clone());

        let path = "/tmp/workspace/racy";
        let repo_a = repo.clone();
        let repo_b = repo.clone();

        // Race two creates against the same path on the shared pool.
        let req_a = sample_request(path, "from-a");
        let req_b = sample_request(path, "from-b");
        let (a, b) = tokio::join!(repo_a.create_project(&req_a), repo_b.create_project(&req_b),);
        let a = a.expect("concurrent create A must not surface a constraint error");
        let b = b.expect("concurrent create B must not surface a constraint error");

        // Both callers see the same single row.
        assert_eq!(a.id, b.id);
        assert_eq!(a.name, b.name);
        assert_eq!(a.created_at, b.created_at);
        assert_eq!(count_projects_by_path(db_arc.as_ref(), path).await, 1);

        // A later create with the same path returns the existing row unchanged:
        // neither name nor timestamps are overwritten.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let again = repo
            .create_project(&sample_request(path, "different-name"))
            .await
            .unwrap();
        assert_eq!(again.id, a.id);
        assert_eq!(again.name, a.name);
        assert_eq!(again.created_at, a.created_at);
        assert_eq!(again.updated_at, a.updated_at);
        assert_eq!(count_projects_by_path(db_arc.as_ref(), path).await, 1);
    }
}
