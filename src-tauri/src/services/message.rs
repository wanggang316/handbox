// 消息服务实现

use crate::clients::api_provider::{
    ChatMessage as ApiChatMessage, ChatRequest as ApiChatRequest, ChatUsage,
};
use crate::clients::llm_client::create_llm_client;
use crate::models::{
    AppError, ChatRequest, ChatResponse, Message, MessageAttachment, MessageConfig, MessageRole,
    UUID,
};
use crate::services::{DatabaseService, ProviderService};
use crate::storage::MessageRepository;
use std::sync::Arc;

/// LLM API 响应结构
#[derive(Debug)]
struct LlmApiResponse {
    content: String,
    usage: Option<ChatUsage>,
    duration: Option<f64>,
}

/// 消息服务
#[derive(Clone)]
pub struct MessageService {
    repository: MessageRepository,
    provider_service: Arc<ProviderService>,
}

impl MessageService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self {
            repository: MessageRepository::new(db.clone()),
            provider_service: Arc::new(ProviderService::new(db)),
        }
    }

    /// 发送消息
    pub async fn send_message(&self, request: ChatRequest) -> Result<ChatResponse, AppError> {
        tracing::info!(
            "[MessageService::send_message] Starting to send message for chat_id: {:?}",
            request.chat_id
        );

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

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
        let user_message_id = uuid::Uuid::new_v4().to_string();
        let chat_id = request.chat_id.as_ref().ok_or_else(|| {
            let error = "Chat ID is required";
            tracing::error!(
                "[MessageService::send_message] Validation failed: {}",
                error
            );
            AppError::validation_error(error)
        })?;

        let user_message = Message {
            id: user_message_id.clone(),
            chat_id: chat_id.clone(),
            role: MessageRole::User,
            content: last_message.content.clone(),
            config: Some(MessageConfig {
                temperature: request.parameters.as_ref().and_then(|p| p.temperature),
                top_p: request.parameters.as_ref().and_then(|p| p.top_p),
                max_tokens: request.parameters.as_ref().and_then(|p| p.max_tokens),
                stream: request.parameters.as_ref().and_then(|p| p.stream),
                model_id: Some(request.model_id.clone()),
                provider_id: Some(request.provider_id.clone()),
                system_prompt: None, // 系统提示由聊天级别设置决定
                mcp_servers: None,   // MCP 服务器由聊天级别设置决定
            }),
            attachments: request.attachments.as_ref().map(|attachments| {
                attachments
                    .iter()
                    .map(|att| MessageAttachment {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: att.name.clone(),
                        mime_type: att.mime_type.clone(),
                        size: att.data.len() as i64,
                        path: format!("/tmp/{}", uuid::Uuid::new_v4()), // 临时路径，实际应该保存文件
                    })
                    .collect()
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

        self.repository
            .create_message(&user_message)
            .await
            .map_err(|e| {
                let error = format!("Failed to save user message: {}", e);
                tracing::error!("[MessageService::send_message] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message] User message saved with ID: {}",
            user_message_id
        );

        // 3. 调用实际的 LLM API
        let assistant_message_id = uuid::Uuid::new_v4().to_string();
        let llm_response = self.call_llm_api(&request).await?;

        // 4. 保存助手消息到数据库
        let assistant_message = Message {
            id: assistant_message_id.clone(),
            chat_id: chat_id.clone(),
            role: MessageRole::Assistant,
            content: llm_response.content.clone(),
            config: Some(MessageConfig {
                temperature: request.parameters.as_ref().and_then(|p| p.temperature),
                top_p: request.parameters.as_ref().and_then(|p| p.top_p),
                max_tokens: request.parameters.as_ref().and_then(|p| p.max_tokens),
                stream: request.parameters.as_ref().and_then(|p| p.stream),
                model_id: Some(request.model_id.clone()),
                provider_id: Some(request.provider_id.clone()),
                system_prompt: None,
                mcp_servers: None,
            }),
            attachments: None,
            input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
            start_time: Some(now),
            end_time: Some(now),
            duration: llm_response.duration.map(|d| d as i64),
            created_at: now,
            updated_at: now,
        };

        self.repository
            .create_message(&assistant_message)
            .await
            .map_err(|e| {
                let error = format!("Failed to save assistant message: {}", e);
                tracing::error!("[MessageService::send_message] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message] Assistant message saved with ID: {}",
            assistant_message_id
        );

        let response = ChatResponse {
            chat_id: chat_id.clone(),
            message_id: assistant_message_id.clone(),
            content: llm_response.content,
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
    async fn call_llm_api(&self, request: &ChatRequest) -> Result<LlmApiResponse, AppError> {
        tracing::info!(
            "[MessageService::call_llm_api] Calling LLM API with provider: {}, model: {}",
            request.provider_id,
            request.model_id
        );

        // 1. 获取供应商配置
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

        // 2. 创建 LLM 客户端
        let llm_client = create_llm_client(&provider.provider_type).map_err(|e| {
            let error = format!(
                "Failed to create LLM client for provider type {}: {}",
                provider.provider_type, e
            );
            tracing::error!("[MessageService::call_llm_api] {}", error);
            e
        })?;

        // 3. 转换请求格式
        let api_request = self.convert_to_api_request(request)?;

        // 4. 调用 API
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
    fn convert_to_api_request(&self, request: &ChatRequest) -> Result<ApiChatRequest, AppError> {
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
            })
            .collect();

        Ok(ApiChatRequest {
            model: request.model_id.clone(),
            messages,
            temperature: request.parameters.as_ref().and_then(|p| p.temperature),
            max_tokens: request.parameters.as_ref().and_then(|p| p.max_tokens),
            stream: request.parameters.as_ref().and_then(|p| p.stream),
        })
    }

    /// 流式调用 LLM API
    pub async fn call_llm_api_stream<F>(
        &self,
        request: &ChatRequest,
        mut callback: F,
    ) -> Result<ChatResponse, AppError>
    where
        F: FnMut(String) + Send + 'static,
    {
        tracing::info!("[MessageService::call_llm_api_stream] Starting stream call with provider: {}, model: {}", 
            request.provider_id, request.model_id);

        // 1. 获取供应商配置
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

        // 2. 创建 LLM 客户端
        let llm_client = create_llm_client(&provider.provider_type).map_err(|e| {
            let error = format!(
                "Failed to create LLM client for provider type {}: {}",
                provider.provider_type, e
            );
            tracing::error!("[MessageService::call_llm_api_stream] {}", error);
            e
        })?;

        // 3. 转换请求格式
        let mut api_request = self.convert_to_api_request(request)?;
        api_request.stream = Some(true); // 强制启用流式

        // 4. 调用真实的 LLM API (非流式)
        // 由于完整的 SSE 流解析较复杂，我们使用真实的 API 调用，然后模拟流式输出
        let start_time = std::time::Instant::now();
        api_request.stream = Some(false); // 禁用流式，使用常规 API
        
        tracing::info!("[MessageService::call_llm_api_stream] Calling real LLM API...");
        let api_response = llm_client.chat(&provider, api_request).await?;
        
        // 提取真实响应内容
        let real_content = if let Some(choice) = api_response.choices.first() {
            choice.message.as_ref().map(|m| m.content.clone()).unwrap_or_default()
        } else {
            return Err(AppError::internal_error("No response from LLM API"));
        };
        
        tracing::info!("[MessageService::call_llm_api_stream] Received {} characters from API", real_content.len());
        
        // 5. 模拟流式输出真实内容
        let mut accumulated_content = String::new();
        let chars: Vec<char> = real_content.chars().collect();
        let total_chars = chars.len();
        let mut chunk_count = 0;
        
        // 按字符或单词逐步输出
        let mut i = 0;
        while i < total_chars {
            // 决定这次输出多少字符 (1-3个字符，模拟打字效果)
            let chunk_size = if i + 3 < total_chars { 
                rand::random::<usize>() % 3 + 1 
            } else { 
                total_chars - i 
            };
            
            let end = std::cmp::min(i + chunk_size, total_chars);
            let chunk: String = chars[i..end].iter().collect();
            accumulated_content.push_str(&chunk);
            
            callback(accumulated_content.clone());
            chunk_count += 1;
            
            tracing::debug!(
                "[MessageService::call_llm_api_stream] Streaming chunk {}: '{}'",
                chunk_count, chunk
            );
            
            // 短暂延迟以模拟真实流式效果
            let delay = if chunk.trim().is_empty() { 10 } else { 30 }; // 空格延迟更短
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            
            i = end;
        }

        let duration = start_time.elapsed().as_millis() as i64;
        let message_id = uuid::Uuid::new_v4().to_string();

        // 6. 创建最终响应 (使用真实API的统计信息)
        let response = ChatResponse {
            chat_id: request.chat_id.clone().unwrap_or_default(),
            message_id: message_id.clone(),
            content: real_content.clone(),  // 使用真实内容
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: api_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: api_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: api_response.usage.as_ref().map(|u| u.total_tokens),
            duration: Some(duration),
        };

        tracing::info!(
            "[MessageService::call_llm_api_stream] Real API call completed with simulated streaming, chunks: {}, total_content_length: {}, duration: {}ms",
            chunk_count, real_content.len(), duration
        );
        Ok(response)
    }

    /// 从 API 响应格式转换
    fn convert_from_api_response(
        &self,
        api_response: crate::clients::api_provider::ChatResponse,
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
    pub async fn regenerate_message(&self, message_id: UUID) -> Result<ChatResponse, AppError> {
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
        let regenerate_request = ChatRequest {
            chat_id: Some(message.chat_id.clone()),
            artifact_id: None,
            model_id: config.and_then(|c| c.model_id.clone()).unwrap_or_default(),
            provider_id: config
                .and_then(|c| c.provider_id.clone())
                .unwrap_or_default(),
            parameters: Some(crate::models::ModelParameters {
                temperature: config.and_then(|c| c.temperature),
                top_p: config.and_then(|c| c.top_p),
                max_tokens: config.and_then(|c| c.max_tokens),
                context_length: None,
                stream: config.and_then(|c| c.stream),
            }),
            messages: chat_messages
                .iter()
                .filter(|m| m.role != MessageRole::Assistant || m.id != message_id) // 排除要重新生成的消息
                .map(|m| crate::models::ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
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

        Ok(ChatResponse {
            chat_id: message.chat_id,
            message_id,
            content: new_content,
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
