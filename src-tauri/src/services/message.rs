// 消息服务实现

use crate::clients::chat_client::{
    ChatMessage as ApiChatMessage, ChatRequest as ApiChatRequest, ChatUsage,
};
use crate::clients::llm_client::create_llm_client;
use crate::models::{
    AppError, MessageRequest, MessageResponse, Message, MessageConfig, MessageRole,
    UUID,
};
use crate::services::{DatabaseService, ProviderService, ChatService};
use crate::storage::MessageRepository;
use std::sync::Arc;

/// 流式数据结构
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub reasoning: Option<String>,
}

/// LLM API 响应结构
#[derive(Debug)]
struct LlmApiResponse {
    content: String,
    reasoning: Option<String>, // 推理过程内容
    usage: Option<ChatUsage>,
    duration: Option<f64>,
}

/// 消息服务
#[derive(Clone)]
pub struct MessageService {
    repository: MessageRepository,
    provider_service: Arc<ProviderService>,
    chat_service: Arc<ChatService>,
}

impl MessageService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self {
            repository: MessageRepository::new(db.clone()),
            provider_service: Arc::new(ProviderService::new(db.clone())),
            chat_service: Arc::new(ChatService::new(db)),
        }
    }

    /// 发送消息
    pub async fn send_message(&self, request: MessageRequest) -> Result<MessageResponse, AppError> {
        tracing::info!(
            "[MessageService::send_message] Starting to send message for chat_id: {:?}",
            request.chat_id
        );

        tracing::debug!(
            "[MessageService::send_message] Request details: {:?}",
            request
        );
        // 1. 验证请求参数
        if request.messages.is_empty() {
            let error = "No messages provided";
            tracing::error!(
                "[MessageService::send_message] Validation failed: {}",
                error
            );
            return Err(AppError::validation_error(error));
        }

        // 获取最后一条用户消息
        let last_message = &request.messages[request.messages.len() - 1];
        if last_message.role != MessageRole::User {
            let error = "Last message must be from user";
            tracing::error!(
                "[MessageService::send_message] Validation failed: {}",
                error
            );
            return Err(AppError::validation_error(error));
        }

        // 2. 保存用户消息到数据库
        let chat_id = request.chat_id.as_ref().ok_or_else(|| {
            let error = "Chat ID is required";
            tracing::error!(
                "[MessageService::send_message] Validation failed: {}",
                error
            );
            AppError::validation_error(error)
        })?;

        // 从 chats 表获取配置参数
        let chat = self.get_chat_config(chat_id).await.map_err(|e| {
            let error = format!("Failed to get chat config: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
        })?;

        let config = Self::extract_message_config_from_chat(&request, &chat);
        let user_message_id = self.save_user_message(
            chat_id,
            &last_message.content,
            config,
        ).await.map_err(|e| {
            let error = format!("Failed to save user message: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
        })?;

        tracing::info!(
            "[MessageService::send_message] User message saved with ID: {}",
            user_message_id
        );

        // 3. 调用实际的 LLM API
        let llm_response = self.call_llm_api(&request).await?;

        // 4. 保存助手消息到数据库
        let config = Self::extract_message_config_from_chat(&request, &chat);
        let now = chrono::Utc::now().timestamp_millis();
        let assistant_message_id = self.save_assistant_message(
            chat_id,
            &llm_response.content,
            llm_response.reasoning.clone(),
            config,
            now,
            llm_response.duration.unwrap_or(0.0) as i64,
            llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            llm_response.usage.as_ref().map(|u| u.completion_tokens),
            llm_response.usage.as_ref().map(|u| u.total_tokens),
        ).await.map_err(|e| {
            let error = format!("Failed to save assistant message: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
        })?;

        tracing::info!(
            "[MessageService::send_message] Assistant message saved with ID: {}",
            assistant_message_id
        );

        let response = MessageResponse {
            chat_id: chat_id.clone(),
            message_id: assistant_message_id.clone(),
            content: llm_response.content,
            reasoning: llm_response.reasoning,
            model_id: request.model_id,
            provider_id: request.provider_id,
            input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
            duration: llm_response.duration.map(|d| d as i64),
        };

        tracing::info!("[MessageService::send_message] Successfully completed for chat_id: {:?}, message_id: {}", chat_id, assistant_message_id);
        Ok(response)
    }

    /// 调用 LLM API
    async fn call_llm_api(&self, request: &MessageRequest) -> Result<LlmApiResponse, AppError> {
        tracing::info!(
            "[MessageService::call_llm_api] Calling LLM API with provider: {}, model: {}",
            request.provider_id,
            request.model_id
        );

        // 1. 获取聊天配置
        let chat = if let Some(chat_id) = &request.chat_id {
            self.get_chat_config(chat_id).await.map_err(|e| {
                let error = format!("Failed to get chat config: {}", e);
                tracing::error!("[MessageService::call_llm_api] {}", error);
                e
            })?
        } else {
            return Err(AppError::validation_error("Chat ID is required"));
        };

        // 2. 获取供应商配置
        let provider = self
            .provider_service
            .get_provider(&request.provider_id)
            .await
            .map_err(|e| {
                let error = format!("Failed to get provider {}: {}", request.provider_id, e);
                tracing::error!("[MessageService::call_llm_api] {}", error);
                AppError::validation_error(&error)
            })?;

        // 调试：检查 API Key 是否存在
        if provider.api_key.is_empty() {
            tracing::error!(
                "[MessageService::call_llm_api] Provider {} has empty API key",
                request.provider_id
            );
            return Err(AppError::validation_error(
                "Provider has no API key configured",
            ));
        } else {
            let api_key_preview = if provider.api_key.len() > 8 {
                format!(
                    "{}...{}",
                    &provider.api_key[..4],
                    &provider.api_key[provider.api_key.len() - 4..]
                )
            } else {
                "***".to_string()
            };
            tracing::info!(
                "[MessageService::call_llm_api] Using provider: {} ({}), API key: {}",
                provider.name,
                provider.provider_type,
                api_key_preview
            );
        }

        // 3. 创建 LLM 客户端
        let llm_client = create_llm_client(&provider.provider_type).map_err(|e| {
            let error = format!(
                "Failed to create LLM client for provider type {}: {}",
                provider.provider_type, e
            );
            tracing::error!("[MessageService::call_llm_api] {}", error);
            e
        })?;

        // 4. 转换请求格式
        let api_request = self.convert_to_api_request(request, &chat)?;

        // 5. 调用 API
        let start_time = std::time::Instant::now();
        let api_response = llm_client.chat(&provider, api_request).await.map_err(|e| {
            let error = format!("LLM API call failed: {}", e);
            tracing::error!("[MessageService::call_llm_api] {}", error);
            e
        })?;
        let duration = start_time.elapsed().as_millis() as f64;

        // 5. 转换响应格式
        let llm_response = self.convert_from_api_response(api_response, duration)?;

        tracing::info!(
            "[MessageService::call_llm_api] API call successful, duration: {}ms",
            duration
        );
        Ok(llm_response)
    }

    /// 转换为 API 请求格式
    fn convert_to_api_request(&self, request: &MessageRequest, chat: &crate::models::Chat) -> Result<ApiChatRequest, AppError> {
        let messages: Vec<ApiChatMessage> = request
            .messages
            .iter()
            .map(|msg| ApiChatMessage {
                role: match msg.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::System => "system".to_string(),
                },
                content: msg.content.clone(),
                reasoning: msg.reasoning.clone(),
            })
            .collect();

        Ok(ApiChatRequest {
            model: request.model_id.clone(),
            messages,
            temperature: chat.temperature,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
        })
    }

    /// 流式调用 LLM API
    pub async fn call_llm_api_stream<F>(
        &self,
        request: &MessageRequest,
        mut callback: F,
    ) -> Result<MessageResponse, AppError>
    where
        F: FnMut(StreamChunk) + Send + 'static,
    {
        tracing::info!("[MessageService::call_llm_api_stream] Starting stream call with provider: {}, model: {}", 
            request.provider_id, request.model_id);

        // 1. 获取聊天配置
        let chat = if let Some(chat_id) = &request.chat_id {
            self.get_chat_config(chat_id).await.map_err(|e| {
                let error = format!("Failed to get chat config: {}", e);
                tracing::error!("[MessageService::call_llm_api_stream] {}", error);
                e
            })?
        } else {
            return Err(AppError::validation_error("Chat ID is required for streaming"));
        };

        // 2. 获取供应商配置
        let provider = self
            .provider_service
            .get_provider(&request.provider_id)
            .await
            .map_err(|e| {
                let error = format!("Failed to get provider {}: {}", request.provider_id, e);
                tracing::error!("[MessageService::call_llm_api_stream] {}", error);
                AppError::validation_error(&error)
            })?;

        // 验证API Key
        if provider.api_key.is_empty() {
            tracing::error!(
                "[MessageService::call_llm_api_stream] Provider {} has empty API key",
                request.provider_id
            );
            return Err(AppError::validation_error(
                "Provider has no API key configured",
            ));
        }

        let api_key_preview = if provider.api_key.len() > 8 {
            format!(
                "{}...{}",
                &provider.api_key[..4],
                &provider.api_key[provider.api_key.len() - 4..]
            )
        } else {
            "***".to_string()
        };
        tracing::info!(
            "[MessageService::call_llm_api_stream] Using provider: {} ({}), API key: {}",
            provider.name,
            provider.provider_type,
            api_key_preview
        );

        // 3. 创建 LLM 客户端
        let llm_client = create_llm_client(&provider.provider_type).map_err(|e| {
            let error = format!(
                "Failed to create LLM client for provider type {}: {}",
                provider.provider_type, e
            );
            tracing::error!("[MessageService::call_llm_api_stream] {}", error);
            e
        })?;

        // 4. 转换请求格式
        let mut api_request = self.convert_to_api_request(request, &chat)?;
        api_request.stream = Some(true); // 强制启用流式

        // 5. 保存用户输入消息到数据库（如果有chat_id）
        if let Some(chat_id) = &request.chat_id {
            // 从消息列表中找到最后一条用户消息进行保存
            if let Some(last_user_message) = request.messages.iter().rev()
                .find(|m| matches!(m.role, crate::models::MessageRole::User)) {
                
                // 从 chats 表获取配置参数
                let chat = match self.get_chat_config(chat_id).await {
                    Ok(chat) => chat,
                    Err(e) => {
                        tracing::error!("[MessageService::call_llm_api_stream] Failed to get chat config: {}", e);
                        return Err(e);
                    }
                };
                
                let config = Self::extract_message_config_from_chat(&request, &chat);
                if let Err(e) = self.save_user_message(
                    chat_id,
                    &last_user_message.content,
                    config,
                ).await {
                    tracing::error!("[MessageService::call_llm_api_stream] Failed to save user message: {}", e);
                }
            }
        }

        // 6. 调用真实的 LLM 流式 API
        let start_time = std::time::Instant::now();
        
        tracing::info!("[MessageService::call_llm_api_stream] Calling real LLM streaming API...");
        let mut stream = llm_client.chat_stream(&provider, api_request).await?;
        
        let mut accumulated_content = String::new();
        let mut accumulated_reasoning = String::new();
        let mut chunk_count = 0;
        
        // 7. 处理真实的流式响应
        use futures::StreamExt;
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk_response) => {
                    // 提取流式内容
                    if let Some(choice) = chunk_response.choices.first() {
                        if let Some(delta) = &choice.delta {
                            // 处理内容
                            let chunk = &delta.content;
                            accumulated_content.push_str(chunk);
                            
                            // 处理推理过程
                            if let Some(reasoning_chunk) = &delta.reasoning {
                                accumulated_reasoning.push_str(reasoning_chunk);
                            }
                            
                            callback(StreamChunk {
                                content: accumulated_content.clone(),
                                reasoning: delta.reasoning.clone(),
                            });
                            chunk_count += 1;
                            
                            tracing::debug!(
                                "[MessageService::call_llm_api_stream] Real streaming chunk {}: content='{}', reasoning='{}'",
                                chunk_count, 
                                &delta.content,
                                delta.reasoning.as_deref().unwrap_or("")
                            );
                            
                            // 添加小延迟以控制流速
                            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        }
                        
                        // 检查是否完成
                        if choice.finish_reason.is_some() {
                            tracing::info!(
                                "[MessageService::call_llm_api_stream] Stream finished with reason: {:?}",
                                choice.finish_reason
                            );
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("[MessageService::call_llm_api_stream] Stream error: {}", e);
                    return Err(e);
                }
            }
        }

        let duration = start_time.elapsed().as_millis() as i64;
        let message_id = uuid::Uuid::new_v4().to_string();

        // 7. 创建最终响应 (使用流式内容)
        let response = MessageResponse {
            chat_id: request.chat_id.clone().unwrap_or_default(),
            message_id: message_id.clone(),
            content: accumulated_content.clone(),  // 使用流式累积的内容
            reasoning: if accumulated_reasoning.is_empty() { None } else { Some(accumulated_reasoning.clone()) },
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: None,  // 流式API通常不返回token统计
            output_tokens: None,
            total_tokens: None,
            duration: Some(duration),
        };

        tracing::info!(
            "[MessageService::call_llm_api_stream] Real streaming API call completed, chunks: {}, total_content_length: {}, duration: {}ms",
            chunk_count, accumulated_content.len(), duration
        );

        // 8. 流式输出完成后，保存完整的AI响应消息到数据库
        if !accumulated_content.is_empty() && request.chat_id.is_some() {
            // 从 chats 表获取配置参数
            let chat = match self.get_chat_config(&request.chat_id.clone().unwrap()).await {
                Ok(chat) => chat,
                Err(e) => {
                    tracing::error!("[MessageService::call_llm_api_stream] Failed to get chat config for saving: {}", e);
                    return Err(e);
                }
            };
            
            let config = Self::extract_message_config_from_chat(&request, &chat);
            let start_time_millis = chrono::Utc::now().timestamp_millis() - duration;
            
            if let Err(e) = self.save_assistant_message(
                &request.chat_id.clone().unwrap(),
                &accumulated_content,
                if accumulated_reasoning.is_empty() { None } else { Some(accumulated_reasoning) },
                config,
                start_time_millis,
                duration,
                None,  // 流式API通常不返回token统计
                None,
                None,
            ).await {
                tracing::error!(
                    "[MessageService::call_llm_api_stream] Failed to save assistant message: {}",
                    e
                );
                // 注意：这里不返回错误，因为流式输出已经完成，用户已经看到了响应
                // 只是数据库保存失败，不应该影响用户体验
            }
        }

        Ok(response)
    }

    /// 构造并保存用户消息
    async fn save_user_message(
        &self,
        chat_id: &str,
        content: &str,
        config: Option<MessageConfig>,
    ) -> Result<String, AppError> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        
        let message = Message {
            id: message_id.clone(),
            chat_id: chat_id.to_string(),
            role: crate::models::MessageRole::User,
            content: content.to_string(),
            reasoning: None, // 用户消息没有推理过程
            config,
            attachments: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: Some(now),
            end_time: Some(now),
            duration: Some(0), // 用户消息无耗时
            created_at: now,
            updated_at: now,
        };

        self.repository.create_message(&message).await?;
        tracing::info!("[MessageService] User message saved: {}", message_id);
        Ok(message_id)
    }

    /// 构造并保存AI响应消息
    async fn save_assistant_message(
        &self,
        chat_id: &str,
        content: &str,
        reasoning: Option<String>,
        config: Option<MessageConfig>,
        start_time_millis: i64,
        duration_ms: i64,
        input_tokens: Option<i32>,
        output_tokens: Option<i32>,
        total_tokens: Option<i32>,
    ) -> Result<String, AppError> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        
        let message = Message {
            id: message_id.clone(),
            chat_id: chat_id.to_string(),
            role: crate::models::MessageRole::Assistant,
            content: content.to_string(),
            reasoning,
            config,
            attachments: None,
            input_tokens,
            output_tokens,
            total_tokens,
            start_time: Some(start_time_millis),
            end_time: Some(now),
            duration: Some(duration_ms),
            created_at: now,
            updated_at: now,
        };

        self.repository.create_message(&message).await?;
        tracing::info!("[MessageService] Assistant message saved: {}", message_id);
        Ok(message_id)
    }

    /// 获取聊天配置
    async fn get_chat_config(&self, chat_id: &str) -> Result<crate::models::Chat, AppError> {
        self.chat_service.get_chat(chat_id.to_string()).await
    }

    /// 从聊天信息中提取消息配置
    fn extract_message_config_from_chat(request: &MessageRequest, chat: &crate::models::Chat) -> Option<MessageConfig> {
        Some(MessageConfig {
            model_id: Some(request.model_id.clone()),
            provider_id: Some(request.provider_id.clone()),
            temperature: chat.temperature,
            top_p: chat.top_p,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
        })
    }

    /// 从 API 响应格式转换
    fn convert_from_api_response(
        &self,
        api_response: crate::clients::chat_client::ChatResponse,
        duration: f64,
    ) -> Result<LlmApiResponse, AppError> {
        let choice = api_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AppError::internal_error("No choices in API response"))?;

        let message = choice
            .message
            .ok_or_else(|| AppError::internal_error("No message in API choice"))?;

        Ok(LlmApiResponse {
            content: message.content,
            reasoning: message.reasoning,
            usage: api_response.usage,
            duration: Some(duration),
        })
    }

    /// 获取消息
    pub async fn get_messages(
        &self,
        chat_id: UUID,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Message>, AppError> {
        tracing::info!(
            "[MessageService::get_messages] Getting messages for chat_id: {}",
            chat_id
        );
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let messages = self
            .repository
            .get_messages_by_chat(&chat_id, limit, offset)
            .await
            .map_err(|e| {
                let error = format!("Failed to get messages: {}", e);
                tracing::error!(
                    "[MessageService::get_messages] Database error for chat_id {}: {}",
                    chat_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::get_messages] Retrieved {} messages for chat_id: {}",
            messages.len(),
            chat_id
        );
        Ok(messages)
    }

    /// 获取单条消息
    pub async fn get_message(&self, message_id: UUID) -> Result<Message, AppError> {
        tracing::info!(
            "[MessageService::get_message] Getting message: {}",
            message_id
        );

        match self.repository.get_message_by_id(&message_id).await? {
            Some(message) => Ok(message),
            None => {
                let error = format!("Message not found: {}", message_id);
                tracing::warn!("[MessageService::get_message] {}", error);
                Err(AppError::not_found(&error))
            }
        }
    }

    /// 更新消息
    pub async fn update_message(
        &self,
        message_id: UUID,
        content: String,
    ) -> Result<Message, AppError> {
        tracing::info!(
            "[MessageService::update_message] Updating message: {}",
            message_id
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 先检查消息是否存在
        let _existing = self.get_message(message_id.clone()).await?;

        // 更新消息内容
        self.repository
            .update_message_content(&message_id, &content, now)
            .await
            .map_err(|e| {
                let error = format!("Failed to update message: {}", e);
                tracing::error!(
                    "[MessageService::update_message] Database error for message_id {}: {}",
                    message_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::update_message] Successfully updated message: {}",
            message_id
        );

        // 返回更新后的消息
        self.get_message(message_id).await
    }

    /// 删除消息
    pub async fn delete_message(&self, message_id: UUID) -> Result<(), AppError> {
        tracing::info!(
            "[MessageService::delete_message] Deleting message: {}",
            message_id
        );

        // 先检查消息是否存在
        self.get_message(message_id.clone()).await?;

        // 删除消息
        self.repository
            .delete_message(&message_id)
            .await
            .map_err(|e| {
                let error = format!("Failed to delete message: {}", e);
                tracing::error!(
                    "[MessageService::delete_message] Database error for message_id {}: {}",
                    message_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::delete_message] Successfully deleted message: {}",
            message_id
        );

        Ok(())
    }

    /// 重新生成消息
    pub async fn regenerate_message(&self, message_id: UUID) -> Result<MessageResponse, AppError> {
        tracing::info!(
            "[MessageService::regenerate_message] Regenerating message: {}",
            message_id
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // 1. 获取要重新生成的消息
        let message = self.get_message(message_id.clone()).await?;

        // 2. 验证消息是否为助手消息
        if message.role != MessageRole::Assistant {
            let error = "Can only regenerate assistant messages";
            tracing::error!(
                "[MessageService::regenerate_message] Validation failed for message_id {}: {}",
                message_id,
                error
            );
            return Err(AppError::validation_error(error));
        }

        // 3. 获取聊天的历史消息，重新调用 LLM API
        let chat_messages = self
            .get_messages(message.chat_id.clone(), None, None)
            .await?;

        // 构造重新生成请求（使用原始请求参数）
        let config = message.config.as_ref();
        let regenerate_request = MessageRequest {
            chat_id: Some(message.chat_id.clone()),
            model_id: config.and_then(|c| c.model_id.clone()).unwrap_or_default(),
            provider_id: config
                .and_then(|c| c.provider_id.clone())
                .unwrap_or_default(),
            messages: chat_messages
                .iter()
                .filter(|m| m.role != MessageRole::Assistant || m.id != message_id) // 排除要重新生成的消息
                .map(|m| crate::models::ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    reasoning: None, // 历史消息没有推理过程
                })
                .collect(),
            attachments: None,
        };

        // 调用 LLM API 重新生成
        let llm_response = self.call_llm_api(&regenerate_request).await?;
        let new_content = llm_response.content;

        // 4. 更新消息内容
        self.repository
            .update_message_content(&message_id, &new_content, now)
            .await
            .map_err(|e| {
                let error = format!("Failed to update regenerated message: {}", e);
                tracing::error!(
                    "[MessageService::regenerate_message] Database error for message_id {}: {}",
                    message_id,
                    error
                );
                e
            })?;

        tracing::info!(
            "[MessageService::regenerate_message] Successfully regenerated message: {}",
            message_id
        );

        Ok(MessageResponse {
            chat_id: message.chat_id,
            message_id,
            content: new_content,
            reasoning: llm_response.reasoning,
            model_id: config.and_then(|c| c.model_id.clone()).unwrap_or_default(),
            provider_id: config
                .and_then(|c| c.provider_id.clone())
                .unwrap_or_default(),
            input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
            duration: llm_response.duration.map(|d| d as i64),
        })
    }
}

#[cfg(test)]
#[path = "message_test.rs"]
mod message_test;
