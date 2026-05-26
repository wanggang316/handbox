// Session 服务实现

use crate::models::AppError;
use crate::models::llm_types::LlmMessageRole;
use crate::services::chat_engine::{self, ChatMessage, ChatOptions, ChatProvider};
use crate::services::{Database, ProviderService};
use crate::storage::types::{McpServerConfig, Session, SessionReasoningConfig, UUID};
use crate::storage::{AgentRepository, MessageRepository, SessionRepository};
use std::collections::HashMap;
use std::sync::Arc;

/// Session 参数类型
pub enum SessionParameter {
    Name(String),
    Temperature(Option<f32>),
    TopP(Option<f32>),
    TopK(Option<i32>),
    MaxTokens(Option<i32>),
    Stream(Option<bool>),
    Model {
        model_id: String,
        provider_id: String,
    },
    SystemPrompt(Option<String>),
    McpServers(Vec<McpServerConfig>),
    TurnCount(Option<i32>),
    Reasoning(Option<SessionReasoningConfig>),
}

/// Session 服务
#[derive(Clone)]
pub struct SessionService {
    repository: SessionRepository,
    agent_repository: AgentRepository,
    message_repository: MessageRepository,
    provider_service: Arc<ProviderService>,
}

impl SessionService {
    pub fn new(db: Arc<Database>, provider_service: Arc<ProviderService>) -> Self {
        Self {
            repository: SessionRepository::new(db.clone()),
            agent_repository: AgentRepository::new(db.clone()),
            message_repository: MessageRepository::new(db),
            provider_service,
        }
    }

