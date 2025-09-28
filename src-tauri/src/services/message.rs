// 消息服务实现

use crate::llm_client::{create_llm_client};
use crate::llm_client::types::{
    ChatMessage as LlmChatMessage, ChatRequest,
    ChatToolCall, ChatToolChoice, ChatUsage, RequestTool, RequestToolFunction,
    ChatMessageRole, ChatResponse,
};
use crate::mcp_client::McpClientFactory;
use crate::models::{
    AppError, McpServer, McpServerStatus, Message, MessageConfig, MessageRequest,
    MessageResponse, MessageRole, UUID,
};
use crate::services::{ChatService, Database, McpService, ProviderService};
use crate::storage::MessageRepository;
use rmcp::model::CallToolResult;
use serde_json::{Map, Value};
use std::sync::Arc;

/// 流式数据结构
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<ChatToolCall>>,
}

/// LLM API 响应结构
#[derive(Debug)]
struct LlmApiResponse {
    content: String,
    reasoning: Option<String>,
    tool_calls: Option<Vec<ChatToolCall>>,
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
            .save_user_message(chat_id, &last_message.content, config.clone())
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
            tool_calls: None,
        };

        tracing::info!(
            "[MessageService::send_message] Successfully completed for chat_id: {:?}, message_id: {}",
            chat_id, assistant_message_id
        );
        Ok(response)
    }

    /// 调用 LLM API
    async fn call_llm_api(
        &self,
        request: &MessageRequest,
    ) -> Result<LlmApiResponse, AppError> {
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
        let llm_response = self.convert_from_api_response(api_response, duration)?;

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
        let messages: Vec<LlmChatMessage> = request
            .messages
            .iter()
            .map(|msg| LlmChatMessage {
                role: match msg.role {
                    MessageRole::User => ChatMessageRole::User,
                    MessageRole::Assistant => ChatMessageRole::Assistant,
                    MessageRole::System => ChatMessageRole::System,
                },
                content: msg.content.clone(),
                reasoning: msg.reasoning.clone(),
                tool_calls: None,
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
                function_name == call.function.name
            }) {
                let arguments = Self::parse_tool_arguments(&call.function.arguments);

                match self.invoke_mcp_tool(&server, &tool.name, arguments).await {
                    Ok(result) => return Self::format_mcp_tool_result(&result),
                    Err(error) => {
                        tracing::error!(
                            "[MessageService::execute_tool_call_direct] Tool {} failed: {}",
                            call.function.name,
                            error
                        );
                        return format!("调用工具 {} 失败: {}", call.function.name, error.message);
                    }
                }
            }
        }

        // 如果没有找到工具
        tracing::warn!(
            "[MessageService::execute_tool_call_direct] Tool {} not found in any MCP server",
            call.function.name
        );
        format!("工具 {} 未在任何 MCP 服务器中找到", call.function.name)
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
            tool_calls: Some(all_tool_calls.clone()),
            model_id: request.model_id.clone(),
            provider_id: request.provider_id.clone(),
            input_tokens: None, // 流式API通常不返回token统计
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
                    Some(all_tool_calls.clone()),
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
            tool_calls: None,
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
    ) -> Result<String, AppError> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        let message = Message {
            id: message_id.clone(),
            chat_id: chat_id.to_string(),
            role: crate::models::MessageRole::Assistant,
            content: content.to_string(),
            reasoning,
            tool_calls,
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
        api_response: ChatResponse,
        duration: f64,
    ) -> Result<LlmApiResponse, AppError> {
        let choice = api_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AppError::internal_error("No choices in API response"))?;

        let message = choice
            .delta
            .ok_or_else(|| AppError::internal_error("No message in API choice"))?;

        Ok(LlmApiResponse {
            content: message.content,
            reasoning: message.reasoning,
            tool_calls: message.tool_calls,
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
            input_tokens: llm_response.usage.as_ref().map(|u| u.prompt_tokens),
            output_tokens: llm_response.usage.as_ref().map(|u| u.completion_tokens),
            total_tokens: llm_response.usage.as_ref().map(|u| u.total_tokens),
            duration: llm_response.duration.map(|d| d as i64),
            tool_calls: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, MessageConfig, MessageRequest, MessageRole, ModelParameters};
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
