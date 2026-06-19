// Session 数据访问层

use crate::models::AppError;
use crate::storage::types::{Session, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Session 仓储层
#[derive(Clone)]
pub struct SessionRepository {
    db: Arc<Database>,
}

/// 把空白的可选外键引用归一化为 `None`，使其以 SQL NULL 形式绑定。
///
/// 空字符串的 `agent_id` / `artifact_id` 语义上等同「无引用」。若按 `""` 绑定，
/// 会触发 `agent_id -> agents(id)` 外键约束失败（不存在 id == "" 的行），导致整行
/// UPDATE（如重命名）整体回滚。配合读取侧将 NULL 正确解码为 `None`，杜绝 `""` 进入
/// 外键列。
fn blank_ref_to_none(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|s| !s.trim().is_empty())
}

impl SessionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建 Session
    pub async fn create_session(&self, session: &Session) -> Result<(), AppError> {
        let mcp_servers_json = serde_json::to_string(&session.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let reasoning_json = session
            .reasoning
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok());

        let query = r#"
            INSERT INTO sessions (id, name, temperature, top_p, top_k, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, artifact_id, agent_id, reasoning, generative_ui, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        "#;

        sqlx::query(query)
            .bind(&session.id)
            .bind(&session.name)
            .bind(session.temperature)
            .bind(session.top_p)
            .bind(session.top_k)
            .bind(session.max_tokens)
            .bind(session.stream)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&mcp_servers_json)
            .bind(session.turn_count)
            .bind(blank_ref_to_none(&session.artifact_id))
            .bind(blank_ref_to_none(&session.agent_id))
            .bind(reasoning_json)
            .bind(session.generative_ui)
            .bind(session.created_at)
            .bind(session.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create session: {}", e)))?;

        Ok(())
    }

    /// 向后兼容：创建聊天
    pub async fn create_chat(&self, chat: &Session) -> Result<(), AppError> {
        self.create_session(chat).await
    }

    /// 获取 Session 列表
    pub async fn list_sessions(&self, limit: i32, offset: i32) -> Result<Vec<Session>, AppError> {
        let query = r#"
            SELECT id, name, last_message_at, message_count, temperature, top_p, top_k, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, artifact_id, agent_id, reasoning, generative_ui, created_at, updated_at
            FROM sessions ORDER BY updated_at DESC LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list sessions: {}", e)))?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(self.row_to_session(row)?);
        }

        Ok(sessions)
    }

    /// 向后兼容：获取聊天列表
    pub async fn list_chats(&self, limit: i32, offset: i32) -> Result<Vec<Session>, AppError> {
        self.list_sessions(limit, offset).await
    }

    /// 根据 ID 获取 Session
    pub async fn get_session_by_id(&self, session_id: &UUID) -> Result<Option<Session>, AppError> {
        let query = r#"
            SELECT id, name, last_message_at, message_count, temperature, top_p, top_k, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, artifact_id, agent_id, reasoning, generative_ui, created_at, updated_at
            FROM sessions WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(session_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get session: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_session(row)?))
        } else {
            Ok(None)
        }
    }

    /// 向后兼容：根据 ID 获取聊天
    pub async fn get_chat_by_id(&self, chat_id: &UUID) -> Result<Option<Session>, AppError> {
        self.get_session_by_id(chat_id).await
    }

    /// 更新 Session
    pub async fn update_session(&self, session: &Session) -> Result<(), AppError> {
        let mcp_servers_json = serde_json::to_string(&session.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let reasoning_json = session
            .reasoning
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok());

        let query = r#"
            UPDATE sessions SET name = $1, temperature = $2, top_p = $3, top_k = $4, max_tokens = $5, stream = $6, model_id = $7, provider_id = $8, system_prompt = $9, mcp_servers = $10, turn_count = $11, artifact_id = $12, agent_id = $13, reasoning = $14, updated_at = $15
            WHERE id = $16
        "#;

        let result = sqlx::query(query)
            .bind(&session.name)
            .bind(session.temperature)
            .bind(session.top_p)
            .bind(session.top_k)
            .bind(session.max_tokens)
            .bind(session.stream)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&mcp_servers_json)
            .bind(session.turn_count)
            .bind(blank_ref_to_none(&session.artifact_id))
            .bind(blank_ref_to_none(&session.agent_id))
            .bind(reasoning_json)
            .bind(session.updated_at)
            .bind(&session.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update session: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Session not found: {}", session.id)));
        }

        Ok(())
    }

    /// 向后兼容：更新聊天
    pub async fn update_chat(&self, chat: &Session) -> Result<(), AppError> {
        self.update_session(chat).await
    }

    /// 删除 Session
    pub async fn delete_session(&self, session_id: &UUID) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(session_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete session: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Session not found: {}", session_id)));
        }

        Ok(())
    }

    /// 向后兼容：删除聊天
    pub async fn delete_chat(&self, chat_id: &UUID) -> Result<(), AppError> {
        self.delete_session(chat_id).await
    }

    /// 更新 Session 的消息统计
    pub async fn update_message_stats(
        &self,
        session_id: &UUID,
        message_count: i32,
        last_message_at: i64,
    ) -> Result<(), AppError> {
        let query = r#"
            UPDATE sessions SET message_count = $1, last_message_at = $2, updated_at = $3
            WHERE id = $4
        "#;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        sqlx::query(query)
            .bind(message_count)
            .bind(last_message_at)
            .bind(now)
            .bind(session_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update message stats: {}", e))
            })?;

        Ok(())
    }

    /// 统计使用指定 MCP 服务器的 Session 数量
    pub async fn count_sessions_using_mcp_server(&self, server_id: &str) -> Result<i32, AppError> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM sessions
            WHERE mcp_servers LIKE '%' || $1 || '%'
        "#;

        let row = sqlx::query(query)
            .bind(server_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count sessions: {}", e)))?;

        let count: i32 = row.try_get("count")?;
        Ok(count)
    }

    /// 向后兼容：统计使用指定 MCP 服务器的聊天数量
    pub async fn count_chats_using_mcp_server(&self, server_id: &str) -> Result<i32, AppError> {
        self.count_sessions_using_mcp_server(server_id).await
    }

    /// 从所有 Session 中移除指定 MCP 服务器的引用
    pub async fn remove_mcp_server_from_sessions(&self, server_id: &str) -> Result<i32, AppError> {
        use crate::storage::types::McpServerConfig;

        // 获取所有包含该服务器的 Session
        let query = r#"
            SELECT id, mcp_servers
            FROM sessions
            WHERE mcp_servers LIKE '%' || $1 || '%'
        "#;

        let rows = sqlx::query(query)
            .bind(server_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to query sessions with MCP server: {}", e))
            })?;

        let mut updated_count = 0;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for row in rows {
            let session_id: String = row.try_get("id")?;
            let mcp_servers_json: Option<String> = row.try_get("mcp_servers")?;

            if let Some(json) = mcp_servers_json {
                let mut mcp_servers: Vec<McpServerConfig> =
                    serde_json::from_str(&json).unwrap_or_default();

                // 移除指定服务器
                let original_len = mcp_servers.len();
                mcp_servers.retain(|config| config.server_id != server_id);

                // 只有在实际移除了服务器时才更新
                if mcp_servers.len() < original_len {
                    let updated_json = serde_json::to_string(&mcp_servers).map_err(|e| {
                        AppError::internal_error(&format!("Failed to serialize MCP servers: {}", e))
                    })?;

                    let update_query = r#"
                        UPDATE sessions
                        SET mcp_servers = $1, updated_at = $2
                        WHERE id = $3
                    "#;

                    sqlx::query(update_query)
                        .bind(&updated_json)
                        .bind(now)
                        .bind(&session_id)
                        .execute(self.db.pool())
                        .await
                        .map_err(|e| {
                            AppError::internal_error(&format!(
                                "Failed to update session MCP servers: {}",
                                e
                            ))
                        })?;

                    updated_count += 1;
                }
            }
        }

        Ok(updated_count)
    }

    /// 向后兼容：从所有聊天中移除指定 MCP 服务器的引用
    pub async fn remove_mcp_server_from_chats(&self, server_id: &str) -> Result<i32, AppError> {
        self.remove_mcp_server_from_sessions(server_id).await
    }

    /// 统计使用指定供应商的 Session 数量
    pub async fn count_sessions_using_provider(&self, provider_id: &str) -> Result<i32, AppError> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM sessions
            WHERE provider_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(provider_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count sessions: {}", e)))?;

        let count: i32 = row.try_get("count")?;
        Ok(count)
    }

    /// 向后兼容：统计使用指定供应商的聊天数量
    pub async fn count_chats_using_provider(&self, provider_id: &str) -> Result<i32, AppError> {
        self.count_sessions_using_provider(provider_id).await
    }

    /// 统计使用指定模型的 Session 数量
    pub async fn count_sessions_using_model(&self, model_id: &str) -> Result<i32, AppError> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM sessions
            WHERE model_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(model_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count sessions: {}", e)))?;

        let count: i32 = row.try_get("count")?;
        Ok(count)
    }

    /// 向后兼容：统计使用指定模型的聊天数量
    pub async fn count_chats_using_model(&self, model_id: &str) -> Result<i32, AppError> {
        self.count_sessions_using_model(model_id).await
    }

    // 辅助方法：将数据库行转换为 Session
    fn row_to_session(&self, row: sqlx::sqlite::SqliteRow) -> Result<Session, AppError> {
        use crate::storage::types::McpServerConfig;

        let mcp_servers_json: Option<String> = row.try_get("mcp_servers")?;
        let mcp_servers: Vec<McpServerConfig> = if let Some(json) = mcp_servers_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        let reasoning = row
            .try_get::<Option<String>, _>("reasoning")?
            .and_then(|raw| match raw.trim() {
                "" => None,
                value => serde_json::from_str(value).ok(),
            });

        // 明确处理 NULL 值：SQLx 要求我们指定要读取 Option<T> 类型
        let temperature: Option<f32> = row.try_get::<Option<f32>, _>("temperature")?;
        let top_p: Option<f32> = row.try_get::<Option<f32>, _>("top_p")?;
        let top_k: Option<i32> = row.try_get::<Option<i32>, _>("top_k")?;
        let max_tokens: Option<i32> = row.try_get::<Option<i32>, _>("max_tokens")?;
        let stream: Option<bool> = row.try_get::<Option<bool>, _>("stream")?;
        // Option<bool> 显式解码：SQL NULL -> None（旧行），INTEGER 0/1 -> Some(false/true)。
        let generative_ui: Option<bool> = row.try_get::<Option<bool>, _>("generative_ui")?;

        Ok(Session {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            last_message_at: row.try_get("last_message_at").ok(),
            message_count: row.try_get("message_count")?,
            temperature,
            top_p,
            top_k,
            max_tokens,
            stream,
            model_id: row.try_get("model_id").ok(),
            provider_id: row.try_get("provider_id").ok(),
            system_prompt: row.try_get("system_prompt").ok(),
            mcp_servers,
            turn_count: row.try_get("turn_count").ok(),
            // 显式按 Option 解码：避免 NULL 经 `try_get::<String>().ok()` 落成
            // `Some("")`（再回写时触发 agents 外键失败）。空白同样视为「无引用」。
            artifact_id: row
                .try_get::<Option<String>, _>("artifact_id")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty()),
            agent_id: row
                .try_get::<Option<String>, _>("agent_id")
                .ok()
                .flatten()
                .filter(|s| !s.trim().is_empty()),
            reasoning,
            generative_ui,
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

    #[tokio::test]
    async fn test_session_crud() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = SessionRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test Session".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(2048),
            stream: Some(true),
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant.".to_string()),
            mcp_servers: vec![
                crate::storage::types::McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec!["tool1".to_string()],
                },
                crate::storage::types::McpServerConfig {
                    server_id: "server2".to_string(),
                    execution_mode: "manual".to_string(),
                    enabled_tools: vec![],
                },
            ],
            turn_count: Some(5),
            artifact_id: None,
            agent_id: None,
            reasoning: None,
            generative_ui: None,
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_session(&session).await.unwrap();

        // Get by ID
        let fetched = repo.get_session_by_id(&session.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched_session = fetched.unwrap();
        assert_eq!(fetched_session.name, session.name);
        assert_eq!(fetched_session.mcp_servers, session.mcp_servers);

        // List
        let sessions = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Update
        let mut updated_session = session.clone();
        updated_session.name = "Updated Session".to_string();
        updated_session.updated_at = now + 1000;

        repo.update_session(&updated_session).await.unwrap();

        let fetched_updated = repo.get_session_by_id(&session.id).await.unwrap();
        assert_eq!(fetched_updated.unwrap().name, "Updated Session");

        // Delete
        repo.delete_session(&session.id).await.unwrap();
        let deleted = repo.get_session_by_id(&session.id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_session_with_agent_id() {
        let (db, _temp_dir) = create_test_db().await;
        let db = Arc::new(db);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 先 seed 被引用的 agent，满足 sessions.agent_id -> agents(id) 外键
        // （sqlx 默认 FK=ON），否则 create_session 会以 787 失败。
        let agent_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO agents (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(&agent_id)
            .bind("Test Agent")
            .bind(now)
            .bind(now)
            .execute(db.pool())
            .await
            .unwrap();

        let repo = SessionRepository::new(db);

        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Session from Agent".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            agent_id: Some(agent_id.clone()),
            reasoning: None,
            generative_ui: None,
            created_at: now,
            updated_at: now,
        };

        repo.create_session(&session).await.unwrap();

        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.agent_id, Some(agent_id));
    }

    // 回归：空字符串的 agent_id / artifact_id 必须归一化为 NULL（读回 None）。
    // 此前整行 UPDATE（重命名 / 自动生成标题）会绑定 ""，触发 agents 外键失败而整体
    // 回滚，导致标题永远写不进去。
    #[tokio::test]
    async fn test_blank_agent_and_artifact_ids_normalize_to_none() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = SessionRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: "新会话".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            stream: None,
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: Some(String::new()),
            agent_id: Some(String::new()),
            reasoning: None,
            generative_ui: None,
            created_at: now,
            updated_at: now,
        };

        // 创建：空白引用落库为 NULL，读回为 None。
        repo.create_session(&session).await.unwrap();
        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.agent_id, None);
        assert_eq!(fetched.artifact_id, None);

        // 重命名（整行 UPDATE）：归一化后不再绑定 ""，可正常持久化。
        let mut renamed = fetched.clone();
        renamed.name = "已生成标题".to_string();
        renamed.updated_at = now + 1000;
        repo.update_session(&renamed).await.unwrap();

        let after = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(after.name, "已生成标题");
        assert_eq!(after.agent_id, None);
        assert_eq!(after.artifact_id, None);
    }

    // VAL-AGENT-009: a session row with generative_ui NULL decodes to None on both
    // read paths (get_session_by_id + list_sessions) without a sqlx decode error.
    #[tokio::test]
    async fn test_session_generative_ui_null_decodes_to_none() {
        let (db, _temp_dir) = create_test_db().await;
        let db = Arc::new(db);
        let repo = SessionRepository::new(db.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Simulate a legacy row: insert directly, omitting generative_ui (stays NULL).
        let session_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO sessions (id, name, message_count, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&session_id)
        .bind("Legacy Session")
        .bind(0_i32)
        .bind(now)
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        let fetched = repo.get_session_by_id(&session_id).await.unwrap().unwrap();
        assert_eq!(fetched.generative_ui, None);

        let listed = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].generative_ui, None);
    }

    // write-once：`update_session` 的 SET 子句故意不含 `generative_ui`，因此一次
    // 整行 UPDATE（即便传入的 session.generative_ui == None）也不能清掉已落库的值。
    // 这把会话级 generative_ui 的「创建时由 Agent 快照、此后只读」语义钉死。
    #[tokio::test]
    async fn test_update_session_does_not_clobber_generative_ui() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = SessionRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Original".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            agent_id: None,
            reasoning: None,
            generative_ui: Some(true),
            created_at: now,
            updated_at: now,
        };

        repo.create_session(&session).await.unwrap();

        // 整行 UPDATE：改名，且故意把 generative_ui 置为 None。
        let mut updated = session.clone();
        updated.name = "Renamed".to_string();
        updated.generative_ui = None;
        updated.updated_at = now + 1000;
        repo.update_session(&updated).await.unwrap();

        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Renamed");
        // 仍为 Some(true)：SET 子句排除了 generative_ui，落库值未被覆盖。
        assert_eq!(fetched.generative_ui, Some(true));
    }
}
