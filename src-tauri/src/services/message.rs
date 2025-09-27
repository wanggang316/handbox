// 消息服务实现

use crate::llm_client::create_llm_client;
use crate::llm_client::types::{
    ChatFunction, ChatMessage, ChatRequest, ChatResponse, ChatTool,
    ChatToolCall, ChatToolChoice, ChatUsage,
};
use crate::mcp_client::McpClientFactory;
use crate::models::{
    AppError, McpServer, McpServerStatus, McpTool, Message, MessageConfig, MessageRequest,
    MessageResponse, MessageRole, PendingMcpCall, PendingMcpToolCall, UUID,
};
use crate::services::{ChatService, Database, McpService, ProviderService};
use crate::storage::MessageRepository;
use rmcp::model::CallToolResult;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 流式数据结构
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub reasoning: Option<String>,
}


#[derive(Clone)]
struct PendingToolCallState {
    message_id: String,
    chat_id: String,
    provider_id: String,
    model_id: String,
    api_request: ChatRequest,
    tool_calls: Vec<ChatToolCall>,
}

struct PendingCallResult {
    pending_id: String,
    state: PendingToolCallState,
    placeholder_content: String,
    placeholder_reasoning: Option<String>,
    pending_call: PendingMcpCall,
}

enum ToolCallOutcome {
    Final(ChatResponse),
    Pending(PendingCallResult),
}

