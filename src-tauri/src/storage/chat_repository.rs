// Chat 数据访问层

use crate::models::AppError;
use crate::storage::types::{Chat, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Chat 仓储层
#[derive(Clone)]
pub struct ChatRepository {
    db: Arc<Database>,
}

impl ChatRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 创建聊天
    pub async fn create_chat(&self, chat: &Chat) -> Result<(), AppError> {
        let mcp_servers_json = serde_json::to_string(&chat.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let query = r#"
            INSERT INTO chats (id, name, temperature, top_p, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#;

        sqlx::query(query)
            .bind(&chat.id)
            .bind(&chat.name)
            .bind(chat.temperature)
            .bind(chat.top_p)
            .bind(chat.max_tokens)
            .bind(chat.stream)
            .bind(&chat.model_id)
            .bind(&chat.provider_id)
            .bind(&chat.system_prompt)
            .bind(&mcp_servers_json)
            .bind(chat.turn_count)
            .bind(chat.created_at)
            .bind(chat.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create chat: {}", e)))?;

        Ok(())
    }

    /// 获取聊天列表
    pub async fn list_chats(&self, limit: i32, offset: i32) -> Result<Vec<Chat>, AppError> {
        let query = r#"
            SELECT id, name, last_message_at, message_count, temperature, top_p, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, artifact_id, created_at, updated_at
            FROM chats ORDER BY updated_at DESC LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list chats: {}", e)))?;

        let mut chats = Vec::new();
        for row in rows {
            chats.push(self.row_to_chat(row)?);
        }

        Ok(chats)
    }

    /// 根据 ID 获取聊天
    pub async fn get_chat_by_id(&self, chat_id: &UUID) -> Result<Option<Chat>, AppError> {
        let query = r#"
            SELECT id, name, last_message_at, message_count, temperature, top_p, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers, turn_count, artifact_id, created_at, updated_at
            FROM chats WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(chat_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get chat: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_chat(row)?))
        } else {
            Ok(None)
        }
    }

    /// 更新聊天
    pub async fn update_chat(&self, chat: &Chat) -> Result<(), AppError> {
        let mcp_servers_json = serde_json::to_string(&chat.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid MCP servers: {}", e)))?;

        let query = r#"
            UPDATE chats SET name = $1, temperature = $2, top_p = $3, max_tokens = $4, stream = $5, model_id = $6, provider_id = $7, system_prompt = $8, mcp_servers = $9, turn_count = $10, updated_at = $11
            WHERE id = $12
        "#;

        let result = sqlx::query(query)
            .bind(&chat.name)
            .bind(chat.temperature)
            .bind(chat.top_p)
            .bind(chat.max_tokens)
            .bind(chat.stream)
            .bind(&chat.model_id)
            .bind(&chat.provider_id)
            .bind(&chat.system_prompt)
            .bind(&mcp_servers_json)
            .bind(chat.turn_count)
            .bind(chat.updated_at)
            .bind(&chat.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update chat: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Chat not found: {}", chat.id)));
        }

        Ok(())
    }

    /// 删除聊天
    pub async fn delete_chat(&self, chat_id: &UUID) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM chats WHERE id = $1")
            .bind(chat_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete chat: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Chat not found: {}", chat_id)));
        }

        Ok(())
    }

    /// 更新聊天的消息统计
    pub async fn update_message_stats(
        &self,
        chat_id: &UUID,
        message_count: i32,
        last_message_at: i64,
    ) -> Result<(), AppError> {
        let query = r#"
            UPDATE chats SET message_count = $1, last_message_at = $2, updated_at = $3
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
            .bind(chat_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update message stats: {}", e))
            })?;

        Ok(())
    }

    /// 统计使用指定 MCP 服务器的聊天数量
    pub async fn count_chats_using_mcp_server(&self, server_id: &str) -> Result<i32, AppError> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM chats
            WHERE mcp_servers LIKE '%' || $1 || '%'
        "#;

        let row = sqlx::query(query)
            .bind(server_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count chats: {}", e)))?;

        let count: i32 = row.try_get("count")?;
        Ok(count)
    }

    /// 从所有聊天中移除指定 MCP 服务器的引用
    pub async fn remove_mcp_server_from_chats(&self, server_id: &str) -> Result<i32, AppError> {
        use crate::storage::types::McpServerConfig;

        // 获取所有包含该服务器的聊天
        let query = r#"
            SELECT id, mcp_servers
            FROM chats
            WHERE mcp_servers LIKE '%' || $1 || '%'
        "#;

        let rows = sqlx::query(query)
            .bind(server_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to query chats with MCP server: {}", e))
            })?;

        let mut updated_count = 0;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for row in rows {
            let chat_id: String = row.try_get("id")?;
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
                        UPDATE chats
                        SET mcp_servers = $1, updated_at = $2
                        WHERE id = $3
                    "#;

                    sqlx::query(update_query)
                        .bind(&updated_json)
                        .bind(now)
                        .bind(&chat_id)
                        .execute(self.db.pool())
                        .await
                        .map_err(|e| {
                            AppError::internal_error(&format!(
                                "Failed to update chat MCP servers: {}",
                                e
                            ))
                        })?;

                    updated_count += 1;
                }
            }
        }

        Ok(updated_count)
    }

    /// 统计使用指定供应商的聊天数量
    pub async fn count_chats_using_provider(&self, provider_id: &str) -> Result<i32, AppError> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM chats
            WHERE provider_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(provider_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count chats: {}", e)))?;

        let count: i32 = row.try_get("count")?;
        Ok(count)
    }

    /// 统计使用指定模型的聊天数量
    pub async fn count_chats_using_model(&self, model_id: &str) -> Result<i32, AppError> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM chats
            WHERE model_id = $1
        "#;

        let row = sqlx::query(query)
            .bind(model_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count chats: {}", e)))?;

        let count: i32 = row.try_get("count")?;
        Ok(count)
    }

    // 辅助方法：将数据库行转换为 Chat
    fn row_to_chat(&self, row: sqlx::sqlite::SqliteRow) -> Result<Chat, AppError> {
        use crate::storage::types::McpServerConfig;

        let mcp_servers_json: Option<String> = row.try_get("mcp_servers")?;
        let mcp_servers: Vec<McpServerConfig> = if let Some(json) = mcp_servers_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        // 明确处理 NULL 值：SQLx 要求我们指定要读取 Option<T> 类型
        let temperature: Option<f32> = row.try_get::<Option<f32>, _>("temperature")?;
        let top_p: Option<f32> = row.try_get::<Option<f32>, _>("top_p")?;
        let max_tokens: Option<i32> = row.try_get::<Option<i32>, _>("max_tokens")?;
        let stream: Option<bool> = row.try_get::<Option<bool>, _>("stream")?;

        Ok(Chat {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            last_message_at: row.try_get("last_message_at").ok(),
            message_count: row.try_get("message_count")?,
            temperature,
            top_p,
            max_tokens,
            stream,
            model_id: row.try_get("model_id").ok(),
            provider_id: row.try_get("provider_id").ok(),
            system_prompt: row.try_get("system_prompt").ok(),
            mcp_servers,
            turn_count: row.try_get("turn_count").ok(),
            artifact_id: row.try_get("artifact_id").ok(),
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
    async fn test_chat_crud() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ChatRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let chat = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Test Chat".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: Some(0.7),
            top_p: Some(0.9),
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
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_chat(&chat).await.unwrap();

        // Get by ID
        let fetched = repo.get_chat_by_id(&chat.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched_chat = fetched.unwrap();
        assert_eq!(fetched_chat.name, chat.name);
        assert_eq!(fetched_chat.mcp_servers, chat.mcp_servers);

        // List
        let chats = repo.list_chats(10, 0).await.unwrap();
        assert_eq!(chats.len(), 1);

        // Update
        let mut updated_chat = chat.clone();
        updated_chat.name = "Updated Chat".to_string();
        updated_chat.updated_at = now + 1000;

        repo.update_chat(&updated_chat).await.unwrap();

        let fetched_updated = repo.get_chat_by_id(&chat.id).await.unwrap();
        assert_eq!(fetched_updated.unwrap().name, "Updated Chat");

        // Delete
        repo.delete_chat(&chat.id).await.unwrap();
        let deleted = repo.get_chat_by_id(&chat.id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_count_chats_using_mcp_server() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ChatRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Create chat with mcp server1
        let chat1 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 1".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![crate::storage::types::McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec![],
            }],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with server1 and server2
        let chat2 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 2".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![
                crate::storage::types::McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec![],
                },
                crate::storage::types::McpServerConfig {
                    server_id: "server2".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec![],
                },
            ],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with server2 only
        let chat3 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 3".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![crate::storage::types::McpServerConfig {
                server_id: "server2".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec![],
            }],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat without MCP servers
        let chat4 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 4".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        repo.create_chat(&chat1).await.unwrap();
        repo.create_chat(&chat2).await.unwrap();
        repo.create_chat(&chat3).await.unwrap();
        repo.create_chat(&chat4).await.unwrap();

        // Count chats using server1
        let count1 = repo.count_chats_using_mcp_server("server1").await.unwrap();
        assert_eq!(count1, 2); // chat1 and chat2

        // Count chats using server2
        let count2 = repo.count_chats_using_mcp_server("server2").await.unwrap();
        assert_eq!(count2, 2); // chat2 and chat3

        // Count chats using non-existent server
        let count3 = repo.count_chats_using_mcp_server("server3").await.unwrap();
        assert_eq!(count3, 0);
    }

    #[tokio::test]
    async fn test_count_chats_using_provider() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ChatRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Create chat with provider1
        let chat1 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 1".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some("model1".to_string()),
            provider_id: Some("provider1".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with provider1
        let chat2 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 2".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some("model2".to_string()),
            provider_id: Some("provider1".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with provider2
        let chat3 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 3".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some("model3".to_string()),
            provider_id: Some("provider2".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        repo.create_chat(&chat1).await.unwrap();
        repo.create_chat(&chat2).await.unwrap();
        repo.create_chat(&chat3).await.unwrap();

        // Count chats using provider1
        let count1 = repo.count_chats_using_provider("provider1").await.unwrap();
        assert_eq!(count1, 2);

        // Count chats using provider2
        let count2 = repo.count_chats_using_provider("provider2").await.unwrap();
        assert_eq!(count2, 1);

        // Count chats using non-existent provider
        let count3 = repo.count_chats_using_provider("provider3").await.unwrap();
        assert_eq!(count3, 0);
    }

    #[tokio::test]
    async fn test_count_chats_using_model() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ChatRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Create chat with model1
        let chat1 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 1".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some("model1".to_string()),
            provider_id: Some("provider1".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with model1
        let chat2 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 2".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some("model1".to_string()),
            provider_id: Some("provider1".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with model2
        let chat3 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 3".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some("model2".to_string()),
            provider_id: Some("provider1".to_string()),
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat without model
        let chat4 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 4".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        repo.create_chat(&chat1).await.unwrap();
        repo.create_chat(&chat2).await.unwrap();
        repo.create_chat(&chat3).await.unwrap();
        repo.create_chat(&chat4).await.unwrap();

        // Count chats using model1
        let count1 = repo.count_chats_using_model("model1").await.unwrap();
        assert_eq!(count1, 2);

        // Count chats using model2
        let count2 = repo.count_chats_using_model("model2").await.unwrap();
        assert_eq!(count2, 1);

        // Count chats using non-existent model
        let count3 = repo.count_chats_using_model("model3").await.unwrap();
        assert_eq!(count3, 0);
    }

    #[tokio::test]
    async fn test_remove_mcp_server_from_chats() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = ChatRepository::new(Arc::new(db));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // Create chat with server1 only
        let chat1 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 1".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![crate::storage::types::McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec![],
            }],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with server1 and server2
        let chat2 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 2".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![
                crate::storage::types::McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec![],
                },
                crate::storage::types::McpServerConfig {
                    server_id: "server2".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec![],
                },
            ],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat with server2 only
        let chat3 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 3".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![crate::storage::types::McpServerConfig {
                server_id: "server2".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec![],
            }],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        // Create chat without MCP servers
        let chat4 = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Chat 4".to_string(),
            last_message_at: None,
            message_count: 0,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            mcp_servers: vec![],
            turn_count: None,
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        repo.create_chat(&chat1).await.unwrap();
        repo.create_chat(&chat2).await.unwrap();
        repo.create_chat(&chat3).await.unwrap();
        repo.create_chat(&chat4).await.unwrap();

        // Remove server1 from all chats
        let updated_count = repo.remove_mcp_server_from_chats("server1").await.unwrap();
        assert_eq!(updated_count, 2); // chat1 and chat2 were updated

        // Verify chat1 now has no MCP servers
        let updated_chat1 = repo.get_chat_by_id(&chat1.id).await.unwrap().unwrap();
        assert_eq!(updated_chat1.mcp_servers.len(), 0);

        // Verify chat2 now has only server2
        let updated_chat2 = repo.get_chat_by_id(&chat2.id).await.unwrap().unwrap();
        assert_eq!(updated_chat2.mcp_servers.len(), 1);
        assert_eq!(updated_chat2.mcp_servers[0].server_id, "server2");

        // Verify chat3 still has server2
        let updated_chat3 = repo.get_chat_by_id(&chat3.id).await.unwrap().unwrap();
        assert_eq!(updated_chat3.mcp_servers.len(), 1);
        assert_eq!(updated_chat3.mcp_servers[0].server_id, "server2");

        // Verify chat4 is unchanged
        let updated_chat4 = repo.get_chat_by_id(&chat4.id).await.unwrap().unwrap();
        assert_eq!(updated_chat4.mcp_servers.len(), 0);

        // Count chats still using server1
        let count1 = repo.count_chats_using_mcp_server("server1").await.unwrap();
        assert_eq!(count1, 0);

        // Count chats using server2
        let count2 = repo.count_chats_using_mcp_server("server2").await.unwrap();
        assert_eq!(count2, 2); // chat2 and chat3
    }
}
