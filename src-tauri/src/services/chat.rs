// 聊天服务实现

use crate::llm_client::create_llm_client;
use crate::llm_client::types::{ChatMessage, ChatRequest};
use crate::models::{AppError, Chat, MessageRole, UUID};
use crate::services::{Database, ProviderService};
use crate::storage::{ChatRepository, MessageRepository};
use std::sync::Arc;

/// 聊天服务
#[derive(Clone)]
pub struct ChatService {
    repository: ChatRepository,
    message_repository: MessageRepository,
    provider_service: Arc<ProviderService>,
}

impl ChatService {
    pub fn new(db: Arc<Database>, provider_service: Arc<ProviderService>) -> Self {
        Self {
            repository: ChatRepository::new(db.clone()),
            message_repository: MessageRepository::new(db),
            provider_service,
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

    /// 生成聊天标题
    pub async fn generate_title(&self, chat_id: UUID) -> Result<String, AppError> {
        tracing::info!(
            "[ChatService::generate_title] Generating title for chat: {}",
            chat_id
        );

        // 1. 获取聊天信息
        let chat = self.get_chat(chat_id.clone()).await?;

        // 2. 验证模型和供应商配置
        let model_id = chat
            .model_id
            .ok_or_else(|| AppError::validation_error("Chat has no model configured"))?;
        let provider_id = chat
            .provider_id
            .ok_or_else(|| AppError::validation_error("Chat has no provider configured"))?;

        // 3. 获取聊天的最近消息（最多10条）
        let messages = self
            .message_repository
            .get_messages_by_chat(&chat_id, 100, 0)
            .await?;

        if messages.is_empty() {
            return Err(AppError::validation_error(
                "No messages found for title generation",
            ));
        }

        // 4. 只获取用户发送的消息
        let user_messages: Vec<String> = messages
            .iter()
            .filter(|msg| matches!(msg.role, MessageRole::User))
            .take(20)
            .map(|msg| msg.content.clone())
            .collect();

        if user_messages.is_empty() {
            return Err(AppError::validation_error(
                "No user messages found for title generation",
            ));
        }

        // 5. 构建对话上下文
        let conversation_context = user_messages.join("\n\n");

        // 6. 构建标题生成提示词
        let title_prompt = format!(
            "根据用户的以下问题或话题，生成一个简洁、准确的标题（不超过20个字符，不要包含引号或特殊符号）：\n\n{}\n\n请直接回复标题，不要包含任何解释或额外文字。",
            conversation_context
        );

        // 7. 获取提供商信息
        let provider = self.provider_service.get_provider(&provider_id).await?;

        // 8. 创建LLM客户端
        let llm_client = create_llm_client(&provider.provider_type).map_err(|e| {
            let error = format!(
                "Failed to create LLM client for provider type {}: {}",
                provider.provider_type, e
            );
            tracing::error!("[ChatService::generate_title] {}", error);
            AppError::internal_error(&error)
        })?;

        // 9. 构建API请求
        let api_request = ChatRequest {
            model: model_id,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: title_prompt,
                reasoning: None,
                tool_calls: None,
                tool_call_deltas: None,
                tool_call_id: None,
            }],
            temperature: Some(0.1), // 使用低温度确保稳定输出
            max_tokens: Some(50),   // 限制输出长度
            stream: Some(false),
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        // 10. 调用LLM API
        let response = llm_client.chat(&provider, api_request).await.map_err(|e| {
            let error = format!("Failed to call LLM API: {}", e);
            tracing::error!("[ChatService::generate_title] {}", error);
            AppError::internal_error(&error)
        })?;

        // 11. 提取并清理标题
        let generated_title = if let Some(choice) = response.choices.first() {
            if let Some(message) = &choice.message {
                message.content.trim()
            } else {
                return Err(AppError::internal_error("No message in response"));
            }
        } else {
            return Err(AppError::internal_error("No choices in response"));
        };

        // 确保标题不为空且长度合理
        if generated_title.is_empty() {
            return Err(AppError::internal_error("Generated title is empty"));
        }

        // 截断过长的标题
        let final_title = if generated_title.len() > 30 {
            generated_title.chars().take(30).collect::<String>()
        } else {
            generated_title.to_string()
        };

        tracing::info!(
            "[ChatService::generate_title] Generated title: {}",
            final_title
        );
        Ok(final_title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelParameters;
    use crate::services::ProviderService;
    use crate::storage::Database;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_database() -> Arc<Database> {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        Arc::new(
            Database::new(&db_path)
                .await
                .expect("Failed to create database"),
        )
    }

    #[tokio::test]
    async fn creates_service_successfully() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let _service = ChatService::new(db, provider_service);
    }

    #[tokio::test]
    async fn creates_chat_with_all_fields() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        let chat = service
            .create_chat(
                "Test Chat".to_string(),
                Some(0.7),
                Some(0.9),
                Some(2048),
                Some(true),
                Some("gpt-4o".to_string()),
                Some("openai".to_string()),
                Some("System prompt".to_string()),
                Some(vec!["server1".to_string()]),
            )
            .await
            .expect("chat creation failed");

        assert_eq!(chat.name, "Test Chat");
        assert_eq!(chat.temperature, Some(0.7));
        assert_eq!(chat.top_p, Some(0.9));
        assert_eq!(chat.max_tokens, Some(2048));
        assert_eq!(chat.stream, Some(true));
        assert_eq!(chat.model_id, Some("gpt-4o".to_string()));
        assert_eq!(chat.provider_id, Some("openai".to_string()));
        assert_eq!(chat.system_prompt, Some("System prompt".to_string()));
        assert_eq!(chat.mcp_servers, vec!["server1".to_string()]);
        assert_eq!(chat.message_count, 0);
        assert!(chat.last_message_at.is_none());
    }

    #[tokio::test]
    async fn lists_chats_sorted_by_updated_at() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        service
            .create_chat(
                "Chat 1".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        service
            .create_chat(
                "Chat 2".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let chats = service
            .list_chats(Some(10), Some(0))
            .await
            .expect("list chats failed");

        assert_eq!(chats.len(), 2);
        assert_eq!(chats[0].name, "Chat 2");
        assert_eq!(chats[1].name, "Chat 1");
    }

    #[tokio::test]
    async fn fetches_chat_by_id() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        let created = service
            .create_chat(
                "Test Chat".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let fetched = service
            .get_chat(created.id.clone())
            .await
            .expect("expected chat");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Test Chat");
    }

    #[tokio::test]
    async fn get_chat_returns_not_found_error() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        let err = service
            .get_chat("nonexistent_chat".to_string())
            .await
            .expect_err("expected error");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn updates_existing_chat() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        let created = service
            .create_chat(
                "Original Name".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let updated = service
            .update_chat(
                created.id.clone(),
                Some("Updated Name".to_string()),
                Some(0.8),
                Some(0.95),
                Some(4096),
                Some(false),
                Some("claude-3".to_string()),
                Some("anthropic".to_string()),
                Some("Updated prompt".to_string()),
                Some(vec!["server1".to_string(), "server2".to_string()]),
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.temperature, Some(0.8));
        assert_eq!(updated.top_p, Some(0.95));
        assert_eq!(updated.max_tokens, Some(4096));
        assert_eq!(updated.stream, Some(false));
        assert_eq!(updated.model_id, Some("claude-3".to_string()));
        assert_eq!(updated.provider_id, Some("anthropic".to_string()));
        assert_eq!(updated.system_prompt, Some("Updated prompt".to_string()));
        assert_eq!(updated.mcp_servers, vec!["server1", "server2"]);
    }

    #[tokio::test]
    async fn delete_chat_removes_record() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        let created = service
            .create_chat(
                "To Delete".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        service
            .delete_chat(created.id.clone())
            .await
            .expect("delete failed");

        let err = service
            .get_chat(created.id)
            .await
            .expect_err("expected missing chat");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn generate_title_requires_messages() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let service = ChatService::new(db, provider_service);

        let chat = service
            .create_chat(
                "No Messages".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let err = service
            .generate_title(chat.id)
            .await
            .expect_err("expected validation error");

        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn model_parameters_default_values_overridable() {
        let params = ModelParameters {
            temperature: Some(0.5),
            top_p: Some(0.8),
            max_tokens: Some(1024),
            context_length: Some(2048),
            stream: Some(false),
        };

        assert_eq!(params.temperature, Some(0.5));
        assert_eq!(params.top_p, Some(0.8));
        assert_eq!(params.max_tokens, Some(1024));
        assert_eq!(params.context_length, Some(2048));
        assert_eq!(params.stream, Some(false));
    }
}
