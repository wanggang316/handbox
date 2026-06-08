// Agent Session 数据访问层
//
// Agent 模式会话及其 transcript 的持久化层，建立在 `agent_sessions` /
// `agent_session_messages` 两张表之上。与 Chat 模式的 `session_repository`
// 完全独立。

use crate::models::AppError;
use crate::storage::types::{AgentSession, AgentSessionMessage, Timestamp, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Agent Session 仓储层
#[derive(Clone)]
pub struct AgentSessionRepository {
    db: Arc<Database>,
}

impl AgentSessionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建 Agent Session
    pub async fn create_session(&self, session: &AgentSession) -> Result<(), AppError> {
        let enabled_tools_json = serde_json::to_string(&session.enabled_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid enabled tools: {}", e)))?;

        let query = r#"
            INSERT INTO agent_sessions (id, name, model_id, provider_id, system_prompt, thinking_level, temperature, max_tokens, working_dir, enabled_tools, tool_execution_mode, message_count, last_message_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#;

        sqlx::query(query)
            .bind(&session.id)
            .bind(&session.name)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&session.thinking_level)
            .bind(session.temperature)
            .bind(session.max_tokens)
            .bind(&session.working_dir)
            .bind(&enabled_tools_json)
            .bind(&session.tool_execution_mode)
            .bind(session.message_count)
            .bind(session.last_message_at)
            .bind(session.created_at)
            .bind(session.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create agent session: {}", e))
            })?;

        Ok(())
    }

    /// 获取 Agent Session 列表（按 updated_at 降序）
    pub async fn list_sessions(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<AgentSession>, AppError> {
        let query = r#"
            SELECT id, name, model_id, provider_id, system_prompt, thinking_level, temperature, max_tokens, working_dir, enabled_tools, tool_execution_mode, message_count, last_message_at, created_at, updated_at
            FROM agent_sessions ORDER BY updated_at DESC LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list agent sessions: {}", e))
            })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(self.row_to_session(row)?);
        }

        Ok(sessions)
    }

    /// 根据 ID 获取 Agent Session
    pub async fn get_session_by_id(
        &self,
        session_id: &UUID,
    ) -> Result<Option<AgentSession>, AppError> {
        let query = r#"
            SELECT id, name, model_id, provider_id, system_prompt, thinking_level, temperature, max_tokens, working_dir, enabled_tools, tool_execution_mode, message_count, last_message_at, created_at, updated_at
            FROM agent_sessions WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(session_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get agent session: {}", e))
            })?;

        if let Some(row) = row {
            Ok(Some(self.row_to_session(row)?))
        } else {
            Ok(None)
        }
    }

    /// 更新 Agent Session
    pub async fn update_session(&self, session: &AgentSession) -> Result<(), AppError> {
        let enabled_tools_json = serde_json::to_string(&session.enabled_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid enabled tools: {}", e)))?;

        let query = r#"
            UPDATE agent_sessions SET name = $1, model_id = $2, provider_id = $3, system_prompt = $4, thinking_level = $5, temperature = $6, max_tokens = $7, working_dir = $8, enabled_tools = $9, tool_execution_mode = $10, message_count = $11, last_message_at = $12, updated_at = $13
            WHERE id = $14
        "#;

        let result = sqlx::query(query)
            .bind(&session.name)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&session.thinking_level)
            .bind(session.temperature)
            .bind(session.max_tokens)
            .bind(&session.working_dir)
            .bind(&enabled_tools_json)
            .bind(&session.tool_execution_mode)
            .bind(session.message_count)
            .bind(session.last_message_at)
            .bind(session.updated_at)
            .bind(&session.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update agent session: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session.id
            )));
        }

        Ok(())
    }

    /// 重命名 Agent Session（同时刷新 updated_at）
    pub async fn rename_session(&self, session_id: &UUID, name: &str) -> Result<(), AppError> {
        let now = Self::now_ms();

        let result =
            sqlx::query("UPDATE agent_sessions SET name = $1, updated_at = $2 WHERE id = $3")
                .bind(name)
                .bind(now)
                .bind(session_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to rename agent session: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            )));
        }

        Ok(())
    }

    /// 删除 Agent Session（显式级联删除其 transcript）
    ///
    /// # 为什么显式删除而不依赖 `ON DELETE CASCADE`？
    ///
    /// 连接池上的 `PRAGMA foreign_keys` 处于 OFF 状态（见 `database.rs`，
    /// 它只设置了 WAL/synchronous/busy_timeout），因此 SQL 级别的级联不会触发。
    /// 这里在同一个事务里先删除全部 `agent_session_messages`，再删除
    /// `agent_sessions` 行，保证不会留下孤儿 transcript 行。
    pub async fn delete_session(&self, session_id: &UUID) -> Result<(), AppError> {
        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to begin transaction: {}", e))
        })?;

        // 1. 先删除该会话的全部 transcript 行（显式级联）
        sqlx::query("DELETE FROM agent_session_messages WHERE session_id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete agent session messages: {}", e))
            })?;

        // 2. 再删除会话行本身
        let result = sqlx::query("DELETE FROM agent_sessions WHERE id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete agent session: {}", e))
            })?;

        if result.rows_affected() == 0 {
            // 会话不存在：回滚（此时无任何写入），返回 NotFound。
            // transcript 删除是按 session_id 限定的，因此其它会话不受影响。
            tx.rollback().await.map_err(|e| {
                AppError::internal_error(&format!("Failed to rollback transaction: {}", e))
            })?;
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            )));
        }

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// 向 transcript 追加一条消息。
    ///
    /// 在同一个事务内：
    /// 1. 为该会话分配 gap-free 单调递增的 `seq`（`COALESCE(MAX(seq), -1) + 1`，从 0 起）。
    /// 2. 插入消息行。
    /// 3. 更新会话的 `message_count`(+1)、`last_message_at`、`updated_at`。
    ///
    /// 返回完整写入的 `AgentSessionMessage`。
    pub async fn append_message(
        &self,
        session_id: &UUID,
        role: &str,
        payload: &serde_json::Value,
        created_at: Timestamp,
    ) -> Result<AgentSessionMessage, AppError> {
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| AppError::validation_error(&format!("Invalid payload: {}", e)))?;

        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to begin transaction: {}", e))
        })?;

        // 1. 计算下一个 seq（gap-free，从 0 起）
        let seq_row = sqlx::query(
            "SELECT COALESCE(MAX(seq), -1) + 1 AS next_seq FROM agent_session_messages WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to compute next seq: {}", e)))?;

        let seq: i64 = seq_row.try_get("next_seq")?;

        // 2. 插入消息
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO agent_session_messages (id, session_id, seq, role, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(&id)
        .bind(session_id)
        .bind(seq)
        .bind(role)
        .bind(&payload_json)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to append agent session message: {}", e))
        })?;

        // 3. 更新会话计数与时间戳（同一逻辑操作）
        sqlx::query(
            r#"
            UPDATE agent_sessions
            SET message_count = message_count + 1, last_message_at = $1, updated_at = $1
            WHERE id = $2
        "#,
        )
        .bind(created_at)
        .bind(session_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to update agent session counters: {}", e))
        })?;

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(AgentSessionMessage {
            id,
            session_id: session_id.clone(),
            seq,
            role: role.to_string(),
            payload: payload.clone(),
            created_at,
        })
    }

    /// 获取某个会话的全部 transcript（按 seq 升序）
    pub async fn list_messages(
        &self,
        session_id: &UUID,
    ) -> Result<Vec<AgentSessionMessage>, AppError> {
        let query = r#"
            SELECT id, session_id, seq, role, payload, created_at
            FROM agent_session_messages WHERE session_id = $1 ORDER BY seq ASC
        "#;

        let rows = sqlx::query(query)
            .bind(session_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list agent session messages: {}", e))
            })?;

        // 逐行隔离 payload 解析（VAL-PERSIST-012）：单条 payload 的存储 JSON 文本
        // 损坏（例如被外部写入非法 JSON）时记录并跳过该行，而非让整批 transcript
        // 加载失败、白屏整条 timeline；其余行照常返回，保持 seq 升序。
        let mut messages = Vec::new();
        for row in rows {
            match Self::row_to_message(row) {
                Ok(Some(message)) => messages.push(message),
                Ok(None) => {}           // 坏 payload 行：已记录并跳过
                Err(e) => return Err(e), // 真实的 DB 列读取故障：照常冒泡
            }
        }

        Ok(messages)
    }

    /// 当前时间（毫秒）
    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    // 辅助方法：将数据库行转换为 AgentSession
    fn row_to_session(&self, row: sqlx::sqlite::SqliteRow) -> Result<AgentSession, AppError> {
        let enabled_tools_json: Option<String> = row.try_get("enabled_tools")?;
        let enabled_tools: Vec<String> = if let Some(json) = enabled_tools_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        let temperature: Option<f32> = row.try_get::<Option<f32>, _>("temperature")?;
        let max_tokens: Option<i32> = row.try_get::<Option<i32>, _>("max_tokens")?;

        Ok(AgentSession {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            model_id: row.try_get("model_id").ok(),
            provider_id: row.try_get("provider_id").ok(),
            system_prompt: row.try_get("system_prompt").ok(),
            thinking_level: row.try_get("thinking_level").ok(),
            temperature,
            max_tokens,
            working_dir: row.try_get("working_dir").ok(),
            enabled_tools,
            tool_execution_mode: row.try_get("tool_execution_mode").ok(),
            message_count: row.try_get("message_count")?,
            last_message_at: row.try_get("last_message_at").ok(),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    // 辅助方法：将数据库行转换为 AgentSessionMessage。
    //
    // 区分两类失败：DB 列读取错误（基础设施故障）冒泡为 `Err`；唯独 payload 存储
    // 的 JSON 文本损坏（无法 parse）返回 `Ok(None)`，由 `list_messages` 跳过该行，
    // 以实现损坏行的优雅降级（VAL-PERSIST-012）。
    fn row_to_message(
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<Option<AgentSessionMessage>, AppError> {
        let id: UUID = row.try_get("id")?;
        let session_id: UUID = row.try_get("session_id")?;
        let seq: i64 = row.try_get("seq")?;
        let role: String = row.try_get("role")?;
        let created_at: Timestamp = row.try_get("created_at")?;
        let payload_json: String = row.try_get("payload")?;

        let payload: serde_json::Value = match serde_json::from_str(&payload_json) {
            Ok(value) => value,
            Err(e) => {
                eprintln!(
                    "Skipping corrupt agent transcript row (session_id={}, seq={}): {}",
                    session_id, seq, e
                );
                return Ok(None);
            }
        };

        Ok(Some(AgentSessionMessage {
            id,
            session_id,
            seq,
            role,
            payload,
            created_at,
        }))
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

    fn sample_session(id: &str, name: &str, now: i64) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            name: name.to_string(),
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a coding agent.".to_string()),
            thinking_level: Some("high".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            working_dir: Some("/tmp/project".to_string()),
            enabled_tools: vec!["read".to_string(), "write".to_string()],
            tool_execution_mode: Some("auto".to_string()),
            message_count: 0,
            last_message_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 统计某会话的 transcript 行数（用于断言无孤儿行）
    async fn count_messages(db: &Database, session_id: &str) -> i64 {
        let row = sqlx::query(
            "SELECT COUNT(*) AS count FROM agent_session_messages WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        row.try_get::<i64, _>("count").unwrap()
    }

    #[tokio::test]
    async fn test_agent_session_crud_roundtrip() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Coding Session", now);

        // Create
        repo.create_session(&session).await.unwrap();

        // Get by ID
        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, session.name);
        assert_eq!(fetched.model_id, session.model_id);
        assert_eq!(fetched.enabled_tools, session.enabled_tools);
        assert_eq!(fetched.thinking_level, session.thinking_level);
        assert_eq!(fetched.message_count, 0);

        // List
        let sessions = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Update
        let mut updated = session.clone();
        updated.name = "Renamed Session".to_string();
        updated.enabled_tools = vec!["read".to_string()];
        updated.updated_at = now + 1000;
        repo.update_session(&updated).await.unwrap();

        let fetched_updated = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched_updated.name, "Renamed Session");
        assert_eq!(fetched_updated.enabled_tools, vec!["read".to_string()]);

        // Rename
        repo.rename_session(&session.id, "Final Name")
            .await
            .unwrap();
        let after_rename = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(after_rename.name, "Final Name");

        // Delete
        repo.delete_session(&session.id).await.unwrap();
        assert!(repo.get_session_by_id(&session.id).await.unwrap().is_none());
    }

    /// VAL-PERSIST-009: delete_session removes session AND all transcript rows
    /// even with `PRAGMA foreign_keys` OFF (explicit delete, not FK cascade).
    #[tokio::test]
    async fn test_delete_session_explicit_cascade_with_fk_off() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        // Force FK enforcement OFF on this connection so the SQL-level
        // `ON DELETE CASCADE` cannot fire. This proves the explicit
        // repository cascade — not the FK — is what removes the transcript.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(db_arc.pool())
            .await
            .unwrap();
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(db_arc.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(fk, 0, "FK enforcement must be OFF for this test");

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Cascade Session", now);
        repo.create_session(&session).await.unwrap();

        // Append N messages.
        for i in 0..5 {
            repo.append_message(
                &session.id,
                "user",
                &serde_json::json!({ "text": format!("msg {}", i) }),
                now + i,
            )
            .await
            .unwrap();
        }
        assert_eq!(count_messages(db_arc.as_ref(), &session.id).await, 5);

        // Delete the session.
        repo.delete_session(&session.id).await.unwrap();

        // Session gone AND zero orphan transcript rows.
        assert!(repo.get_session_by_id(&session.id).await.unwrap().is_none());
        assert_eq!(
            count_messages(db_arc.as_ref(), &session.id).await,
            0,
            "explicit cascade must leave zero orphan transcript rows"
        );
    }

    /// VAL-PERSIST-010: deleting session A leaves session B's
    /// message_count / last_message_at exactly unchanged.
    #[tokio::test]
    async fn test_delete_session_sibling_isolation() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session_a = sample_session(&uuid::Uuid::new_v4().to_string(), "Session A", now);
        let session_b = sample_session(&uuid::Uuid::new_v4().to_string(), "Session B", now);
        repo.create_session(&session_a).await.unwrap();
        repo.create_session(&session_b).await.unwrap();

        // A gets 2 messages, B gets 3 (with distinct timestamps).
        for i in 0..2 {
            repo.append_message(
                &session_a.id,
                "user",
                &serde_json::json!({ "i": i }),
                now + i,
            )
            .await
            .unwrap();
        }
        for i in 0..3 {
            repo.append_message(
                &session_b.id,
                "user",
                &serde_json::json!({ "i": i }),
                now + 100 + i,
            )
            .await
            .unwrap();
        }

        let b_before = repo
            .get_session_by_id(&session_b.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b_before.message_count, 3);
        assert_eq!(b_before.last_message_at, Some(now + 102));

        // Delete A.
        repo.delete_session(&session_a.id).await.unwrap();

        // B unchanged in count / last_message_at, and B's transcript intact.
        let b_after = repo
            .get_session_by_id(&session_b.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b_after.message_count, b_before.message_count);
        assert_eq!(b_after.last_message_at, b_before.last_message_at);
        assert_eq!(count_messages(db_arc.as_ref(), &session_b.id).await, 3);

        // A and its transcript are gone.
        assert!(repo
            .get_session_by_id(&session_a.id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(count_messages(db_arc.as_ref(), &session_a.id).await, 0);
    }

    /// VAL-SESSION-015: deleting an already-removed id is a clean NotFound,
    /// no panic, no orphan rows, and other sessions are unaffected.
    #[tokio::test]
    async fn test_delete_session_double_delete_is_clean() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Doomed", now);
        let bystander = sample_session(&uuid::Uuid::new_v4().to_string(), "Bystander", now);
        repo.create_session(&session).await.unwrap();
        repo.create_session(&bystander).await.unwrap();

        repo.append_message(&session.id, "user", &serde_json::json!({ "x": 1 }), now)
            .await
            .unwrap();
        repo.append_message(&bystander.id, "user", &serde_json::json!({ "y": 1 }), now)
            .await
            .unwrap();

        // First delete succeeds.
        repo.delete_session(&session.id).await.unwrap();

        // Second delete: clean NotFound, no panic.
        let err = repo.delete_session(&session.id).await.unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");

        // No orphan rows for the removed session.
        assert_eq!(count_messages(db_arc.as_ref(), &session.id).await, 0);

        // Bystander untouched.
        let bystander_after = repo
            .get_session_by_id(&bystander.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bystander_after.message_count, 1);
        assert_eq!(count_messages(db_arc.as_ref(), &bystander.id).await, 1);

        // Deleting a never-existed id is also a clean NotFound.
        let err2 = repo
            .delete_session(&"never-existed".to_string())
            .await
            .unwrap_err();
        assert_eq!(err2.code, "NOT_FOUND");
    }

    /// append_message assigns gap-free monotonic seq per session, persisting
    /// across reload; list_messages returns them ordered by seq.
    #[tokio::test]
    async fn test_append_message_seq_monotonic_gap_free() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Seq Session", now);
        repo.create_session(&session).await.unwrap();

        // Append 4 messages; seq should be 0,1,2,3 (gap-free, starting at 0).
        let mut returned_seqs = Vec::new();
        for i in 0..4 {
            let msg = repo
                .append_message(&session.id, "user", &serde_json::json!({ "n": i }), now + i)
                .await
                .unwrap();
            returned_seqs.push(msg.seq);
        }
        assert_eq!(returned_seqs, vec![0, 1, 2, 3]);

        // Reload via a fresh repo over the same DB (persistence across reload).
        let repo2 = AgentSessionRepository::new(db_arc.clone());
        let messages = repo2.list_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), 4);

        // Ordered by seq, gap-free, monotonic.
        let seqs: Vec<i64> = messages.iter().map(|m| m.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3]);

        // Payload round-trips intact, and session counters reflect appends.
        assert_eq!(messages[2].payload, serde_json::json!({ "n": 2 }));
        let reloaded_session = repo2.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(reloaded_session.message_count, 4);
        assert_eq!(reloaded_session.last_message_at, Some(now + 3));
    }

    /// VAL-PERSIST-007/008: a long transcript (>200 messages) loads completely
    /// in strict seq order — no silent truncation / pagination in list_messages.
    #[tokio::test]
    async fn test_list_messages_long_transcript_full_no_truncation() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Long Session", now);
        repo.create_session(&session).await.unwrap();

        let total = 250;
        for i in 0..total {
            repo.append_message(
                &session.id,
                "user",
                &serde_json::json!({ "role": "user", "content": format!("m{}", i) }),
                now + i,
            )
            .await
            .unwrap();
        }

        // Reload via a fresh repo: full count, strictly monotonic gap-free seq.
        let repo2 = AgentSessionRepository::new(db_arc.clone());
        let messages = repo2.list_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), total as usize);

        let seqs: Vec<i64> = messages.iter().map(|m| m.seq).collect();
        let expected: Vec<i64> = (0..total).collect();
        assert_eq!(seqs, expected);
    }

    /// VAL-PERSIST-012: a row whose stored payload is malformed JSON is skipped
    /// on load; the rest of the transcript still returns (graceful degrade, no
    /// whole-batch failure / white screen).
    #[tokio::test]
    async fn test_list_messages_skips_corrupt_payload_row() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Corrupt Session", now);
        repo.create_session(&session).await.unwrap();

        // Two well-formed rows (seq 0, 2) bracketing one corrupt row (seq 1).
        repo.append_message(
            &session.id,
            "user",
            &serde_json::json!({ "role": "user", "content": "first" }),
            now,
        )
        .await
        .unwrap();

        // Inject a row whose payload column holds non-JSON text directly.
        sqlx::query(
            r#"
            INSERT INTO agent_session_messages (id, session_id, seq, role, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session.id)
        .bind(1_i64)
        .bind("assistant")
        .bind("{not valid json")
        .bind(now + 1)
        .execute(db_arc.pool())
        .await
        .unwrap();

        repo.append_message(
            &session.id,
            "user",
            &serde_json::json!({ "role": "user", "content": "third" }),
            now + 2,
        )
        .await
        .unwrap();

        // list_messages must NOT error; it returns the two valid rows, skipping
        // the corrupt one, in seq order.
        let messages = repo.list_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].seq, 0);
        assert_eq!(messages[1].seq, 2);
        assert_eq!(
            messages[0].payload,
            serde_json::json!({ "role": "user", "content": "first" })
        );
        assert_eq!(
            messages[1].payload,
            serde_json::json!({ "role": "user", "content": "third" })
        );
    }
}