    /// 创建 Session
    pub async fn create_chat(
        &self,
        name: String,
        temperature: Option<f32>,
        top_p: Option<f32>,
        top_k: Option<i32>,
        max_tokens: Option<i32>,
        stream: Option<bool>,
        model_id: Option<String>,
        provider_id: Option<String>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<McpServerConfig>>,
    ) -> Result<Session, AppError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let chat = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            last_message_at: None,
            message_count: 0,
            temperature,
            top_p,
            top_k,
            max_tokens,
            stream,
            model_id,
            provider_id,
            system_prompt,
            mcp_servers: mcp_servers.unwrap_or_default(),
            turn_count: Some(5), // 默认值为 5
            artifact_id: None,
            agent_id: None,
            reasoning: None,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_chat(&chat).await?;
        Ok(chat)
    }

    /// 通过 Agent 创建 Session（复制 Agent 的配置到 Session）
    pub async fn create_session_from_agent(&self, agent_id: UUID) -> Result<Session, AppError> {
        // 获取 Agent 配置
        let agent = match self.agent_repository.get_agent_by_id(&agent_id).await? {
            Some(agent) => agent,
            None => return Err(AppError::not_found(&format!("Agent not found: {}", agent_id))),
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 从 Agent 创建 Session，复制所有配置
        let session = Session {
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("{} - Session", agent.name),
            last_message_at: None,
            message_count: 0,
            temperature: agent.temperature,
            top_p: agent.top_p,
            top_k: agent.top_k,
            max_tokens: agent.max_tokens,
            stream: None,
            model_id: agent.model,
            provider_id: None,
            system_prompt: agent.system_prompt,
            mcp_servers: agent.mcp_servers,
            turn_count: Some(5),
            artifact_id: None,
            agent_id: Some(agent_id),
            reasoning: agent.reasoning,
            created_at: now,
            updated_at: now,
        };

        self.repository.create_session(&session).await?;
        Ok(session)
    }

    /// 获取 Session 列表
    pub async fn list_chats(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Session>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        self.repository.list_chats(limit, offset).await
    }

    /// 获取 Session 详情
    pub async fn get_chat(&self, chat_id: UUID) -> Result<Session, AppError> {
        match self.repository.get_chat_by_id(&chat_id).await? {
            Some(chat) => Ok(chat),
            None => Err(AppError::not_found(&format!("Chat not found: {}", chat_id))),
        }
    }

    /// 统一的参数更新方法
    pub async fn update_chat_parameter(
        &self,
        chat_id: UUID,
        parameter: SessionParameter,
    ) -> Result<Session, AppError> {
        let mut chat = self.get_chat(chat_id).await?;

        match parameter {
            SessionParameter::Name(name) => chat.name = name,
            SessionParameter::Temperature(temp) => chat.temperature = temp,
            SessionParameter::TopP(top_p) => chat.top_p = top_p,
            SessionParameter::TopK(top_k) => chat.top_k = top_k,
            SessionParameter::MaxTokens(max_tokens) => chat.max_tokens = max_tokens,
            SessionParameter::Stream(stream) => chat.stream = stream,
            SessionParameter::Model {
                model_id,
                provider_id,
            } => {
                chat.model_id = Some(model_id);
                chat.provider_id = Some(provider_id);
            }
            SessionParameter::SystemPrompt(prompt) => chat.system_prompt = prompt,
            SessionParameter::McpServers(servers) => chat.mcp_servers = servers,
            SessionParameter::TurnCount(turn_count) => chat.turn_count = turn_count,
            SessionParameter::Reasoning(reasoning) => chat.reasoning = reasoning,
        }

        chat.updated_at = Self::current_timestamp();
        self.repository.update_chat(&chat).await?;
        Ok(chat)
    }

    /// 批量更新聊天设置（保留用于兼容性）
    pub async fn update_chat(
        &self,
        chat_id: UUID,
        name: Option<String>,
        temperature: Option<Option<f32>>,
        top_p: Option<Option<f32>>,
        top_k: Option<Option<i32>>,
        max_tokens: Option<Option<i32>>,
        stream: Option<Option<bool>>,
        model_id: Option<String>,
        provider_id: Option<String>,
        system_prompt: Option<String>,
        mcp_servers: Option<Vec<McpServerConfig>>,
        turn_count: Option<i32>,
    ) -> Result<Session, AppError> {
        let mut chat = self.get_chat(chat_id).await?;

        if let Some(n) = name {
            chat.name = n;
        }
        if let Some(t) = temperature {
            chat.temperature = t;
        }
        if let Some(tp) = top_p {
            chat.top_p = tp;
        }
        if let Some(tk) = top_k {
            chat.top_k = tk;
        }
        if let Some(mt) = max_tokens {
            chat.max_tokens = mt;
        }
        if let Some(s) = stream {
            chat.stream = s;
        }
        if let Some(mid) = model_id {
            chat.model_id = Some(mid);
        }
        if let Some(pid) = provider_id {
            chat.provider_id = Some(pid);
        }
        if let Some(sp) = system_prompt {
            chat.system_prompt = Some(sp);
        }
        if let Some(ms) = mcp_servers {
            chat.mcp_servers = ms;
        }
        if let Some(tc) = turn_count {
            chat.turn_count = Some(tc);
        }

        chat.updated_at = Self::current_timestamp();
        self.repository.update_chat(&chat).await?;
        Ok(chat)
    }

    /// 清空模型相关参数
    pub async fn clear_model_parameters(&self, chat_id: UUID) -> Result<Session, AppError> {
        let mut chat = self.get_chat(chat_id).await?;
        chat.temperature = None;
        chat.top_p = None;
        chat.top_k = None;
        chat.max_tokens = None;
        chat.stream = None;
        chat.reasoning = None;
        chat.updated_at = Self::current_timestamp();
        self.repository.update_chat(&chat).await?;
        Ok(chat)
    }

    fn current_timestamp() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
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
            "[SessionService::generate_title] Generating title for chat: {}",
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
            .filter(|msg| matches!(msg.role, LlmMessageRole::User))
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

        // 8. 构造 chat_engine ChatProvider（不再创建 handbox-llm 客户端）
        let chat_provider = ChatProvider {
            provider_type: provider.provider_type.clone(),
            base_url: provider.base_url.clone(),
            api_key: provider.api_key.clone(),
        };

        // 9. 构造单条 User ChatMessage 承载标题生成提示词。
        //    标题生成无附件、无工具、无 reasoning，因此 attachment_ids/tool_calls/
        //    tool_call_id 全部留空，hydrated_attachments 为空 map。
        let chat_messages = vec![ChatMessage {
            id: "title-gen-0".to_string(),
            role: LlmMessageRole::User,
            content: title_prompt,
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            attachment_ids: Vec::new(),
        }];

        // 10. 构造 ChatOptions。保留原有的低 temperature (0.1) 与
        //     max_tokens (50) 以维持标题生成的确定性短输出。
        let chat_options = ChatOptions {
            temperature: Some(0.1),
            max_tokens: Some(50),
            tools: Vec::new(),
            reasoning_effort: None,
            signal: None,
            hydrated_attachments: HashMap::new(),
        };

        // 11. 调用 chat_engine 非流式 API
        let chunk = chat_engine::complete_chat(
            &chat_provider,
            &model_id,
            &chat_messages,
            chat_options,
        )
        .await
        .map_err(|e| {
            tracing::error!(
                "[SessionService::generate_title] chat_engine::complete_chat returned error for provider {}: {}",
                provider.provider_type,
                e.message
            );
            e
        })?;

        // 12. 提取并清理标题
        let raw_title = chunk.content.unwrap_or_default();
        let generated_title = raw_title.trim();

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
            "[SessionService::generate_title] Generated title: {}",
            final_title
        );
        Ok(final_title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::llm_config::LlmConfig;
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
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let _service = SessionService::new(db, provider_service);
    }

    #[tokio::test]
    async fn creates_chat_with_all_fields() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

        let chat = service
            .create_chat(
                "Test Chat".to_string(),
                Some(0.7),
                Some(0.9),
                Some(40),
                Some(2048),
                Some(true),
                Some("gpt-4o".to_string()),
                Some("openai".to_string()),
                Some("System prompt".to_string()),
                Some(vec![McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec!["tool1".to_string()],
                }]),
            )
            .await
            .expect("chat creation failed");

        assert_eq!(chat.name, "Test Chat");
        assert_eq!(chat.temperature, Some(0.7));
        assert_eq!(chat.top_p, Some(0.9));
        assert_eq!(chat.top_k, Some(40));
        assert_eq!(chat.max_tokens, Some(2048));
        assert_eq!(chat.stream, Some(true));
        assert_eq!(chat.model_id, Some("gpt-4o".to_string()));
        assert_eq!(chat.provider_id, Some("openai".to_string()));
        assert_eq!(chat.system_prompt, Some("System prompt".to_string()));
        assert_eq!(
            chat.mcp_servers,
            vec![McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }]
        );
        assert_eq!(chat.message_count, 0);
        assert!(chat.last_message_at.is_none());
    }

    #[tokio::test]
    async fn lists_chats_sorted_by_updated_at() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

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
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

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
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

        let err = service
            .get_chat("nonexistent_chat".to_string())
            .await
            .expect_err("expected error");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn updates_existing_chat() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

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
                None,
            )
            .await
            .unwrap();

        let original_model_id = created.model_id.clone();
        let original_provider_id = created.provider_id.clone();

        let updated = service
            .update_chat(
                created.id.clone(),
                Some("Updated Name".to_string()),
                Some(Some(0.8)),   // Option<Option<f32>>
                Some(Some(0.95)),  // Option<Option<f32>>
                Some(Some(40)),    // Option<Option<i32>>
                Some(Some(4096)),  // Option<Option<i32>>
                Some(Some(false)), // Option<Option<bool>>
                None,              // model unchanged
                None,              // provider unchanged
                Some("Updated prompt".to_string()),
                Some(vec![
                    McpServerConfig {
                        server_id: "server1".to_string(),
                        execution_mode: "auto".to_string(),
                        enabled_tools: vec!["tool1".to_string(), "tool2".to_string()],
                    },
                    McpServerConfig {
                        server_id: "server2".to_string(),
                        execution_mode: "manual".to_string(),
                        enabled_tools: vec!["tool3".to_string()],
                    },
                ]),
                Some(10),
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.temperature, Some(0.8));
        assert_eq!(updated.top_p, Some(0.95));
        assert_eq!(updated.top_k, Some(40));
        assert_eq!(updated.max_tokens, Some(4096));
        assert_eq!(updated.stream, Some(false));
        assert_eq!(updated.system_prompt, Some("Updated prompt".to_string()));
        assert_eq!(
            updated.mcp_servers,
            vec![
                McpServerConfig {
                    server_id: "server1".to_string(),
                    execution_mode: "auto".to_string(),
                    enabled_tools: vec!["tool1".to_string(), "tool2".to_string()],
                },
                McpServerConfig {
                    server_id: "server2".to_string(),
                    execution_mode: "manual".to_string(),
                    enabled_tools: vec!["tool3".to_string()],
                },
            ]
        );
    }

    #[tokio::test]
    async fn delete_chat_removes_record() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

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
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

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

    #[tokio::test]
    async fn clears_parameters_when_passed_some_none() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

        // 创建带有参数的聊天
        let created = service
            .create_chat(
                "Test Chat".to_string(),
                Some(0.7),
                Some(0.9),
                Some(40),
                Some(2048),
                Some(true),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(created.temperature, Some(0.7));
        assert_eq!(created.top_p, Some(0.9));
        assert_eq!(created.top_k, Some(40));
        assert_eq!(created.max_tokens, Some(2048));
        assert_eq!(created.stream, Some(true));

        // 清空所有参数（通过传递 Some(None)）
        let updated = service
            .update_chat(
                created.id.clone(),
                None,
                Some(None), // 清空 temperature
                Some(None), // 清空 top_p
                Some(None), // 清空 top_k
                Some(None), // 清空 max_tokens
                Some(None), // 清空 stream
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("update failed");

        assert_eq!(updated.temperature, None);
        assert_eq!(updated.top_p, None);
        assert_eq!(updated.top_k, None);
        assert_eq!(updated.max_tokens, None);
        assert_eq!(updated.stream, None);
    }

    #[tokio::test]
    async fn clears_parameters_via_service_method() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

        let created = service
            .create_chat(
                "Test Chat".to_string(),
                Some(0.8),
                Some(0.6),
                Some(20),
                Some(1024),
                Some(true),
                Some("model-1".to_string()),
                Some("provider-1".to_string()),
                None,
                None,
            )
            .await
            .unwrap();

        let cleared = service
            .clear_model_parameters(created.id.clone())
            .await
            .expect("clear failed");

        assert!(cleared.temperature.is_none());
        assert!(cleared.top_p.is_none());
        assert!(cleared.top_k.is_none());
        assert!(cleared.max_tokens.is_none());
        assert!(cleared.stream.is_none());
        assert!(cleared.reasoning.is_none());
    }

    #[tokio::test]
    async fn preserves_parameters_when_passed_none() {
        let db = create_test_database().await;
        let llm_config = Arc::new(LlmConfig::new());
        let llm_config_provider = llm_config.clone();
        let provider_service = Arc::new(ProviderService::new(
            db.clone(),
            llm_config_provider.clone(),
        ));
        let service = SessionService::new(db, provider_service);

        // 创建带有参数的聊天
        let created = service
            .create_chat(
                "Test Chat".to_string(),
                Some(0.7),
                Some(0.9),
                Some(40),
                Some(2048),
                Some(true),
                None,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // 不传递参数（None），应该保持原值
        let updated = service
            .update_chat(
                created.id.clone(),
                Some("Updated Name".to_string()),
                None, // 不修改 temperature，保持原值
                None, // 不修改 top_p，保持原值
                None, // 不修改 top_k，保持原值
                None, // 不修改 max_tokens，保持原值
                None, // 不修改 stream，保持原值
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("update failed");

        assert_eq!(updated.name, "Updated Name");
        assert_eq!(updated.temperature, Some(0.7)); // 保持原值
        assert_eq!(updated.top_p, Some(0.9)); // 保持原值
        assert_eq!(updated.top_k, Some(40)); // 保持原值
        assert_eq!(updated.max_tokens, Some(2048)); // 保持原值
        assert_eq!(updated.stream, Some(true)); // 保持原值
    }
}
