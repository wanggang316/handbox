// Message 数据访问层

use crate::llm_client::types::ChatMessageRole;
use crate::llm_client::ChatToolCall;
use crate::models::{AppError, Message, MessageConfig, Timestamp, UUID};
use crate::storage::Database;
use serde_json;
use sqlx::query::Query;
use sqlx::sqlite::SqliteArguments;
use sqlx::{Row, Sqlite};
use std::sync::Arc;

/// Message 仓储层
#[derive(Clone)]
pub struct MessageRepository {
    db: Arc<Database>,
}

impl MessageRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// 序列化 attachments 字段
    fn serialize_attachments(message: &Message) -> Result<Option<String>, AppError> {
        message
            .attachments
            .as_ref()
            .map(|attachments| {
                serde_json::to_string(attachments)
                    .map_err(|e| AppError::validation_error(&format!("Invalid attachments: {}", e)))
            })
            .transpose()
    }

    /// 序列化 config 字段
    fn serialize_config(message: &Message) -> Result<Option<String>, AppError> {
        message
            .config
            .as_ref()
            .map(|config| {
                serde_json::to_string(config)
                    .map_err(|e| AppError::validation_error(&format!("Invalid config: {}", e)))
            })
            .transpose()
    }

    /// 序列化 tool_calls 字段
    fn serialize_tool_calls(message: &Message) -> Result<Option<String>, AppError> {
        message
            .tool_calls
            .as_ref()
            .map(|tools| {
                serde_json::to_string(tools)
                    .map_err(|e| AppError::validation_error(&format!("Invalid tools: {}", e)))
            })
            .transpose()
    }

    /// 创建并绑定插入消息的 SQL 查询
    ///
    /// # 为什么要分离序列化和绑定？
    ///
    /// 由于 Rust 的生命周期限制，我们不能在函数内部创建 JSON 字符串、
    /// 借用它们给 Query、然后返回 Query 和字符串的所有权。
    /// 因此必须让调用方持有 JSON 字符串，Query 只借用它们的引用。
    fn create_insert_query<'a>(
        message: &'a Message,
        attachments_json: &'a Option<String>,
        config_json: &'a Option<String>,
        tools_json: &'a Option<String>,
    ) -> Query<'a, Sqlite, SqliteArguments<'a>> {
        let role_str = message.role.as_str();

        let sql = r#"
            INSERT INTO messages (id, chat_id, role, content, reasoning, config, tools, attachments,
                                input_tokens, output_tokens, total_tokens, start_time,
                                end_time, duration, turn_id, tool_call_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        "#;

        sqlx::query(sql)
            .bind(&message.id)
            .bind(&message.chat_id)
            .bind(role_str)
            .bind(&message.content)
            .bind(&message.reasoning)
            .bind(config_json)
            .bind(tools_json)
            .bind(attachments_json)
            .bind(message.input_tokens)
            .bind(message.output_tokens)
            .bind(message.total_tokens)
            .bind(message.start_time)
            .bind(message.end_time)
            .bind(message.duration)
            .bind(&message.turn_id)
            .bind(&message.tool_call_id)
            .bind(message.created_at)
            .bind(message.updated_at)
    }

    /// 创建消息
    pub async fn create_message(&self, message: &Message) -> Result<(), AppError> {
        // 序列化 JSON 字段
        let attachments_json = Self::serialize_attachments(message)?;
        let config_json = Self::serialize_config(message)?;
        let tools_json = Self::serialize_tool_calls(message)?;

        // 创建并执行查询
        Self::create_insert_query(message, &attachments_json, &config_json, &tools_json)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create message: {}", e)))?;

        Ok(())
    }

    /// 批量创建消息（在一个事务中）
    pub async fn create_messages_batch(&self, messages: &[Message]) -> Result<(), AppError> {
        if messages.is_empty() {
            return Ok(());
        }

        // 开始事务
        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to begin transaction: {}", e))
        })?;

        for message in messages {
            // 序列化 JSON 字段
            let attachments_json = Self::serialize_attachments(message)?;
            let config_json = Self::serialize_config(message)?;
            let tools_json = Self::serialize_tool_calls(message)?;

            // 创建并执行查询
            Self::create_insert_query(message, &attachments_json, &config_json, &tools_json)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to create message in batch: {}", e))
                })?;
        }

        // 提交事务
        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// 获取聊天的消息列表（主方法）
    pub async fn get_messages(
        &self,
        chat_id: &UUID,
        limit: i32,
        offset: i32,
        roles: Option<Vec<ChatMessageRole>>, // 指定要包含的角色，None表示包含所有角色
        order_by_desc: bool,
    ) -> Result<Vec<Message>, AppError> {
        let order_direction = if order_by_desc { "DESC" } else { "ASC" };

        // 构建WHERE条件
        let mut where_conditions = vec!["chat_id = $1".to_string()];
        let mut bind_values = vec![chat_id.as_str()];

        // 如果指定了角色过滤
        if let Some(role_list) = roles {
            if !role_list.is_empty() {
                let role_placeholders: Vec<String> = role_list
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("${}", bind_values.len() + i + 1))
                    .collect();

                where_conditions.push(format!("role IN ({})", role_placeholders.join(", ")));

                for role in role_list {
                    bind_values.push(role.as_str());
                }
            }
        }

        let where_clause = where_conditions.join(" AND ");
        let limit_param = format!("${}", bind_values.len() + 1);
        let offset_param = format!("${}", bind_values.len() + 2);

        let query = format!(
            r#"
            SELECT id, chat_id, role, content, reasoning, config, tools, attachments, input_tokens, output_tokens,
                   total_tokens, start_time, end_time, duration, turn_id, tool_call_id, created_at, updated_at
            FROM messages WHERE {} ORDER BY created_at {} LIMIT {} OFFSET {}
        "#,
            where_clause, order_direction, limit_param, offset_param
        );

        let mut sqlx_query = sqlx::query(&query);
        for value in &bind_values {
            sqlx_query = sqlx_query.bind(value);
        }
        sqlx_query = sqlx_query.bind(limit).bind(offset);

        let rows = sqlx_query
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get messages: {}", e)))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(self.row_to_message(row)?);
        }

        Ok(messages)
    }

    /// 获取聊天的消息列表（不包含工具角色消息）- 用于前端显示
    pub async fn get_messages_by_chat(
        &self,
        chat_id: &UUID,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Message>, AppError> {
        self.get_messages(
            chat_id,
            limit,
            offset,
            Some(vec![
                ChatMessageRole::User,
                ChatMessageRole::Assistant,
                ChatMessageRole::System,
            ]), // 排除 Tool 角色
            false, // 升序排列
        )
        .await
    }

    /// 获取聊天的消息列表（包含所有角色）- 用于内部处理
    pub async fn get_all_messages_by_chat(
        &self,
        chat_id: &UUID,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Message>, AppError> {
        self.get_messages(
            chat_id, limit, offset, None,  // 包含所有角色
            false, // 升序排列
        )
        .await
    }

    /// 获取聊天的消息列表（带角色筛选和排序）
    pub async fn get_messages_by_chat_with_filters(
        &self,
        chat_id: &UUID,
        limit: i32,
        offset: i32,
        role_filter: Option<&ChatMessageRole>,
        order_by_desc: bool,
    ) -> Result<Vec<Message>, AppError> {
        let roles = role_filter.map(|role| vec![role.clone()]);
        self.get_messages(chat_id, limit, offset, roles, order_by_desc)
            .await
    }

    /// 获取最新的指定角色消息
    pub async fn get_latest_message_by_role(
        &self,
        chat_id: &UUID,
        role: &ChatMessageRole,
    ) -> Result<Option<Message>, AppError> {
        let messages = self
            .get_messages(chat_id, 1, 0, Some(vec![role.clone()]), true)
            .await?;

        Ok(messages.into_iter().next())
    }

    /// 根据 ID 获取消息
    pub async fn get_message_by_id(&self, message_id: &UUID) -> Result<Option<Message>, AppError> {
        tracing::debug!(
            "[MessageRepository::get_message_by_id] Querying message with ID: {}",
            message_id
        );

        let query = r#"
            SELECT id, chat_id, role, content, reasoning, config, tools, attachments, input_tokens, output_tokens,
                   total_tokens, start_time, end_time, duration, turn_id, tool_call_id, created_at, updated_at
            FROM messages WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(message_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                tracing::error!(
                    "[MessageRepository::get_message_by_id] Database error for message_id {}: {}",
                    message_id,
                    e
                );
                AppError::internal_error(&format!("Failed to get message: {}", e))
            })?;

        if let Some(row) = row {
            tracing::debug!(
                "[MessageRepository::get_message_by_id] Found message: {}",
                message_id
            );
            Ok(Some(self.row_to_message(row)?))
        } else {
            tracing::warn!(
                "[MessageRepository::get_message_by_id] Message not found: {}",
                message_id
            );
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

    /// 更新消息推理内容
    pub async fn update_message_reasoning(
        &self,
        message_id: &UUID,
        reasoning: Option<&str>,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let query = "UPDATE messages SET reasoning = $1, updated_at = $2 WHERE id = $3";

        let result = sqlx::query(query)
            .bind(reasoning)
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

    /// 更新消息配置
    pub async fn update_message_config(
        &self,
        message_id: &UUID,
        config: Option<&MessageConfig>,
        updated_at: i64,
    ) -> Result<(), AppError> {
        let config_json = if let Some(config) = config {
            Some(
                serde_json::to_string(config)
                    .map_err(|e| AppError::validation_error(&format!("Invalid config: {}", e)))?,
            )
        } else {
            None
        };

        let query = "UPDATE messages SET config = $1, updated_at = $2 WHERE id = $3";

        let result = sqlx::query(query)
            .bind(config_json)
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

    /// 更新消息工具数据
    pub async fn update_message_tools(
        &self,
        message_id: &UUID,
        tools: Option<&Vec<ChatToolCall>>,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        let tools_json = if let Some(tools) = tools {
            Some(
                serde_json::to_string(tools)
                    .map_err(|e| AppError::validation_error(&format!("Invalid tools: {}", e)))?,
            )
        } else {
            None
        };

        let query = "UPDATE messages SET tools = $1, updated_at = $2 WHERE id = $3";
        let result = sqlx::query(query)
            .bind(&tools_json)
            .bind(updated_at)
            .bind(message_id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update message tools: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Message not found: {}",
                message_id
            )));
        }

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

    /// 删除某个消息之后的所有消息（基于 created_at 时间戳）
    pub async fn delete_messages_after(
        &self,
        chat_id: &UUID,
        message_id: &UUID,
    ) -> Result<Vec<String>, AppError> {
        // 首先获取目标消息的创建时间
        let target_message = self
            .get_message_by_id(message_id)
            .await?
            .ok_or_else(|| AppError::not_found(&format!("Message not found: {}", message_id)))?;

        // 先查询要删除的消息ID列表
        let rows = sqlx::query("SELECT id FROM messages WHERE chat_id = $1 AND created_at > $2")
            .bind(chat_id)
            .bind(target_message.created_at)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to query messages to delete: {}", e))
            })?;

        let message_ids: Vec<String> = rows.iter().map(|row| row.get::<String, _>("id")).collect();

        // 如果没有要删除的消息，直接返回
        if message_ids.is_empty() {
            return Ok(message_ids);
        }

        // 直接按 ID 删除（避免重复条件查询）
        let placeholders = message_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let delete_query = format!("DELETE FROM messages WHERE id IN ({})", placeholders);

        let mut query = sqlx::query(&delete_query);
        for id in &message_ids {
            query = query.bind(id);
        }

        query
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete messages: {}", e)))?;

        Ok(message_ids)
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

    /// 根据 turn_id 获取所有相关消息
    pub async fn get_messages_by_turn_id(&self, turn_id: i32) -> Result<Vec<Message>, AppError> {
        let query = r#"
            SELECT id, chat_id, role, content, reasoning, config, tools, attachments, input_tokens, output_tokens,
                   total_tokens, start_time, end_time, duration, turn_id, tool_call_id, created_at, updated_at
            FROM messages WHERE turn_id = $1 ORDER BY created_at ASC
        "#;

        let rows = sqlx::query(query)
            .bind(turn_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get messages by turn_id: {}", e))
            })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(self.row_to_message(row)?);
        }

        Ok(messages)
    }

    /// 根据聊天 ID 和 turn_id 获取消息
    pub async fn get_messages_by_chat_and_turn(
        &self,
        chat_id: &UUID,
        turn_id: i32,
    ) -> Result<Vec<Message>, AppError> {
        let query = r#"
            SELECT id, chat_id, role, content, reasoning, config, tools, attachments, input_tokens, output_tokens,
                   total_tokens, start_time, end_time, duration, turn_id, tool_call_id, created_at, updated_at
            FROM messages WHERE chat_id = $1 AND turn_id = $2 ORDER BY created_at ASC
        "#;

        let rows = sqlx::query(query)
            .bind(chat_id)
            .bind(turn_id)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get messages by chat and turn: {}", e))
            })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(self.row_to_message(row)?);
        }

        Ok(messages)
    }

    /// 获取聊天的下一个 turn_id
    pub async fn get_next_turn_id(&self, chat_id: &UUID) -> Result<i32, AppError> {
        let query =
            "SELECT COALESCE(MAX(turn_id), 0) + 1 as next_turn_id FROM messages WHERE chat_id = $1";

        let row = sqlx::query(query)
            .bind(chat_id)
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get next turn_id: {}", e)))?;

        let next_turn_id: i32 = row.try_get("next_turn_id").map_err(|e| {
            AppError::internal_error(&format!("Failed to parse next_turn_id: {}", e))
        })?;

        Ok(next_turn_id)
    }

    // 辅助方法：将数据库行转换为 Message
    fn row_to_message(&self, row: sqlx::sqlite::SqliteRow) -> Result<Message, AppError> {
        let role_str: String = row.try_get("role").unwrap_or_default();
        let role = role_str
            .parse::<ChatMessageRole>()
            .unwrap_or(ChatMessageRole::User);

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

        let tool_calls: Option<Vec<ChatToolCall>> =
            if let Ok(tools_json) = row.try_get::<String, _>("tools") {
                serde_json::from_str(&tools_json).ok()
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
            tool_calls,
            turn_id: row.try_get("turn_id").ok(),
            tool_call_id: row.try_get("tool_call_id").ok(),
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
    use crate::{models::MessageConfig, storage::Database};
    use tempfile::tempdir;

    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_service = Database::new(&db_path).await.unwrap();
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
            role: ChatMessageRole::User,
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
            tool_calls: None,
            turn_id: Some(1),
            tool_call_id: None,
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
        assert_eq!(fetched_message.role, ChatMessageRole::User);

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

    #[tokio::test]
    async fn test_get_next_turn_id() {
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

        // 第一次调用应该返回 1
        let next_turn_id = repo.get_next_turn_id(&chat_id).await.unwrap();
        assert_eq!(next_turn_id, 1);

        // 创建一条消息
        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: chat_id.clone(),
            role: ChatMessageRole::User,
            content: "Hello".to_string(),
            reasoning: None,
            config: None,
            tool_calls: None,
            turn_id: Some(1),
            tool_call_id: None,
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

        repo.create_message(&message).await.unwrap();

        // 第二次调用应该返回 2
        let next_turn_id = repo.get_next_turn_id(&chat_id).await.unwrap();
        assert_eq!(next_turn_id, 2);
    }

    #[tokio::test]
    async fn test_create_messages_batch() {
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

        // 创建多条消息
        let messages = vec![
            Message {
                id: uuid::Uuid::new_v4().to_string(),
                chat_id: chat_id.clone(),
                role: ChatMessageRole::Tool,
                content: "Tool result 1".to_string(),
                reasoning: None,
                config: None,
                tool_calls: None,
                turn_id: Some(1),
                tool_call_id: Some("tool_1".to_string()),
                attachments: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                start_time: None,
                end_time: None,
                duration: None,
                created_at: now,
                updated_at: now,
            },
            Message {
                id: uuid::Uuid::new_v4().to_string(),
                chat_id: chat_id.clone(),
                role: ChatMessageRole::Tool,
                content: "Tool result 2".to_string(),
                reasoning: None,
                config: None,
                tool_calls: None,
                turn_id: Some(1),
                tool_call_id: Some("tool_2".to_string()),
                attachments: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                start_time: None,
                end_time: None,
                duration: None,
                created_at: now + 1,
                updated_at: now + 1,
            },
            Message {
                id: uuid::Uuid::new_v4().to_string(),
                chat_id: chat_id.clone(),
                role: ChatMessageRole::Tool,
                content: "Tool result 3".to_string(),
                reasoning: None,
                config: None,
                tool_calls: None,
                turn_id: Some(1),
                tool_call_id: Some("tool_3".to_string()),
                attachments: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                start_time: None,
                end_time: None,
                duration: None,
                created_at: now + 2,
                updated_at: now + 2,
            },
        ];

        // 批量创建
        repo.create_messages_batch(&messages).await.unwrap();

        // 验证所有消息都已创建
        let all_messages = repo
            .get_all_messages_by_chat(&chat_id, 100, 0)
            .await
            .unwrap();
        assert_eq!(all_messages.len(), 3);
        assert_eq!(all_messages[0].content, "Tool result 1");
        assert_eq!(all_messages[1].content, "Tool result 2");
        assert_eq!(all_messages[2].content, "Tool result 3");

        // 验证每条消息都可以单独获取
        for msg in &messages {
            let fetched = repo.get_message_by_id(&msg.id).await.unwrap();
            assert!(fetched.is_some());
            assert_eq!(fetched.unwrap().content, msg.content);
        }
    }

    #[tokio::test]
    async fn test_create_messages_batch_empty() {
        let (db_service, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db_service);
        let repo = MessageRepository::new(db_arc);

        // 测试空数组不会报错
        let result = repo.create_messages_batch(&[]).await;
        assert!(result.is_ok());
    }
}
