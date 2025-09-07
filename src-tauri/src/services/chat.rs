// 聊天服务实现

use crate::models::{AppError, Chat, UUID};
use crate::services::DatabaseService;
use crate::storage::ChatRepository;
use std::sync::Arc;

/// 聊天服务
pub struct ChatService {
    repository: ChatRepository,
}

impl ChatService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self {
            repository: ChatRepository::new(db),
        }
    }

    /// 创建聊天
    pub async fn create_chat(
        &self,
        name: String,
        temperature: Option<f32>,
        top_p: Option<f32>,
        max_tokens: Option<i32>,
        stream: Option<bool>,
        model_id: Option<String>,
        provider_id: Option<String>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<String>>,
    ) -> Result<Chat, AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let chat = Chat {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            last_message_at: None,
            message_count: 0,
            temperature,
            top_p,
            max_tokens,
            stream,
            model_id,
            provider_id,
            system_prompt,
            mcp_servers: mcp_servers.unwrap_or_default(),
            artifact_id: None,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_chat(&chat).await?;
        Ok(chat)
    }

    /// 获取聊天列表
    pub async fn list_chats(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Chat>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        self.repository.list_chats(limit, offset).await
    }

    /// 获取聊天详情
    pub async fn get_chat(&self, chat_id: UUID) -> Result<Chat, AppError> {
        match self.repository.get_chat_by_id(&chat_id).await? {
            Some(chat) => Ok(chat),
            None => Err(AppError::not_found(&format!("Chat not found: {}", chat_id))),
        }
    }

    /// 更新聊天
    pub async fn update_chat(
        &self,
        chat_id: UUID,
        name: Option<String>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        max_tokens: Option<i32>,
        stream: Option<bool>,
        model_id: Option<String>,
        provider_id: Option<String>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<String>>,
    ) -> Result<Chat, AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 先检查聊天是否存在
        let existing_chat = self.get_chat(chat_id.clone()).await?;

        // 构建更新后的聊天数据
        let updated_chat = Chat {
            id: existing_chat.id,
            name: name.unwrap_or(existing_chat.name),
            last_message_at: existing_chat.last_message_at,
            message_count: existing_chat.message_count,
            temperature: temperature.or(existing_chat.temperature),
            top_p: top_p.or(existing_chat.top_p),
            max_tokens: max_tokens.or(existing_chat.max_tokens),
            stream: stream.or(existing_chat.stream),
            model_id: model_id.or(existing_chat.model_id),
            provider_id: provider_id.or(existing_chat.provider_id),
            system_prompt: system_prompt.or(existing_chat.system_prompt),
            mcp_servers: mcp_servers.unwrap_or(existing_chat.mcp_servers),
            artifact_id: existing_chat.artifact_id,
            created_at: existing_chat.created_at,
            updated_at: now,
        };

        self.repository.update_chat(&updated_chat).await?;
        Ok(updated_chat)
    }

    /// 删除聊天
    pub async fn delete_chat(&self, chat_id: UUID) -> Result<(), AppError> {
        // 先检查聊天是否存在
        self.get_chat(chat_id.clone()).await?;

        // 删除聊天（相关消息会通过外键级联删除）
        self.repository.delete_chat(&chat_id).await
    }
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;
