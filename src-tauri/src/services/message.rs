// 消息服务实现

use crate::llm_client::{create_llm_client};
use crate::llm_client::types::{
    ChatMessage, ChatRequest,
    ChatToolCall, ChatToolChoice, RequestTool, RequestToolFunction,
    ChatMessageRole, ChatResponse,
};
use crate::models::{
    AppError, McpServer, McpServerStatus, Message, MessageConfig, MessageRequest,
    MessageResponse, UUID,
};
use crate::services::{ChatService, Database, McpService, ProviderService};
use crate::storage::MessageRepository;
use std::sync::Arc;

/// 流式数据结构
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
}


/// 消息服务
#[derive(Clone)]
pub struct MessageService {
    repository: MessageRepository,
    provider_service: Arc<ProviderService>,
    chat_service: Arc<ChatService>,
    mcp_service: Arc<McpService>,
}

impl MessageService {
    pub fn new(
        db: Arc<Database>,
        provider_service: Arc<ProviderService>,
        chat_service: Arc<ChatService>,
        mcp_service: Arc<McpService>,
    ) -> Self {
        Self {
            repository: MessageRepository::new(db),
            provider_service,
            chat_service,
            mcp_service,
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

        // 验证最后一条消息
        let last_message = &request.messages[request.messages.len() - 1];
        if last_message.role != ChatMessageRole::User {
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
        // 获取下一个 turn_id 用于这轮对话
        let turn_id = Some(self.repository.get_next_turn_id(chat_id).await.map_err(|e| {
            let error = format!("Failed to get next turn_id: {}", e);
            tracing::error!("[MessageService::send_message] Database error: {}", error);
            e
        })?);
        let user_message_id = self
            .save_user_message(chat_id, &last_message.content, config.clone(), turn_id.clone())
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
        let llm_response = self.call_llm_api(&request).await?;

        // 4. 保存助手消息到数据库
        // let config = Self::extract_message_config_from_chat(&request, &chat);
        let now = chrono::Utc::now().timestamp_millis();
        let assistant_message_id = self
            .save_assistant_message(
                chat_id,
                &llm_response.content,
                llm_response.reasoning.clone(),
                llm_response.tool_calls.clone(),
                config.clone(),
                now,
                llm_response.duration.unwrap_or(0),
                llm_response.input_tokens,
                llm_response.output_tokens,
                llm_response.total_tokens,
                turn_id.clone(),
            )
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

        let response = MessageResponse {
            chat_id: chat_id.clone(),
            message_id: assistant_message_id.clone(),
            content: llm_response.content,
            reasoning: llm_response.reasoning,
            model_id: request.model_id,
            provider_id: request.provider_id,
            input_tokens: llm_response.input_tokens,
            output_tokens: llm_response.output_tokens,
            total_tokens: llm_response.total_tokens,
            duration: llm_response.duration,
            tool_calls: llm_response.tool_calls,
        };

        tracing::info!(
            "[MessageService::send_message] Successfully completed for chat_id: {:?}, message_id: {}",
            chat_id, assistant_message_id
        );
        Ok(response)
    }

    /// 发送流式消息 - 处理完整的聊天逻辑包括消息保存
    pub async fn send_message_stream<F>(
        &self,
        request: MessageRequest,
        callback: F,
    ) -> Result<MessageResponse, AppError>
    where
        F: FnMut(StreamChunk) + Send + 'static,
    {
        tracing::info!(
            "[MessageService::send_message_stream] Starting to send streaming message for chat_id: {:?}",
            request.chat_id
        );

        // 1. 验证请求参数
        if request.messages.is_empty() {
            let error = "No messages provided";
            tracing::error!(
                "[MessageService::send_message_stream] Validation failed: {}",
                error
            );
            return Err(AppError::validation_error(error));
        }

        // 验证最后一条消息
        let last_message = &request.messages[request.messages.len() - 1];
        if last_message.role != ChatMessageRole::User {
            let error = "Last message must be from user";
            tracing::error!(
                "[MessageService::send_message_stream] Validation failed: {}",
                error
            );
            return Err(AppError::validation_error(error));
        }

        // 2. 保存用户消息到数据库
        let chat_id = request.chat_id.as_ref().ok_or_else(|| {
            let error = "Chat ID is required";
            tracing::error!(
                "[MessageService::send_message_stream] Validation failed: {}",
                error
            );
            AppError::validation_error(error)
        })?;

        // 从 chats 表获取配置参数
        let chat = self.get_chat_config(chat_id).await.map_err(|e| {
            let error = format!("Failed to get chat config: {}", e);
            tracing::error!("[MessageService::send_message_stream] Database error: {}", error);
            e
        })?;

        let config = Self::extract_message_config_from_chat(&request, &chat);
        // 获取下一个 turn_id 用于这轮对话
        let turn_id = Some(self.repository.get_next_turn_id(chat_id).await.map_err(|e| {
            let error = format!("Failed to get next turn_id: {}", e);
            tracing::error!("[MessageService::send_message_stream] Database error: {}", error);
            e
        })?);
        let user_message_id = self
            .save_user_message(chat_id, &last_message.content, config.clone(), turn_id.clone())
            .await
            .map_err(|e| {
                let error = format!("Failed to save user message: {}", e);
                tracing::error!("[MessageService::send_message_stream] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message_stream] User message saved with ID: {}",
            user_message_id
        );

        // 3. 调用流式 LLM API
        let llm_response = self.call_llm_api_stream(&request, callback).await?;

        // 4. 保存助手消息到数据库
        let now = chrono::Utc::now().timestamp_millis();
        let assistant_message_id = self
            .save_assistant_message(
                chat_id,
                &llm_response.content,
                llm_response.reasoning.clone(),
                llm_response.tool_calls.clone(),
                config.clone(),
                now,
                llm_response.duration.unwrap_or(0),
                llm_response.input_tokens,
                llm_response.output_tokens,
                llm_response.total_tokens,
                turn_id.clone(),
            )
            .await
            .map_err(|e| {
                let error = format!("Failed to save assistant message: {}", e);
                tracing::error!("[MessageService::send_message_stream] Database error: {}", error);
                e
            })?;

        tracing::info!(
            "[MessageService::send_message_stream] Assistant message saved with ID: {}",
            assistant_message_id
        );

        let response = MessageResponse {
            chat_id: chat_id.clone(),
            message_id: assistant_message_id.clone(),
            content: llm_response.content,
            reasoning: llm_response.reasoning,
            model_id: request.model_id,
            provider_id: request.provider_id,
            input_tokens: llm_response.input_tokens,
            output_tokens: llm_response.output_tokens,
            total_tokens: llm_response.total_tokens,
            duration: llm_response.duration,
            tool_calls: llm_response.tool_calls,
        };

        tracing::info!(
            "[MessageService::send_message_stream] Successfully completed for chat_id: {:?}, message_id: {}",
            chat_id, assistant_message_id
        );
        Ok(response)
    }

    /// 调用 LLM API
    async fn call_llm_api(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, AppError> {
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

        // 检查 API Key 是否存在
        if provider.api_key.is_empty() {
            tracing::error!(
                "[MessageService::call_llm_api] Provider {} has empty API key",
                request.provider_id
            );
            return Err(AppError::validation_error(
                "Provider has no API key configured",
            ));
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
        let api_request = self
            .convert_to_api_request(request, &chat)
            .await?;

        // 5. 调用 API
        let start_time = std::time::Instant::now();
        let api_response = llm_client
            .chat(&provider, api_request)
            .await
            .map_err(|e| {
                let error = format!("LLM API call failed: {}", e);
                tracing::error!("[MessageService::call_llm_api] {}", error);
                e
            })?;

        let duration = start_time.elapsed().as_millis() as f64;
        let llm_response = self.convert_from_api_response(api_response, duration, request)?;

        tracing::info!(
            "[MessageService::call_llm_api] API call successful, duration: {}ms",
            duration
        );

        Ok(llm_response)
    }



    /// 转换为 API 请求格式
    async fn convert_to_api_request(
        &self,
        request: &MessageRequest,
        chat: &crate::models::Chat
    ) -> Result<ChatRequest, AppError> {
        let messages: Vec<ChatMessage> = request
            .messages
            .iter()
            .map(|msg| ChatMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
                reasoning: msg.reasoning.clone(),
                tool_calls: msg.tool_calls.clone(),
                tool_call_id: msg.tool_call_id.clone(),
            })
            .collect();

        let tools = self.prepare_tools(&chat).await?;

        Ok(ChatRequest {
            model: request.model_id.clone(),
            messages,
            temperature: chat.temperature,
            max_tokens: chat.max_tokens,
            stream: Some(false),
            tools: if tools.is_empty() { None } else { Some(tools.clone()) },
            tool_choice: if tools.is_empty() { None } else { Some(ChatToolChoice::Auto) },
            parallel_tool_calls: if tools.is_empty() { None } else { Some(true) },
        })
    }

    async fn prepare_tools(
        &self,
        chat: &crate::models::Chat,
    ) -> Result<Vec<RequestTool>, AppError> {
        if chat.mcp_servers.is_empty() {
            return Ok(Vec::new());
        }

        let servers = self
            .mcp_service
            .get_servers_by_ids(&chat.mcp_servers)
            .await?;

        let active_servers: Vec<McpServer> = servers
            .into_iter()
            .filter(|server| server.enabled && matches!(server.status, McpServerStatus::Ready))
            .collect();

        if active_servers.is_empty() {
            return Ok(Vec::new());
        }

        let mut tools = Vec::new();

        for server in active_servers {
            for tool in &server.tools {

                let description = tool
                    .description
                    .clone()
                    .filter(|desc| !desc.is_empty())
                    .unwrap_or_else(|| {
                        let display_name = server
                            .display_name
                            .as_deref()
                            .filter(|name| !name.is_empty())
                            .unwrap_or(&server.name);
                        format!("MCP 服务器 {} 的工具 {}", display_name, tool.name)
                    });

                tools.push(RequestTool {
                    tool_type: "function".to_string(),
                    function: RequestToolFunction {
                        name: tool.name.clone(),
                        description,
                        parameters: tool.input_schema.clone(),
                    },
                });
            }
        }

        Ok(tools)
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
            return Err(AppError::validation_error(
                "Chat ID is required for streaming",
            ));
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
        let mut api_request = self
            .convert_to_api_request(request, &chat)
            .await?;
        api_request.stream = Some(true); // 强制启用流式


        // 6. 调用 LLM 流式 API
        let start_time = std::time::Instant::now();

        tracing::info!("[MessageService::call_llm_api_stream] Calling real LLM streaming API...");
        let mut stream = llm_client.chat_stream(&provider, api_request).await?;

        let mut accumulated_content = String::new();
        let mut accumulated_reasoning = String::new();
        let mut all_tool_calls: Vec<ChatToolCall> = Vec::new();
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
                            if let Some(chunk) = &delta.content {
                                accumulated_content.push_str(chunk);
                            }

                            // 处理推理过程
                            if let Some(reasoning_chunk) = &delta.reasoning {
                                accumulated_reasoning.push_str(reasoning_chunk);
                            }

                            // 积累工具调用信息
                            if let Some(tool_calls) = &delta.tool_calls {
                                for tool_call_delta in tool_calls {
                                    let index = tool_call_delta.index as usize;

                                    // 确保有足够的空间
                                    while all_tool_calls.len() <= index {
                                        all_tool_calls.push(ChatToolCall {
                                            id: String::new(),
                                            tool_type: String::new(),
                                            function: crate::llm_client::types::ChatToolFunction {
                                                name: String::new(),
                                                arguments: String::new(),
                                            },
                                        });
                                    }

                                    let tool_call = &mut all_tool_calls[index];

                                    // 更新工具调用信息
                                    if let Some(id) = &tool_call_delta.id {
                                        tool_call.id = id.clone();
                                    }
                                    if let Some(tool_type) = &tool_call_delta.tool_type {
                                        tool_call.tool_type = tool_type.clone();
                                    }
                                    if let Some(function) = &tool_call_delta.function {
                                        if let Some(name) = &function.name {
                                            tool_call.function.name = name.clone();
                                        }
                                        if let Some(arguments) = &function.arguments {
                                            tool_call.function.arguments.push_str(arguments);
                                        }
                                    }
                                }
                            }


                            callback(StreamChunk {
                                content: accumulated_content.clone(),
                                reasoning: delta.reasoning.clone(),
                                tool_calls: Some(all_tool_calls.clone()),
                            });
                            chunk_count += 1;

                            tracing::debug!(
                                "[MessageService::call_llm_api_stream] Real streaming chunk {}: content='{}', reasoning='{}'",
                                chunk_count,
                                delta.content.as_deref().unwrap_or(""),
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
        tracing::info!(
            "[MessageService::call_llm_api_stream] Real streaming API call completed, chunks: {}, total_content_length: {}, duration: {}ms",
            chunk_count, accumulated_content.len(), duration
        );

        // 7. 构造并返回 MessageResponse
        let response = MessageResponse {
            chat_id: request.chat_id.clone().unwrap_or_default(),
            message_id: uuid::Uuid::new_v4().to_string(), // 临时ID，实际使用时会被覆盖
            content: accumulated_content,
            reasoning: if accumulated_reasoning.is_empty() {
                None
            } else {
                Some(accumulated_reasoning)
            },
            tool_calls: if all_tool_calls.is_empty() {
                None
            } else {
                Some(all_tool_calls)
            },
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: None, // 流式API通常不返回token统计
            output_tokens: None,
            total_tokens: None,
            duration: Some(duration),
        };

        Ok(response)
    }

    /// 构造并保存用户消息
    async fn save_user_message(
        &self,
        chat_id: &str,
        content: &str,
        config: Option<MessageConfig>,
        turn_id: Option<i32>,
    ) -> Result<String, AppError> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        let message = Message {
            id: message_id.clone(),
            chat_id: chat_id.to_string(),
            role: ChatMessageRole::User,
            content: content.to_string(),
            reasoning: None, // 用户消息没有推理过程
            config,
            tool_calls: None,
            turn_id,
            tool_call_id: None, // 用户消息没有关联的工具调用
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
        tool_calls: Option<Vec<ChatToolCall>>,
        config: Option<MessageConfig>,
        start_time_millis: i64,
        duration_ms: i64,
        input_tokens: Option<i32>,
        output_tokens: Option<i32>,
        total_tokens: Option<i32>,
        turn_id: Option<i32>,
    ) -> Result<String, AppError> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        let message = Message {
            id: message_id.clone(),
            chat_id: chat_id.to_string(),
            role: ChatMessageRole::Assistant,
            content: content.to_string(),
            reasoning,
            tool_calls,
            config,
            turn_id,
            tool_call_id: None, // 助手消息没有关联的工具调用ID（工具调用本身在tool_calls字段中）
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
    fn extract_message_config_from_chat(
        request: &MessageRequest,
        chat: &crate::models::Chat,
    ) -> Option<MessageConfig> {
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

    /// 从消息和聊天配置中提取消息配置
    fn extract_message_config_from_request_and_chat(
        message: &Message,
        chat: &crate::models::Chat,
    ) -> Option<MessageConfig> {
        Some(MessageConfig {
            model_id: message.config.as_ref().and_then(|c| c.model_id.clone()),
            provider_id: message.config.as_ref().and_then(|c| c.provider_id.clone()),
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
        api_response: ChatResponse,
        duration: f64,
        request: &MessageRequest,
    ) -> Result<MessageResponse, AppError> {
        let choice = api_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AppError::internal_error("No choices in API response"))?;

        let message = choice
            .delta
            .ok_or_else(|| AppError::internal_error("No message in API choice"))?;

        Ok(MessageResponse {
            chat_id: request.chat_id.clone().unwrap_or_default(),
            message_id: uuid::Uuid::new_v4().to_string(), // 临时ID，实际使用时会被覆盖
            content: message.content,
            reasoning: message.reasoning,
            tool_calls: message.tool_calls,
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: api_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: api_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: api_response.usage.as_ref().map(|u| u.total_tokens),
            duration: Some(duration as i64),
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

    /// 执行工具调用并发送结果给模型继续对话
    pub async fn execute_tool_calls<F>(
        &self,
        message_id: String,
        tool_call_id: String,
        callback: F,
    ) -> Result<MessageResponse, AppError>
    where
        F: FnMut(StreamChunk) + Send + 'static,
    {
        tracing::info!(
            "[MessageService::execute_tool_calls] Executing tool call {} for message: {}",
            tool_call_id,
            message_id
        );

        // 1. 获取消息
        tracing::debug!(
            "[MessageService::execute_tool_calls] Attempting to get message with ID: {}",
            message_id
        );
        let message = self.get_message(message_id.clone()).await
            .map_err(|e| {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Failed to get message {}: {}",
                    message_id, e
                );
                e
            })?;

        // 2. 验证消息是否为 assistant消息且包含工具调用
        if message.role != ChatMessageRole::Assistant {
            return Err(AppError::validation_error(
                "Can only execute tool calls on assistant messages"
            ));
        }

        // 3. 从数据库中获取工具调用信息
        let stored_tool_calls = message.tool_calls.as_ref().ok_or_else(|| {
            AppError::validation_error("Message does not contain any tool calls")
        })?;

        // 4. 根据 tool_call_id 找到要执行的工具调用
        let tool_call = stored_tool_calls
            .iter()
            .find(|tc| tc.id == tool_call_id)
            .ok_or_else(|| {
                AppError::validation_error(&format!(
                    "Tool call with ID {} not found in message",
                    tool_call_id
                ))
            })?
            .clone();

        // 5. 执行工具调用并构造工具结果消息
        let result = match self.mcp_service.execute_tool(&tool_call.function.name, &tool_call.function.arguments).await {
            Ok(result) => result,
            Err(error) => {
                tracing::error!(
                    "[MessageService::execute_tool_calls] Tool execution failed: {}",
                    error
                );
                format!("工具执行失败: {}", error.message)
            }
        };

        // 5. 先将工具调用结果作为消息保存到数据库
        let tool_result_message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: message.chat_id.clone(),
            role: ChatMessageRole::Tool,
            content: result,
            reasoning: None,
            tool_calls: None,
            turn_id: message.turn_id.clone(), // 使用原始消息的 turn_id 保持在同一轮对话
            tool_call_id: Some(tool_call_id.clone()), // 关联对应的工具调用 ID
            config: None,
            attachments: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        };

        self.repository.create_message(&tool_result_message).await?;

        // 6. 获取 chat 配置
        let chat = self.get_chat_config(&message.chat_id).await?;

        // 7. 构建包含工具调用结果的新请求，调用 send_message
        let mut request_messages = Vec::new();

        // 添加 system 消息（如果有系统提示词）
        if let Some(system_prompt) = &chat.system_prompt {
            if !system_prompt.trim().is_empty() {
                request_messages.push(ChatMessage {
                    role: ChatMessageRole::System,
                    content: system_prompt.clone(),
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }

        // 添加当前 turn_id 的所有消息（按时间顺序）
        if let Some(turn_id) = message.turn_id {
            if let Ok(turn_messages) = self.repository
                .get_messages_by_chat_and_turn(&message.chat_id, turn_id)
                .await
            {
                for turn_message in turn_messages {
                    request_messages.push(ChatMessage {
                        role: turn_message.role,
                        content: turn_message.content,
                        reasoning: None,
                        tool_calls: turn_message.tool_calls,
                        tool_call_id: turn_message.tool_call_id,
                    });
                }
            }
        }

        let request = MessageRequest {
            chat_id: Some(message.chat_id.clone()),
            model_id: chat.model_id.unwrap_or_default(),
            provider_id: chat.provider_id.unwrap_or_default(),
            messages: request_messages,
            attachments: None,
        };

        tracing::info!(
            "[MessageService::execute_tool_calls] Request: {:?}",
            request
        );

        // 调用流式 LLM API 处理响应
        let llm_response = self.call_llm_api_stream(&request, callback).await?;

        // 保存助手消息到数据库
        let now = chrono::Utc::now().timestamp_millis();
        let chat = self.get_chat_config(&message.chat_id).await?;
        let config = Self::extract_message_config_from_chat(&request, &chat);

        let assistant_message_id = self
            .save_assistant_message(
                &message.chat_id,
                &llm_response.content,
                llm_response.reasoning.clone(),
                llm_response.tool_calls.clone(),
                config,
                now,
                llm_response.duration.unwrap_or(0),
                llm_response.input_tokens,
                llm_response.output_tokens,
                llm_response.total_tokens,
                message.turn_id,
            )
            .await?;

        let response = MessageResponse {
            chat_id: message.chat_id.clone(),
            message_id: assistant_message_id,
            content: llm_response.content,
            reasoning: llm_response.reasoning,
            model_id: request.model_id,
            provider_id: request.provider_id,
            input_tokens: llm_response.input_tokens,
            output_tokens: llm_response.output_tokens,
            total_tokens: llm_response.total_tokens,
            duration: llm_response.duration,
            tool_calls: llm_response.tool_calls,
        };

        Ok(response)
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
        if message.role != ChatMessageRole::Assistant {
            let error = "Can only regenerate assistant messages";
            tracing::error!(
                "[MessageService::regenerate_message] Validation failed for message_id {}: {}",
                message_id,
                error
            );
            return Err(AppError::validation_error(error));
        }

        // 3. 获取聊天的历史消息（包含所有角色），重新调用 LLM API
        let chat_messages = self
            .repository
            .get_all_messages_by_chat(&message.chat_id, 100, 0)
            .await?;

        // 构造重新生成请求（使用原始请求参数）
        let config = message.config.as_ref();
        // 获取聊天的系统提示词
        let chat = self.chat_service.get_chat(message.chat_id.clone()).await?;

        // 构建消息数组，如果有系统提示词则添加到开头
        let mut request_messages = Vec::new();
        if let Some(system_prompt) = &chat.system_prompt {
            if !system_prompt.trim().is_empty() {
                request_messages.push(ChatMessage {
                    role: ChatMessageRole::System,
                    content: system_prompt.clone(),
                    reasoning: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }

        // 添加历史消息（排除要重新生成的助手消息）
        request_messages.extend(
            chat_messages
                .iter()
                .filter(|m| m.role != ChatMessageRole::Assistant || m.id != message_id)
                .map(|m| ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    reasoning: m.reasoning.clone(),
                    tool_calls: m.tool_calls.clone(),
                    tool_call_id: None,
                }),
        );

        let regenerate_request = MessageRequest {
            chat_id: Some(message.chat_id.clone()),
            model_id: config.and_then(|c| c.model_id.clone()).unwrap_or_default(),
            provider_id: config
                .and_then(|c| c.provider_id.clone())
                .unwrap_or_default(),
            messages: request_messages,
            attachments: None,
        };

        // 调用 LLM API 重新生成
        let llm_response = self
            .call_llm_api(&regenerate_request)
            .await?;
        let new_content = llm_response.content.clone();

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

        self.repository
            .update_message_reasoning(&message_id, llm_response.reasoning.as_deref(), now)
            .await?;

        // 清理配置中的待执行状态
        let final_config = MessageConfig {
            temperature: chat.temperature,
            top_p: chat.top_p,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            model_id: Some(regenerate_request.model_id.clone()),
            provider_id: Some(regenerate_request.provider_id.clone()),
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
                    };
        self.repository
            .update_message_config(&message_id, Some(&final_config), now)
            .await?;

        tracing::info!(
            "[MessageService::regenerate_message] Successfully regenerated message: {}",
            message_id
        );

        Ok(MessageResponse {
            chat_id: message.chat_id,
            message_id,
            content: llm_response.content,
            reasoning: llm_response.reasoning,
            model_id: config.and_then(|c| c.model_id.clone()).unwrap_or_default(),
            provider_id: config
                .and_then(|c| c.provider_id.clone())
                .unwrap_or_default(),
            input_tokens: llm_response.input_tokens,
            output_tokens: llm_response.output_tokens,
            total_tokens: llm_response.total_tokens,
            duration: llm_response.duration,
            tool_calls: llm_response.tool_calls,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MessageConfig, MessageRequest, ModelParameters};
    use crate::llm_client::types::ChatMessage;
    use crate::llm_client::types::ChatMessageRole;
    use crate::services::{ChatService, McpService, ProviderService};
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

    async fn setup_services() -> (Arc<ChatService>, MessageService, String) {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let mcp_service = Arc::new(McpService::new(db.clone()));
        let chat_service = Arc::new(ChatService::new(db.clone(), provider_service.clone()));
        let message_service =
            MessageService::new(db, provider_service, chat_service.clone(), mcp_service);

        let chat = chat_service
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

        (chat_service, message_service, chat.id)
    }


    #[tokio::test]
    async fn creates_service_successfully() {
        let db = create_test_database().await;
        let provider_service = Arc::new(ProviderService::new(db.clone()));
        let mcp_service = Arc::new(McpService::new(db.clone()));
        let chat_service = Arc::new(ChatService::new(db.clone(), provider_service.clone()));
        let _service = MessageService::new(db, provider_service, chat_service, mcp_service);
    }

    #[tokio::test]
    async fn send_message_requires_chat_id() {
        let (_chat_service, message_service, _chat_id) = setup_services().await;

        let request = MessageRequest {
            chat_id: None,
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            messages: vec![ChatMessage {
                role: ChatMessageRole::User,
                content: "Hello".to_string(),
                reasoning: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            attachments: None,
        };

        let err = message_service
            .send_message(request)
            .await
            .expect_err("expected validation error");

        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn get_message_returns_not_found() {
        let (_chat_service, message_service, _chat_id) = setup_services().await;

        let err = message_service
            .get_message("nonexistent_message".to_string())
            .await
            .expect_err("expected not found");

        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    #[ignore = "requires provider setup"]
    async fn send_message_with_provider_integration() {
        let (_chat_service, _message_service, _chat_id) = setup_services().await;
    }

    #[test]
    fn message_config_serialization_roundtrip() {
        let config = MessageConfig {
            temperature: Some(0.8),
            top_p: Some(0.9),
            max_tokens: Some(1000),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: Some(vec!["server1".to_string()]),
                    };

        let json = serde_json::to_string(&config).unwrap();
        let roundtrip: MessageConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(roundtrip.temperature, config.temperature);
        assert_eq!(roundtrip.model_id, config.model_id);
        assert_eq!(roundtrip.provider_id, config.provider_id);
    }

    #[test]
    fn model_parameters_default_values() {
        let defaults = ModelParameters::default();

        assert_eq!(defaults.temperature, Some(0.7));
        assert_eq!(defaults.top_p, Some(0.9));
        assert_eq!(defaults.max_tokens, Some(2048));
        assert_eq!(defaults.context_length, Some(4096));
        assert_eq!(defaults.stream, Some(true));
    }
}