enum LlmCallResult {
    Final(LlmApiResponse),
    Pending(MessageResponse),
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
    mcp_service: Arc<McpService>,
    pending_calls: Arc<Mutex<HashMap<String, PendingToolCallState>>>,
    executing_calls: Arc<Mutex<HashSet<String>>>,
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
            pending_calls: Arc::new(Mutex::new(HashMap::new())),
            executing_calls: Arc::new(Mutex::new(HashSet::new())),
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
        let user_message_id = self
            .save_user_message(chat_id, &last_message.content, config)
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
        match self.call_llm_api(&request, None).await? {
            LlmCallResult::Final(llm_response) => {
                // 4. 保存助手消息到数据库
                let config = Self::extract_message_config_from_chat(&request, &chat);
                let now = chrono::Utc::now().timestamp_millis();
                let assistant_message_id = self
                    .save_assistant_message(
                        chat_id,
                        &llm_response.content,
                        llm_response.reasoning.clone(),
                        config,
                        now,
                        llm_response.duration.unwrap_or(0.0) as i64,
                        llm_response.usage.as_ref().map(|u| u.prompt_tokens),
                        llm_response.usage.as_ref().map(|u| u.completion_tokens),
                        llm_response.usage.as_ref().map(|u| u.total_tokens),
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
                    input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
                    output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
                    total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
                    duration: llm_response.duration.map(|d| d as i64),
                    pending_mcp_call: None,
                };

                tracing::info!(
                    "[MessageService::send_message] Successfully completed for chat_id: {:?}, message_id: {}",
                    chat_id, assistant_message_id
                );
                Ok(response)
            }
            LlmCallResult::Pending(response) => {
                tracing::info!(
                    "[MessageService::send_message] Pending MCP confirmation for chat_id: {:?}",
                    chat_id
                );
                Ok(response)
            }
        }
    }

    /// 调用 LLM API
    async fn call_llm_api(
        &self,
        request: &MessageRequest,
        reuse_message_id: Option<String>,
    ) -> Result<LlmCallResult, AppError> {
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
        let api_request = self
            .convert_to_api_request(request, &chat)
            .await?;

        // 5. 调用 API
        let start_time = std::time::Instant::now();
        let api_response = llm_client
            .chat(&provider, api_request.clone())
            .await
            .map_err(|e| {
                let error = format!("LLM API call failed: {}", e);
                tracing::error!("[MessageService::call_llm_api] {}", error);
                e
            })?;

        let outcome = self.process_tool_calls_with_state(
            &chat.id,
            &request.provider_id,
            &request.model_id,
            api_request,
            api_response,
            reuse_message_id.clone(),
        )
        .await?;

        let duration = start_time.elapsed().as_millis() as f64;

        match outcome {
            ToolCallOutcome::Final(api_response) => {
                let llm_response = self.convert_from_api_response(api_response, duration)?;
                tracing::info!(
                    "[MessageService::call_llm_api] API call successful, duration: {}ms",
                    duration
                );
                Ok(LlmCallResult::Final(llm_response))
            }
            ToolCallOutcome::Pending(pending) => {
                let response = self
                    .store_pending_call_and_build_response(
                        &chat,
                        request,
                        pending,
                        duration,
                        reuse_message_id.is_some(),
                    )
                    .await?;
                Ok(LlmCallResult::Pending(response))
            }
        }
    }

    /// 直接执行工具调用（从 toolCallDeltas 创建）
    pub async fn execute_tool_calls_directly(
        &self,
        message_id: String,
        tool_call_deltas: Vec<crate::llm_client::types::ChatToolCallDelta>,
    ) -> Result<MessageResponse, AppError> {
        tracing::info!(
            "[MessageService::execute_tool_calls_directly] Executing {} tool calls for message: {}",
            tool_call_deltas.len(),
            message_id
        );

        // 获取消息以获取聊天配置
        let message = self.repository.get_message_by_id(&message_id).await?
            .ok_or_else(|| AppError::validation_error("Message not found"))?;

        let chat = self.get_chat_config(&message.chat_id).await?;

        // 将 ChatToolCallDelta 转换为 ChatToolCall
        let tool_calls: Vec<crate::llm_client::types::ChatToolCall> = tool_call_deltas
            .into_iter()
            .filter_map(|delta| {
                match (&delta.id, &delta.name, &delta.arguments) {
                    (Some(id), Some(name), Some(arguments)) => {
                        Some(crate::llm_client::types::ChatToolCall {
                            id: id.clone(),
                            tool_type: delta.tool_type.clone(),
                            name: name.clone(),
                            arguments: arguments.clone(),
                        })
                    }
                    _ => {
                        tracing::warn!("Skipping incomplete tool call delta: {:?}", delta);
                        None
                    }
                }
            })
            .collect();

        if tool_calls.is_empty() {
            return Err(AppError::validation_error("No valid tool calls found"));
        }

        // 获取 LLM 客户端和提供商
        let provider_id = message.config.as_ref()
            .and_then(|c| c.provider_id.as_ref())
            .cloned()
            .or_else(|| chat.provider_id.clone())
            .unwrap_or_default();

        let provider = self.provider_service
            .get_provider(&provider_id)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get provider {}: {}", provider_id, e))
            })?;

        let llm_client = create_llm_client(&provider.provider_type)
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create LLM client for provider type {}: {}",
                    provider.provider_type, e))
            })?;

        // 执行工具调用
        let mut results = Vec::new();
        for call in &tool_calls {
            let start_time = std::time::Instant::now();
            let result = self.execute_tool_call_direct(&call).await;
            let _execution_time = start_time.elapsed().as_millis() as i64;
            results.push(result);
        }

        // 构建包含工具执行结果的新消息
        let model_id = message.config.as_ref()
            .and_then(|c| c.model_id.clone())
            .or_else(|| chat.model_id.clone())
            .unwrap_or_default();

        let mut api_request = ChatRequest {
            model: model_id.clone(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            stream: Some(false),
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
        };

        // 添加工具执行结果到消息中
        for (call, result) in tool_calls.iter().zip(results.iter()) {
            api_request.messages.push(ChatMessage {
                role: "tool".to_string(),
                content: result.clone(),
                reasoning: None,
                tool_calls: None,
                tool_call_deltas: None,
                tool_call_id: Some(call.id.clone()),
            });
        }

        // 调用 LLM 获取最终响应
        let start_time = std::time::Instant::now();
        let final_response = llm_client
            .chat(&provider, api_request)
            .await
            .map_err(|e| AppError::internal_error(&format!("LLM API call failed: {}", e)))?;

        let duration_ms = start_time.elapsed().as_millis() as f64;
        let llm_response = self.convert_from_api_response(final_response, duration_ms)?;

        // 更新原消息的内容
        let now = chrono::Utc::now().timestamp_millis();
        self.repository
            .update_message_content(&message_id, &llm_response.content, now)
            .await?;

        if let Some(reasoning) = &llm_response.reasoning {
            self.repository
                .update_message_reasoning(&message_id, Some(reasoning), now)
                .await?;
        }

        Ok(MessageResponse {
            chat_id: message.chat_id,
            message_id,
            content: llm_response.content,
            reasoning: llm_response.reasoning,
            model_id,
            provider_id,
            input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
            duration: llm_response.duration.map(|d| d as i64),
            pending_mcp_call: None,
        })
    }

    /// 执行待确认的 MCP 工具调用
    pub async fn execute_pending_mcp_call(
        &self,
        pending_id: String,
    ) -> Result<MessageResponse, AppError> {
        tracing::info!(
            "[MessageService::execute_pending_mcp_call] Executing pending MCP call: {}",
            pending_id
        );

        // 检查是否已经在执行中
        {
            let mut executing_map = self.executing_calls.lock().await;
            if executing_map.contains(&pending_id) {
                tracing::warn!(
                    "[MessageService::execute_pending_mcp_call] Pending call {} is already being executed",
                    pending_id
                );
                return Err(AppError::validation_error("This MCP tool call is already being executed"));
            }
            executing_map.insert(pending_id.clone());
        }

        // 在函数结束时清理执行状态
        let executing_calls_cleanup = self.executing_calls.clone();
        let pending_id_cleanup = pending_id.clone();

        let state = {
            let mut pending_map = self.pending_calls.lock().await;
            pending_map.remove(&pending_id).ok_or_else(|| {
                tracing::warn!(
                    "[MessageService::execute_pending_mcp_call] Pending call {} not found - it may have already been processed",
                    pending_id
                );
                AppError::validation_error("This MCP tool call has already been executed or is no longer available")
            })?
        };

        let chat = self.get_chat_config(&state.chat_id).await?;

        let provider = self
            .provider_service
            .get_provider(&state.provider_id)
            .await
            .map_err(|e| {
                AppError::validation_error(&format!(
                    "Failed to get provider {}: {}",
                    state.provider_id, e
                ))
            })?;

        if provider.api_key.is_empty() {
            tracing::error!(
                "[MessageService::execute_pending_mcp_call] Provider {} has empty API key",
                state.provider_id
            );
            return Err(AppError::validation_error(
                "Provider has no API key configured",
            ));
        }

        let llm_client = create_llm_client(&provider.provider_type).map_err(|e| {
            AppError::internal_error(&format!(
                "Failed to create LLM client for provider type {}: {}",
                provider.provider_type, e
            ))
        })?;

        // 附加工具执行结果
        let mut api_request = state.api_request.clone();

        for call in &state.tool_calls {
            let start_time = std::time::Instant::now();
            let result = self.execute_tool_call_direct(call).await;
            let _execution_time = start_time.elapsed().as_millis() as i64;

            // 简化版本，不再跟踪复杂的工具执行结果

            api_request.messages.push(ChatMessage {
                role: "tool".to_string(),
                content: result.clone(),
                reasoning: None,
                tool_calls: None,
                tool_call_deltas: None,
                tool_call_id: Some(call.id.clone()),
            });
        }

        // 简化版本，不再需要复杂的工具执行结果跟踪
        api_request.stream = Some(false);

        let start_time = std::time::Instant::now();
        let followup_response = llm_client
            .chat(&provider, api_request.clone())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("LLM API call failed after MCP execution: {}", e))
            })?;
        let duration_ms = start_time.elapsed().as_millis() as i64;

        let outcome = self
            .process_tool_calls_with_state(
                &state.chat_id,
                &state.provider_id,
                &state.model_id,
                api_request,
                followup_response,
                None, // 不复用消息ID，创建新的消息记录
            )
            .await?;

        let result = match outcome {
            ToolCallOutcome::Final(final_response) => {
                let llm_response =
                    self.convert_from_api_response(final_response, duration_ms as f64)?;
                self.finalize_pending_message(&state, &chat, &llm_response, duration_ms)
                    .await?;

                Ok(MessageResponse {
                    chat_id: state.chat_id.clone(),
                    message_id: state.message_id.clone(),
                    content: llm_response.content,
                    reasoning: llm_response.reasoning,
                    model_id: state.model_id,
                    provider_id: state.provider_id,
                    input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
                    output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
                    total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
                    duration: llm_response.duration.map(|d| d as i64),
                    pending_mcp_call: None,
                })
            }
            ToolCallOutcome::Pending(new_pending) => {
                let PendingCallResult {
                    pending_id: new_pending_id,
                    state: new_state,
                    placeholder_content,
                    placeholder_reasoning,
                    pending_call,
                } = new_pending;

                self.update_pending_message(
                    &new_state,
                    &chat,
                    &placeholder_content,
                    &placeholder_reasoning,
                    &pending_call,
                )
                .await?;

                {
                    let mut pending_map = self.pending_calls.lock().await;
                    pending_map.insert(new_pending_id.clone(), new_state);
                }

                Ok(MessageResponse {
                    chat_id: state.chat_id,
                    message_id: state.message_id,
                    content: placeholder_content,
                    reasoning: placeholder_reasoning,
                    model_id: state.model_id,
                    provider_id: state.provider_id,
                    input_tokens: None,
                    output_tokens: None,
                    total_tokens: None,
                    duration: Some(duration_ms),
                    pending_mcp_call: Some(pending_call),
                })
            }
        };

        // 清理执行状态
        {
            let mut executing_map = executing_calls_cleanup.lock().await;
            executing_map.remove(&pending_id_cleanup);
        }

        result
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
                role: match msg.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::System => "system".to_string(),
                },
                content: msg.content.clone(),
                reasoning: msg.reasoning.clone(),
                tool_calls: None,
                tool_call_deltas: None,
                tool_call_id: None,
            })
            .collect();

        let tools = self.prepare_tools(&chat).await?;
            
        Ok(ChatRequest {
            model: request.model_id.clone(),
            messages,
            temperature: chat.temperature,
            max_tokens: chat.max_tokens,
            stream: Some(false),
            tools: if tools.is_empty() { None } else { Some(tools.to_vec()) },
            tool_choice: if tools.is_empty() { None } else { Some(ChatToolChoice::Auto) },
            parallel_tool_calls: if tools.is_empty() { None } else { Some(true) },
        })
    }


    async fn process_tool_calls_with_state(
        &self,
        chat_id: &str,
        provider_id: &str,
        model_id: &str,
        mut api_request: ChatRequest,
        api_response: ChatResponse,
        existing_message_id: Option<String>,
    ) -> Result<ToolCallOutcome, AppError> {
        let tool_calls = Self::extract_tool_calls(&api_response);
        if tool_calls.is_empty() {
            return Ok(ToolCallOutcome::Final(api_response));
        }

        let assistant_message = Self::build_assistant_tool_message(&api_response, &tool_calls);
        api_request.messages.push(assistant_message.clone());

        let pending_id = uuid::Uuid::new_v4().to_string();
        let is_update = existing_message_id.is_some();
        let message_id = existing_message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // 保存包含工具调用的 assistant 消息到数据库
        self.save_assistant_tool_call_message(
            chat_id,
            &message_id,
            &assistant_message,
            provider_id,
            model_id,
            is_update, // 如果有现有的 message_id，则是更新操作
        ).await?;

        let (pending_call, placeholder_content) =
            self.build_pending_call(&pending_id, &assistant_message, &tool_calls).await?;

        let state = PendingToolCallState {
            message_id,
            chat_id: chat_id.to_string(),
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
            api_request,
            tool_calls: tool_calls.clone(),
        };

        let result = PendingCallResult {
            pending_id,
            state,
            placeholder_content,
            placeholder_reasoning: assistant_message.reasoning.clone(),
            pending_call,
        };

        Ok(ToolCallOutcome::Pending(result))
    }

    async fn build_pending_call(
        &self,
        pending_id: &str,
        assistant_message: &ChatMessage,
        tool_calls: &[ChatToolCall],
    ) -> Result<(PendingMcpCall, String), AppError> {
        let mut display_calls = Vec::new();

        // 获取活跃的 MCP 服务器
        let servers = self.mcp_service.list_servers().await
            .map_err(|e| AppError::internal_error(&format!("Failed to get servers: {}", e)))?
            .into_iter()
            .filter(|s| s.enabled)
            .collect::<Vec<_>>();

        for call in tool_calls {
            // 在所有服务器中查找工具
            let mut found = false;
            for server in &servers {
                if let Some(tool) = server.tools.iter().find(|t| {
                    let function_name = format!("{}_{}", server.name, t.name);
                    function_name == call.name
                }) {
                    let display = PendingMcpToolCall {
                        call_id: call.id.clone(),
                        server_id: server.id.clone(),
                        server_name: server.name.clone(),
                        server_display_name: server.display_name.clone(),
                        tool_name: tool.name.clone(),
                        tool_description: tool.description.clone(),
                        arguments: Self::parse_tool_arguments_value(&call.arguments),
                    };

                    display_calls.push(display);
                    found = true;
                    break;
                }
            }

            if !found {
                return Err(AppError::internal_error(&format!(
                    "Tool {} not found in any MCP server",
                    call.name
                )));
            }
        }

        let title = if display_calls.len() == 1 {
            format!("模型请求执行 MCP 工具 {}", display_calls[0].tool_name)
        } else {
            format!("模型请求执行 {} 个 MCP 工具", display_calls.len())
        };

        let description = assistant_message.content.trim().to_string();
        let description = if description.is_empty() {
            None
        } else {
            Some(description)
        };

        let placeholder_content = description.clone().unwrap_or_else(|| title.clone());

        Ok((
            PendingMcpCall {
                id: pending_id.to_string(),
                title,
                description,
                tool_calls: display_calls,
            },
            placeholder_content,
        ))
    }

    fn parse_tool_arguments_value(arguments: &str) -> Value {
        if arguments.trim().is_empty() {
            return Value::Object(Map::new());
        }

        serde_json::from_str(arguments).unwrap_or_else(|_| Value::String(arguments.to_string()))
    }

    async fn store_pending_call_and_build_response(
        &self,
        chat: &crate::models::Chat,
        request: &MessageRequest,
        pending: PendingCallResult,
        duration: f64,
        reuse_existing_message: bool,
    ) -> Result<MessageResponse, AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        let PendingCallResult {
            pending_id,
            state,
            placeholder_content,
            placeholder_reasoning,
            pending_call,
        } = pending;

        let config = MessageConfig {
            temperature: chat.temperature,
            top_p: chat.top_p,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            model_id: Some(request.model_id.clone()),
            provider_id: Some(request.provider_id.clone()),
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
        };

        let tools = crate::models::MessageTools {
            pending_mcp_call: Some(pending_call.clone()),
            tool_call_deltas: None,
        };

        if reuse_existing_message {
            self.repository
                .update_message_content(&state.message_id, &placeholder_content, now)
                .await?;
            self.repository
                .update_message_reasoning(&state.message_id, placeholder_reasoning.as_deref(), now)
                .await?;
            self.repository
                .update_message_config(&state.message_id, Some(&config), now)
                .await?;
            self.repository
                .update_message_tools(&state.message_id, Some(&tools), now)
                .await?;
        } else {
            let message = Message {
                id: state.message_id.clone(),
                chat_id: chat.id.clone(),
                role: MessageRole::Assistant,
                content: placeholder_content.clone(),
                reasoning: placeholder_reasoning.clone(),
                config: Some(config),
                tools: Some(tools),
                attachments: None,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                start_time: Some(now),
                end_time: Some(now),
                duration: None,
                created_at: now,
                updated_at: now,
            };

            self.repository.create_message(&message).await?;
        }

        let message_id = state.message_id.clone();
        let chat_id = chat.id.clone();
        let model_id = request.model_id.clone();
        let provider_id = request.provider_id.clone();

        {
            let mut pending_map = self.pending_calls.lock().await;
            pending_map.insert(pending_id.clone(), state);
        }

        Ok(MessageResponse {
            chat_id,
            message_id,
            content: placeholder_content,
            reasoning: placeholder_reasoning,
            model_id,
            provider_id,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            duration: Some(duration as i64),
            pending_mcp_call: Some(pending_call),
        })
    }

    async fn finalize_pending_message(
        &self,
        state: &PendingToolCallState,
        chat: &crate::models::Chat,
        llm_response: &LlmApiResponse,
        duration_ms: i64,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        self.repository
            .update_message_content(&state.message_id, &llm_response.content, now)
            .await?;

        self.repository
            .update_message_reasoning(&state.message_id, llm_response.reasoning.as_deref(), now)
            .await?;

        self.repository
            .update_message_stats(
                &state.message_id,
                llm_response.usage.as_ref().map(|u| u.prompt_tokens),
                llm_response.usage.as_ref().map(|u| u.completion_tokens),
                llm_response.usage.as_ref().map(|u| u.total_tokens),
                None,
                Some(now),
                Some(if duration_ms > i64::from(i32::MAX) {
                    i32::MAX
                } else {
                    duration_ms as i32
                }),
                now,
            )
            .await?;

        let config = MessageConfig {
            temperature: chat.temperature,
            top_p: chat.top_p,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            model_id: Some(state.model_id.clone()),
            provider_id: Some(state.provider_id.clone()),
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
                    };

        self.repository
            .update_message_config(&state.message_id, Some(&config), now)
            .await?;

        Ok(())
    }

    async fn update_pending_message(
        &self,
        new_state: &PendingToolCallState,
        chat: &crate::models::Chat,
        placeholder_content: &str,
        placeholder_reasoning: &Option<String>,
        pending_call: &PendingMcpCall,
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        self.repository
            .update_message_content(&new_state.message_id, placeholder_content, now)
            .await?;

        self.repository
            .update_message_reasoning(&new_state.message_id, placeholder_reasoning.as_deref(), now)
            .await?;

        let config = MessageConfig {
            temperature: chat.temperature,
            top_p: chat.top_p,
            max_tokens: chat.max_tokens,
            stream: chat.stream,
            model_id: Some(new_state.model_id.clone()),
            provider_id: Some(new_state.provider_id.clone()),
            system_prompt: chat.system_prompt.clone(),
            mcp_servers: Some(chat.mcp_servers.clone()),
        };

        let tools = crate::models::MessageTools {
            pending_mcp_call: Some(pending_call.clone()),
            tool_call_deltas: None,
        };

        self.repository
            .update_message_config(&new_state.message_id, Some(&config), now)
            .await?;
        self.repository
            .update_message_tools(&new_state.message_id, Some(&tools), now)
            .await?;

        Ok(())
    }

    async fn prepare_tools(
        &self,
        chat: &crate::models::Chat,
    ) -> Result<Vec<ChatTool>, AppError> {
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

                tools.push(ChatTool {
                    tool_type: "function".to_string(),
                    function: ChatFunction {
                        name: tool.name.clone(),
                        description,
                        parameters: tool.input_schema.clone(),
                    },
                });
            }
        }

        Ok(tools)
    }

    fn extract_tool_calls(response: &crate::llm_client::types::ChatResponse) -> Vec<ChatToolCall> {
        response
            .choices
            .iter()
            .filter_map(|choice| choice.message.as_ref())
            .filter_map(|message| message.tool_calls.clone())
            .next()
            .unwrap_or_default()
    }

    fn build_assistant_tool_message(
        response: &crate::llm_client::types::ChatResponse,
        tool_calls: &[ChatToolCall],
    ) -> ChatMessage {
        let (content, reasoning) = response
            .choices
            .iter()
            .find_map(|choice| choice.message.as_ref())
            .map(|message| (message.content.clone(), message.reasoning.clone()))
            .unwrap_or_else(|| (String::new(), None));

        ChatMessage {
            role: "assistant".to_string(),
            content,
            reasoning,
            tool_calls: Some(tool_calls.to_vec()),
            tool_call_deltas: None,
            tool_call_id: None,
        }
    }


    /// 直接执行工具调用，不依赖复杂的 MCP 上下文
    async fn execute_tool_call_direct(&self, call: &ChatToolCall) -> String {
        // 获取活跃的 MCP 服务器
        let servers = match self.mcp_service.list_servers().await {
            Ok(servers) => servers.into_iter().filter(|s| s.enabled).collect::<Vec<_>>(),
            Err(e) => {
                tracing::error!("[MessageService::execute_tool_call_direct] Failed to get servers: {}", e);
                return format!("无法获取 MCP 服务器列表: {}", e.message);
            }
        };

        // 在所有服务器中查找工具
        for server in servers {
            if let Some(tool) = server.tools.iter().find(|t| {
                let function_name = format!("{}_{}", server.name, t.name);
                function_name == call.name
            }) {
                let arguments = Self::parse_tool_arguments(&call.arguments);

                match self.invoke_mcp_tool(&server, &tool.name, arguments).await {
                    Ok(result) => return Self::format_mcp_tool_result(&result),
                    Err(error) => {
                        tracing::error!(
                            "[MessageService::execute_tool_call_direct] Tool {} failed: {}",
                            call.name,
                            error
                        );
                        return format!("调用工具 {} 失败: {}", call.name, error.message);
                    }
                }
            }
        }

        // 如果没有找到工具
        tracing::warn!(
            "[MessageService::execute_tool_call_direct] Tool {} not found in any MCP server",
            call.name
        );
        format!("工具 {} 未在任何 MCP 服务器中找到", call.name)
    }

    async fn invoke_mcp_tool(
        &self,
        server: &McpServer,
        tool_name: &str,
        arguments: Option<Value>,
    ) -> Result<CallToolResult, AppError> {
        let client = McpClientFactory::create_client(server).await.map_err(|e| {
            AppError::internal_error(&format!(
                "Failed to connect to MCP server {}: {}",
                server.name, e
            ))
        })?;

        let call_result = client.call_tool(tool_name, arguments).await.map_err(|e| {
            AppError::internal_error(&format!(
                "Failed to call MCP tool {} on {}: {}",
                tool_name, server.name, e
            ))
        });

        if let Err(e) = client.shutdown().await {
            tracing::warn!(
                "[MessageService::invoke_mcp_tool] Failed to shutdown MCP client {}: {}",
                server.name,
                e
            );
        }

        call_result
    }

    fn parse_tool_arguments(arguments: &str) -> Option<Value> {
        if arguments.trim().is_empty() {
            return None;
        }

        match serde_json::from_str::<Value>(arguments) {
            Ok(Value::Object(map)) => Some(Value::Object(map)),
            Ok(other) => {
                let mut wrapper = Map::new();
                wrapper.insert("value".to_string(), other);
                Some(Value::Object(wrapper))
            }
            Err(_) => {
                let mut wrapper = Map::new();
                wrapper.insert(
                    "raw".to_string(),
                    Value::String(arguments.trim().to_string()),
                );
                Some(Value::Object(wrapper))
            }
        }
    }

    fn format_mcp_tool_result(result: &CallToolResult) -> String {
        if let Some(structured) = &result.structured_content {
            return serde_json::to_string_pretty(structured)
                .unwrap_or_else(|_| structured.to_string());
        }

        let mut pieces = Vec::new();
        for content in &result.content {
            match &content.raw {
                rmcp::model::RawContent::Text(text) => pieces.push(text.text.clone()),
                _ => pieces.push(
                    serde_json::to_string(&content).unwrap_or_else(|_| format!("{:?}", content)),
                ),
            }
        }

        if pieces.is_empty() {
            "工具未返回任何内容".to_string()
        } else {
            pieces.join("\n")
        }
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

        // 5. 保存用户输入消息到数据库（如果有chat_id）
        if let Some(chat_id) = &request.chat_id {
            // 从消息列表中找到最后一条用户消息进行保存
            if let Some(last_user_message) = request
                .messages
                .iter()
                .rev()
                .find(|m| matches!(m.role, crate::models::MessageRole::User))
            {
                // 从 chats 表获取配置参数
                let chat = match self.get_chat_config(chat_id).await {
                    Ok(chat) => chat,
                    Err(e) => {
                        tracing::error!(
                            "[MessageService::call_llm_api_stream] Failed to get chat config: {}",
                            e
                        );
                        return Err(e);
                    }
                };

                let config = Self::extract_message_config_from_chat(&request, &chat);
                if let Err(e) = self
                    .save_user_message(chat_id, &last_user_message.content, config)
                    .await
                {
                    tracing::error!(
                        "[MessageService::call_llm_api_stream] Failed to save user message: {}",
                        e
                    );
                }
            }
        }

        // 6. 调用 LLM 流式 API
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
            content: accumulated_content.clone(), // 使用流式累积的内容
            reasoning: if accumulated_reasoning.is_empty() {
                None
            } else {
                Some(accumulated_reasoning.clone())
            },
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: None, // 流式API通常不返回token统计
            output_tokens: None,
            total_tokens: None,
            duration: Some(duration),
            pending_mcp_call: None,
        };

        tracing::info!(
            "[MessageService::call_llm_api_stream] Real streaming API call completed, chunks: {}, total_content_length: {}, duration: {}ms",
            chunk_count, accumulated_content.len(), duration
        );

        // 8. 流式输出完成后，保存完整的AI响应消息到数据库
        if !accumulated_content.is_empty() && request.chat_id.is_some() {
            // 从 chats 表获取配置参数
            let chat = match self
                .get_chat_config(&request.chat_id.clone().unwrap())
                .await
            {
                Ok(chat) => chat,
                Err(e) => {
                    tracing::error!("[MessageService::call_llm_api_stream] Failed to get chat config for saving: {}", e);
                    return Err(e);
                }
            };

            let config = Self::extract_message_config_from_chat(&request, &chat);
            let start_time_millis = chrono::Utc::now().timestamp_millis() - duration;

            if let Err(e) = self
                .save_assistant_message(
                    &request.chat_id.clone().unwrap(),
                    &accumulated_content,
                    if accumulated_reasoning.is_empty() {
                        None
                    } else {
                        Some(accumulated_reasoning)
                    },
                    config,
                    start_time_millis,
                    duration,
                    None, // 流式API通常不返回token统计
                    None,
                    None,
                )
                .await
            {
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
            tools: None,
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
            tools: None,
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

    /// 保存包含工具调用的 assistant 消息
    async fn save_assistant_tool_call_message(
        &self,
        chat_id: &str,
        message_id: &str,
        assistant_message: &crate::llm_client::types::ChatMessage,
        provider_id: &str,
        model_id: &str,
        is_update: bool, // 新增参数：是否是更新现有消息
    ) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp_millis();

        // 创建消息配置
        let config = MessageConfig {
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: None,
            model_id: Some(model_id.to_string()),
            provider_id: Some(provider_id.to_string()),
            system_prompt: None,
            mcp_servers: None,
        };

        // 从 assistant_message 中提取工具调用增量数据
        let tool_call_deltas = assistant_message.tool_call_deltas.clone()
            .or_else(|| {
                // 如果没有 tool_call_deltas，尝试从 tool_calls 转换
                assistant_message.tool_calls.as_ref().map(|calls| {
                    calls.iter().enumerate().map(|(index, call)| {
                        crate::llm_client::types::ChatToolCallDelta {
                            index: index as u32,
                            id: Some(call.id.clone()),
                            tool_type: call.tool_type.clone(),
                            name: Some(call.name.clone()),
                            arguments: Some(call.arguments.clone()),
                        }
                    }).collect()
                })
            });

        // 创建工具数据
        let tools = tool_call_deltas.map(|deltas| crate::models::MessageTools {
            pending_mcp_call: None,
            tool_call_deltas: Some(deltas),
        });

        // 创建消息实体
        let message = Message {
            id: message_id.to_string(),
            chat_id: chat_id.to_string(),
            role: crate::models::MessageRole::Assistant,
            content: assistant_message.content.clone(),
            reasoning: assistant_message.reasoning.clone(),
            config: Some(config),
            tools,
            attachments: None,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: Some(now),
            end_time: Some(now),
            duration: Some(0), // 工具调用消息本身没有执行时间
            created_at: now,
            updated_at: now,
        };

        // 保存到数据库
        if is_update {
            // 更新现有消息
            let message_id_string = message_id.to_string();
            self.repository.update_message_content(&message_id_string, &message.content, now).await?;
            self.repository.update_message_reasoning(&message_id_string, message.reasoning.as_deref(), now).await?;
            if let Some(ref tools) = message.tools {
                self.repository.update_message_tools(&message_id_string, Some(tools), now).await?;
            }
            if let Some(ref config) = message.config {
                self.repository.update_message_config(&message_id_string, Some(config), now).await?;
            }
        } else {
            // 创建新消息
            self.repository.create_message(&message).await?;
        }

        tracing::info!(
            "[MessageService] Assistant tool call message saved: {}",
            message_id
        );

        Ok(())
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

    /// 从 API 响应格式转换
    fn convert_from_api_response(
        &self,
        api_response: crate::llm_client::types::ChatResponse,
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
        // 获取聊天的系统提示词
        let chat = self.chat_service.get_chat(message.chat_id.clone()).await?;

        // 构建消息数组，如果有系统提示词则添加到开头
        let mut request_messages = Vec::new();
        if let Some(system_prompt) = &chat.system_prompt {
            if !system_prompt.trim().is_empty() {
                request_messages.push(crate::models::ChatMessage {
                    role: crate::models::MessageRole::System,
                    content: system_prompt.clone(),
                    reasoning: None,
                });
            }
        }

        // 添加历史消息（排除要重新生成的助手消息）
        request_messages.extend(
            chat_messages
                .iter()
                .filter(|m| m.role != MessageRole::Assistant || m.id != message_id)
                .map(|m| crate::models::ChatMessage {
                    role: m.role.clone(),
                    content: m.content.clone(),
                    reasoning: None, // 历史消息没有推理过程
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
        let llm_result = self
            .call_llm_api(&regenerate_request, Some(message_id.clone()))
            .await?;
        let llm_response = match llm_result {
            LlmCallResult::Final(response) => response,
            LlmCallResult::Pending(response) => {
                // 更新消息为待执行状态
                self.repository
                    .update_message_reasoning(&message_id, response.reasoning.as_deref(), now)
                    .await?;
                self.repository
                    .update_message_content(&message_id, &response.content, now)
                    .await?;
                if let Some(pending) = response.pending_mcp_call.clone() {
                    let config = MessageConfig {
                        temperature: chat.temperature,
                        top_p: chat.top_p,
                        max_tokens: chat.max_tokens,
                        stream: chat.stream,
                        model_id: Some(regenerate_request.model_id.clone()),
                        provider_id: Some(regenerate_request.provider_id.clone()),
                        system_prompt: chat.system_prompt.clone(),
                        mcp_servers: Some(chat.mcp_servers.clone()),
                    };

                    let tools = crate::models::MessageTools {
                        pending_mcp_call: Some(pending.clone()),
                        tool_call_deltas: None,
                    };

                    self.repository
                        .update_message_config(&message_id, Some(&config), now)
                        .await?;
                    self.repository
                        .update_message_tools(&message_id, Some(&tools), now)
                        .await?;
                }

                return Ok(response);
            }
        };
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
            input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
            duration: llm_response.duration.map(|d| d as i64),
            pending_mcp_call: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, MessageConfig, MessageRequest, MessageRole, ModelParameters};
    use crate::services::{ChatService, McpService, ProviderService};
    use crate::storage::Database;
    use std::collections::HashMap;
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

    #[test]
    fn format_mcp_tool_result_prefers_structured_content() {
        let result = CallToolResult::structured(serde_json::json!({"value": 42}));
        let formatted = MessageService::format_mcp_tool_result(&result);
        assert!(formatted.contains("\"value\""));
    }

    #[test]
    fn format_mcp_tool_result_falls_back_to_text() {
        let result = CallToolResult::success(vec![rmcp::model::Content::text("hello world")]);
        let formatted = MessageService::format_mcp_tool_result(&result);
        assert_eq!(formatted, "hello world");
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
                role: MessageRole::User,
                content: "Hello".to_string(),
                reasoning: None,
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
