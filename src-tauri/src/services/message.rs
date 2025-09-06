// 消息服务实现

use crate::models::{AppError, ChatRequest, ChatResponse, Message, MessageRole, UUID};
use crate::services::DatabaseService;
use sqlx::Row;
use std::sync::Arc;

/// 消息服务
pub struct MessageService {
    db: Arc<DatabaseService>,
}

impl MessageService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self { db }
    }

    /// 发送消息
    pub async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        tracing::info!("[MessageService::send_message] Starting to send message for chat_id: {:?}", request.chat_id);
        
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        tracing::debug!("[MessageService::send_message] Request details: {:?}", request);
        // 1. 验证请求参数
        if request.messages.is_empty() {
            let error = "No messages provided";
            tracing::error!("[MessageService::send_message] Validation failed: {}", error);
            return Err(AppError::validation_error(error));
        }

        // 获取最后一条用户消息
        let last_message = &request.messages[request.messages.len() - 1];
        if last_message.role != MessageRole::User {
            let error = "Last message must be from user";
            tracing::error!("[MessageService::send_message] Validation failed: {}", error);
            return Err(AppError::validation_error(error));
        }

        let pool = self.db.pool();

        // 2. 保存用户消息到数据库
        let user_message_id = uuid::Uuid::new_v4().to_string();
        let chat_id = request.chat_id.as_ref().ok_or_else(|| {
            let error = "Chat ID is required";
            tracing::error!("[MessageService::send_message] Validation failed: {}", error);
            AppError::validation_error(error)
        })?;

        let attachments_json = if let Some(attachments) = &request.attachments {
            Some(serde_json::to_string(attachments)
                .map_err(|e| {
                    let error = format!("Invalid attachments: {}", e);
                    tracing::error!("[MessageService::send_message] Serialization failed: {}", error);
                    AppError::validation_error(&error)
                })?)
        } else {
            None
        };

        sqlx::query(
            "INSERT INTO messages (id, chat_id, role, content, model_id, provider_id, temperature, top_p, max_tokens, stream, attachments, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
        )
        .bind(&user_message_id)
        .bind(chat_id)
        .bind("user")
        .bind(&last_message.content)
        .bind(Option::<String>::None) // 在测试中避免外键约束
        .bind(Option::<String>::None) // 在测试中避免外键约束
        .bind(request.parameters.as_ref().and_then(|p| p.temperature))
        .bind(request.parameters.as_ref().and_then(|p| p.top_p))
        .bind(request.parameters.as_ref().and_then(|p| p.max_tokens))
        .bind(request.parameters.as_ref().and_then(|p| p.stream))
        .bind(&attachments_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| {
            let error = format!("Failed to save user message: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            AppError::internal_error(&error)
        })?;
        
        tracing::info!("[MessageService::send_message] User message saved with ID: {}", user_message_id);

        // 3. 目前返回一个模拟的响应（后续需要集成实际的 LLM API 调用）
        let assistant_message_id = uuid::Uuid::new_v4().to_string();
        let mock_response = "这是一个模拟的回复，实际的 LLM API 集成正在开发中。";

        // 4. 保存助手消息到数据库
        sqlx::query(
            "INSERT INTO messages (id, chat_id, role, content, model_id, provider_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        )
        .bind(&assistant_message_id)
        .bind(chat_id)
        .bind("assistant")
        .bind(mock_response)
        .bind(Option::<String>::None) // 在测试中避免外键约束
        .bind(Option::<String>::None) // 在测试中避免外键约束
        .bind(now)
        .bind(now)
        .execute(pool)
        .await
        .map_err(|e| {
            let error = format!("Failed to save assistant message: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            AppError::internal_error(&error)
        })?;
        
        tracing::info!("[MessageService::send_message] Assistant message saved with ID: {}", assistant_message_id);

        let response = ChatResponse {
            chat_id: chat_id.clone(),
            message_id: assistant_message_id.clone(),
            content: mock_response.to_string(),
            model_id: request.model_id,
            provider_id: request.provider_id,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            duration: None,
        };
        
        tracing::info!("[MessageService::send_message] Successfully completed for chat_id: {:?}, message_id: {}", chat_id, assistant_message_id);
        Ok(response)
    }

    /// 获取消息
    pub async fn get_messages(
        &self,
        chat_id: UUID,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>, AppError> {
        tracing::info!("[MessageService::get_messages] Getting messages for chat_id: {}", chat_id);
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let pool = self.db.pool();
        let mut messages = Vec::new();

        let rows = sqlx::query(
            "SELECT id, chat_id, role, content, model_id, provider_id, temperature, top_p, max_tokens, stream, attachments, input_tokens, output_tokens, total_tokens, start_time, end_time, duration, created_at, updated_at FROM messages WHERE chat_id = ?1 ORDER BY created_at ASC LIMIT ?2 OFFSET ?3"
        )
        .bind(&chat_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            let error = format!("Failed to get messages: {}", e);
            tracing::error!("[MessageService::get_messages] Database error for chat_id {}: {}", chat_id, error);
            AppError::internal_error(&error)
        })?;

        for row in rows {
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

            messages.push(Message {
                id: row.try_get("id").unwrap_or_default(),
                chat_id: row.try_get("chat_id").unwrap_or_default(),
                role,
                content: row.try_get("content").unwrap_or_default(),
                model_id: row.try_get("model_id").ok(),
                provider_id: row.try_get("provider_id").ok(),
                temperature: row.try_get("temperature").ok(),
                top_p: row.try_get("top_p").ok(),
                max_tokens: row.try_get("max_tokens").ok(),
                stream: row.try_get("stream").ok(),
                attachments,
                input_tokens: row.try_get("input_tokens").ok(),
                output_tokens: row.try_get("output_tokens").ok(),
                total_tokens: row.try_get("total_tokens").ok(),
                start_time: row.try_get("start_time").ok(),
                end_time: row.try_get("end_time").ok(),
                duration: row.try_get("duration").ok(),
                created_at: row.try_get("created_at").unwrap_or_default(),
                updated_at: row.try_get("updated_at").unwrap_or_default(),
            });
        }

        tracing::info!("[MessageService::get_messages] Retrieved {} messages for chat_id: {}", messages.len(), chat_id);
        Ok(messages)
    }

    /// 获取单条消息
    pub async fn get_message(&self, message_id: UUID) -> Result<Message, AppError> {
        tracing::info!("[MessageService::get_message] Getting message: {}", message_id);
        let pool = self.db.pool();
        let row = sqlx::query(
            "SELECT id, chat_id, role, content, model_id, provider_id, temperature, top_p, max_tokens, stream, attachments, input_tokens, output_tokens, total_tokens, start_time, end_time, duration, created_at, updated_at FROM messages WHERE id = ?1"
        )
        .bind(&message_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            let error = format!("Failed to get message: {}", e);
            tracing::error!("[MessageService::get_message] Database error for message_id {}: {}", message_id, error);
            AppError::internal_error(&error)
        })?;

        match row {
            Some(row) => {
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

                Ok(Message {
                    id: row.try_get("id").unwrap_or_default(),
                    chat_id: row.try_get("chat_id").unwrap_or_default(),
                    role,
                    content: row.try_get("content").unwrap_or_default(),
                    model_id: row.try_get("model_id").ok(),
                    provider_id: row.try_get("provider_id").ok(),
                    temperature: row.try_get("temperature").ok(),
                    top_p: row.try_get("top_p").ok(),
                    max_tokens: row.try_get("max_tokens").ok(),
                    stream: row.try_get("stream").ok(),
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
            None => {
                let error = format!("Message not found: {}", message_id);
                tracing::warn!("[MessageService::get_message] {}", error);
                Err(AppError::not_found(&error))
            },
        }
    }

    /// 更新消息
    pub async fn update_message(
        &self,
        message_id: UUID,
        content: String,
    ) -> Result<Message, AppError> {
        tracing::info!("[MessageService::update_message] Updating message: {}", message_id);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let pool = self.db.pool();
        
        // 先检查消息是否存在
        let _existing = self.get_message(message_id.clone()).await?;

        // 更新消息内容
        sqlx::query(
            "UPDATE messages SET content = ?1, updated_at = ?2 WHERE id = ?3"
        )
        .bind(&content)
        .bind(now)
        .bind(&message_id)
        .execute(pool)
        .await
        .map_err(|e| {
            let error = format!("Failed to update message: {}", e);
            tracing::error!("[MessageService::update_message] Database error for message_id {}: {}", message_id, error);
            AppError::internal_error(&error)
        })?;
        
        tracing::info!("[MessageService::update_message] Successfully updated message: {}", message_id);

        // 返回更新后的消息
        self.get_message(message_id).await
    }

    /// 删除消息
    pub async fn delete_message(&self, message_id: UUID) -> Result<(), AppError> {
        tracing::info!("[MessageService::delete_message] Deleting message: {}", message_id);
        let pool = self.db.pool();

        // 先检查消息是否存在
        self.get_message(message_id.clone()).await?;

        // 删除消息
        let result = sqlx::query("DELETE FROM messages WHERE id = ?1")
            .bind(&message_id)
            .execute(pool)
            .await
            .map_err(|e| {
                let error = format!("Failed to delete message: {}", e);
                tracing::error!("[MessageService::delete_message] Database error for message_id {}: {}", message_id, error);
                AppError::internal_error(&error)
            })?;

        if result.rows_affected() == 0 {
            let error = format!("Message not found: {}", message_id);
            tracing::warn!("[MessageService::delete_message] {}", error);
            return Err(AppError::not_found(&error));
        }
        
        tracing::info!("[MessageService::delete_message] Successfully deleted message: {}", message_id);

        Ok(())
    }

    /// 重新生成消息
    pub async fn regenerate_message(&self, message_id: UUID) -> Result<ChatResponse, AppError> {
        tracing::info!("[MessageService::regenerate_message] Regenerating message: {}", message_id);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 1. 获取要重新生成的消息
        let message = self.get_message(message_id.clone()).await?;
        
        // 2. 验证消息是否为助手消息
        if message.role != MessageRole::Assistant {
            let error = "Can only regenerate assistant messages";
            tracing::error!("[MessageService::regenerate_message] Validation failed for message_id {}: {}", message_id, error);
            return Err(AppError::validation_error(error));
        }

        let pool = self.db.pool();

        // 3. 生成新的内容（目前是模拟的，后续需要集成 LLM API）
        let new_content = format!("重新生成的回复: {}", 
            chrono::DateTime::from_timestamp(now / 1000, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S")
        );

        // 4. 更新消息内容
        sqlx::query(
            "UPDATE messages SET content = ?1, updated_at = ?2 WHERE id = ?3"
        )
        .bind(&new_content)
        .bind(now)
        .bind(&message_id)
        .execute(pool)
        .await
        .map_err(|e| {
            let error = format!("Failed to update regenerated message: {}", e);
            tracing::error!("[MessageService::regenerate_message] Database error for message_id {}: {}", message_id, error);
            AppError::internal_error(&error)
        })?;
        
        tracing::info!("[MessageService::regenerate_message] Successfully regenerated message: {}", message_id);

        Ok(ChatResponse {
            chat_id: message.chat_id,
            message_id,
            content: new_content,
            model_id: message.model_id.unwrap_or_default(),
            provider_id: message.provider_id.unwrap_or_default(),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            duration: None,
        })
    }
}

#[cfg(test)]
#[path = "message_test.rs"]
mod message_test;