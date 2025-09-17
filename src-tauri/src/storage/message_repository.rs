// Message 数据访问层

use crate::models::{AppError, Message, MessageRole, UUID};
use crate::services::DatabaseService;
use sqlx::Row;
use std::sync::Arc;

/// Message 仓储层
#[derive(Clone)]
pub struct MessageRepository {
    db: Arc<DatabaseService>,
}

impl MessageRepository {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self { db }
    }

    /// 创建消息
    pub async fn create_message(&self, message: &Message) -> Result<(), AppError> {
        let attachments_json =
            if let Some(attachments) = &message.attachments {
                Some(serde_json::to_string(attachments).map_err(|e| {
                    AppError::validation_error(&format!("Invalid attachments: {}", e))
                })?)
            } else {
                None
            };

        let config_json = if let Some(config) = &message.config {
            Some(
                serde_json::to_string(config)
                    .map_err(|e| AppError::validation_error(&format!("Invalid config: {}", e)))?,
            )
        } else {
            None
        };

        let role_str = match message.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        };

        let query = r#"
            INSERT INTO messages (id, chat_id, role, content, reasoning, config, attachments, 
                                input_tokens, output_tokens, total_tokens, start_time, 
                                end_time, duration, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#;

        sqlx::query(query)
            .bind(&message.id)
            .bind(&message.chat_id)
            .bind(role_str)
            .bind(&message.content)
            .bind(&message.reasoning)
            .bind(&config_json)
            .bind(&attachments_json)
            .bind(message.input_tokens)
            .bind(message.output_tokens)
            .bind(message.total_tokens)
            .bind(message.start_time)
            .bind(message.end_time)
            .bind(message.duration)
            .bind(message.created_at)
            .bind(message.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create message: {}", e)))?;

        Ok(())
    }

    /// 获取聊天的消息列表
    pub async fn get_messages_by_chat(
        &self,
        chat_id: &UUID,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Message>, AppError> {
        let query = r#"
            SELECT id, chat_id, role, content, reasoning, config, attachments, input_tokens, output_tokens, 
                   total_tokens, start_time, end_time, duration, created_at, updated_at
            FROM messages WHERE chat_id = $1 ORDER BY created_at ASC LIMIT $2 OFFSET $3
        "#;

        let rows = sqlx::query(query)
            .bind(chat_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get messages: {}", e)))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(self.row_to_message(row)?);
        }

        Ok(messages)
    }

    /// 根据 ID 获取消息
    pub async fn get_message_by_id(&self, message_id: &UUID) -> Result<Option<Message>, AppError> {
        let query = r#"
            SELECT id, chat_id, role, content, reasoning, config, attachments, input_tokens, output_tokens, 
                   total_tokens, start_time, end_time, duration, created_at, updated_at
            FROM messages WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(message_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get message: {}", e)))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_message(row)?))
        } else {
            Ok(None)
        }
    }

    /// 更新消息内容
    pub async fn update_message_content(
        &self,
        message_id: &UUID,
        content: &str,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let query = "UPDATE messages SET content = $1, updated_at = $2 WHERE id = $3";

        let result = sqlx::query(query)
            .bind(content)
            .bind(updated_at)
            .bind(message_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update message: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Message not found: {}",
                message_id
            )));
        }

        Ok(())
    }

    /// 更新消息的令牌使用情况和时间统计
    pub async fn update_message_stats(
        &self,
        message_id: &UUID,
        input_tokens: Option<i32>,
        output_tokens: Option<i32>,
        total_tokens: Option<i32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        duration: Option<i32>,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let query = r#"
            UPDATE messages SET input_tokens = $1, output_tokens = $2, total_tokens = $3, 
                              start_time = $4, end_time = $5, duration = $6, updated_at = $7
            WHERE id = $8
        "#;

        sqlx::query(query)
            .bind(input_tokens)
            .bind(output_tokens)
            .bind(total_tokens)
            .bind(start_time)
            .bind(end_time)
            .bind(duration)
            .bind(updated_at)
            .bind(message_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update message stats: {}", e))
            })?;

        Ok(())
    }

    /// 删除消息
    pub async fn delete_message(&self, message_id: &UUID) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(message_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete message: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Message not found: {}",
                message_id
            )));
        }

        Ok(())
    }

    /// 删除聊天的所有消息
    pub async fn delete_messages_by_chat(&self, chat_id: &UUID) -> Result<(), AppError> {
        sqlx::query("DELETE FROM messages WHERE chat_id = $1")
            .bind(chat_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete chat messages: {}", e))
            })?;

        Ok(())
    }

    /// 获取聊天的消息数量
    pub async fn get_message_count_by_chat(&self, chat_id: &UUID) -> Result<i32, AppError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM messages WHERE chat_id = $1")
            .bind(chat_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to count messages: {}", e)))?;

        let count: i64 = row.try_get("count")?;
        Ok(count as i32)
    }

    /// 获取聊天的最后一条消息时间
    pub async fn get_last_message_time(&self, chat_id: &UUID) -> Result<Option<i64>, AppError> {
        let row =
            sqlx::query("SELECT MAX(created_at) as last_time FROM messages WHERE chat_id = $1")
                .bind(chat_id)
                .fetch_one(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to get last message time: {}", e))
                })?;

        let last_time: Option<i64> = row.try_get("last_time").ok();
        Ok(last_time)
    }

    // 辅助方法：将数据库行转换为 Message
    fn row_to_message(&self, row: sqlx::sqlite::SqliteRow) -> Result<Message, AppError> {
        let role_str: String = row.try_get("role").unwrap_or_default();
        let role = match role_str.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => MessageRole::User,
        };

        let attachments_json: Option<String> = row.try_get("attachments").ok();
        let attachments = if let Some(json) = attachments_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            None
        };

        let config_json: Option<String> = row.try_get("config").ok();
        let config = if let Some(json) = config_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            None
        };

        Ok(Message {
            id: row.try_get("id").unwrap_or_default(),
            chat_id: row.try_get("chat_id").unwrap_or_default(),
            role,
            content: row.try_get("content").unwrap_or_default(),
            reasoning: row.try_get("reasoning").ok(),
            config,
            attachments,
            input_tokens: row.try_get("input_tokens").ok(),
            output_tokens: row.try_get("output_tokens").ok(),
            total_tokens: row.try_get("total_tokens").ok(),
            start_time: row.try_get("start_time").ok(),
            end_time: row.try_get("end_time").ok(),
            duration: row.try_get("duration").ok(),
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{models::MessageConfig, services::DatabaseService};
    use tempfile::tempdir;

    async fn create_test_db() -> (DatabaseService, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_service = DatabaseService::new(&db_path).await.unwrap();
        (db_service, temp_dir)
    }

    #[tokio::test]
    async fn test_message_crud() {
        let (db_service, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db_service);
        let repo = MessageRepository::new(db_arc.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let chat_id = uuid::Uuid::new_v4().to_string();

        // 先创建一个 chat 以满足外键约束
        let chat_query = r#"
            INSERT INTO chats (id, name, system_prompt, mcp_servers, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#;
        sqlx::query(chat_query)
            .bind(&chat_id)
            .bind("Test Chat")
            .bind(Option::<String>::None)
            .bind("[]")
            .bind(now)
            .bind(now)
            .execute(db_arc.pool())
            .await
            .unwrap();

        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat_id.clone(),
            role: MessageRole::User,
            content: "Hello, world!".to_string(),
            reasoning: None, // 用户消息没有推理过程
            config: Some(MessageConfig {
                temperature: Some(0.7),
                top_p: Some(0.9),
                max_tokens: Some(1000),
                stream: Some(true),
                model_id: Some("gpt-4o".to_string()),
                provider_id: Some("openai".to_string()),
                system_prompt: None,
                mcp_servers: None,
            }),
            attachments: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: now,
            updated_at: now,
        };

        // Create
        repo.create_message(&message).await.unwrap();

        // Get by ID
        let fetched = repo.get_message_by_id(&message.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched_message = fetched.unwrap();
        assert_eq!(fetched_message.content, message.content);
        assert_eq!(fetched_message.role, MessageRole::User);

        // Get by chat
        let messages = repo.get_messages_by_chat(&chat_id, 10, 0).await.unwrap();
        assert_eq!(messages.len(), 1);

        // Update content
        repo.update_message_content(&message.id, "Updated content", now + 1000)
            .await
            .unwrap();

        let updated = repo.get_message_by_id(&message.id).await.unwrap();
        assert_eq!(updated.unwrap().content, "Updated content");

        // Count messages
        let count = repo.get_message_count_by_chat(&chat_id).await.unwrap();
        assert_eq!(count, 1);

        // Delete
        repo.delete_message(&message.id).await.unwrap();
        let deleted = repo.get_message_by_id(&message.id).await.unwrap();
        assert!(deleted.is_none());
    }
}
