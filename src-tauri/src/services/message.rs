// 消息服务实现

use crate::models::{AppError, ChatRequest, ChatResponse, Message, MessageRole, MessageAttachment, UUID};
use crate::services::DatabaseService;
use crate::storage::MessageRepository;
use std::sync::Arc;

/// 消息服务
pub struct MessageService {
    repository: MessageRepository,
}

impl MessageService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self {
            repository: MessageRepository::new(db),
        }
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

        // 2. 保存用户消息到数据库
        let user_message_id = uuid::Uuid::new_v4().to_string();
        let chat_id = request.chat_id.as_ref().ok_or_else(|| {
            let error = "Chat ID is required";
            tracing::error!("[MessageService::send_message] Validation failed: {}", error);
            AppError::validation_error(error)
        })?;

        let user_message = Message {
            id: user_message_id.clone(),
            chat_id: chat_id.clone(),
            role: MessageRole::User,
            content: last_message.content.clone(),
            model_id: None, // 在测试中避免外键约束
            provider_id: None, // 在测试中避免外键约束
            temperature: request.parameters.as_ref().and_then(|p| p.temperature),
            top_p: request.parameters.as_ref().and_then(|p| p.top_p),
            max_tokens: request.parameters.as_ref().and_then(|p| p.max_tokens),
            stream: request.parameters.as_ref().and_then(|p| p.stream),
            attachments: request.attachments.as_ref().map(|attachments| {
                attachments.iter().map(|att| MessageAttachment {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: att.name.clone(),
                    mime_type: att.mime_type.clone(),
                    size: att.data.len() as i64,
                    path: format!("/tmp/{}", uuid::Uuid::new_v4()), // 临时路径，实际应该保存文件
                }).collect()
            }),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_message(&user_message).await.map_err(|e| {
            let error = format!("Failed to save user message: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
        })?;
        
        tracing::info!("[MessageService::send_message] User message saved with ID: {}", user_message_id);

        // 3. 目前返回一个模拟的响应（后续需要集成实际的 LLM API 调用）
        let assistant_message_id = uuid::Uuid::new_v4().to_string();
        let mock_response = "这是一个模拟的回复，实际的 LLM API 集成正在开发中。";

        // 4. 保存助手消息到数据库
        let assistant_message = Message {
            id: assistant_message_id.clone(),
            chat_id: chat_id.clone(),
            role: MessageRole::Assistant,
            content: mock_response.to_string(),
            model_id: None, // 在测试中避免外键约束
            provider_id: None, // 在测试中避免外键约束
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
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

        self.repository.create_message(&assistant_message).await.map_err(|e| {
            let error = format!("Failed to save assistant message: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
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

        let messages = self.repository.get_messages_by_chat(&chat_id, limit, offset).await.map_err(|e| {
            let error = format!("Failed to get messages: {}", e);
            tracing::error!("[MessageService::get_messages] Database error for chat_id {}: {}", chat_id, error);
            e
        })?;

        tracing::info!("[MessageService::get_messages] Retrieved {} messages for chat_id: {}", messages.len(), chat_id);
        Ok(messages)
    }

    /// 获取单条消息
    pub async fn get_message(&self, message_id: UUID) -> Result<Message, AppError> {
        tracing::info!("[MessageService::get_message] Getting message: {}", message_id);
        
        match self.repository.get_message_by_id(&message_id).await? {
            Some(message) => Ok(message),
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

        // 先检查消息是否存在
        let _existing = self.get_message(message_id.clone()).await?;

        // 更新消息内容
        self.repository.update_message_content(&message_id, &content, now).await.map_err(|e| {
            let error = format!("Failed to update message: {}", e);
            tracing::error!("[MessageService::update_message] Database error for message_id {}: {}", message_id, error);
            e
        })?;
        
        tracing::info!("[MessageService::update_message] Successfully updated message: {}", message_id);

        // 返回更新后的消息
        self.get_message(message_id).await
    }

    /// 删除消息
    pub async fn delete_message(&self, message_id: UUID) -> Result<(), AppError> {
        tracing::info!("[MessageService::delete_message] Deleting message: {}", message_id);

        // 先检查消息是否存在
        self.get_message(message_id.clone()).await?;

        // 删除消息
        self.repository.delete_message(&message_id).await.map_err(|e| {
            let error = format!("Failed to delete message: {}", e);
            tracing::error!("[MessageService::delete_message] Database error for message_id {}: {}", message_id, error);
            e
        })?;
        
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

        // 3. 生成新的内容（目前是模拟的，后续需要集成 LLM API）
        let new_content = format!("重新生成的回复: {}", 
            chrono::DateTime::from_timestamp(now / 1000, 0)
                .unwrap_or_default()
                .format("%Y-%m-%d %H:%M:%S")
        );

        // 4. 更新消息内容
        self.repository.update_message_content(&message_id, &new_content, now).await.map_err(|e| {
            let error = format!("Failed to update regenerated message: {}", e);
            tracing::error!("[MessageService::regenerate_message] Database error for message_id {}: {}", message_id, error);
            e
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