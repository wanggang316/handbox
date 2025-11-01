// OpenAI Responses API 客户端
// 使用 openai-rust SDK 进行通信

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChoice, LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmMessage, LlmMessageRole,
    LlmProvider, LlmRequest, LlmResponse, LlmToolChoice, LlmUsage,
};
use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use openai_rust::client::Error as OpenAIError;
use openai_rust::types::{
    CreateResponseRequest, InputItem, InputMessage, InputMessageContent, InputMessageRole, Item,
    ItemFunctionCall, ItemFunctionCallOutput, ItemStatus, OutputItem, Response as OpenAIResponse,
    ResponseInput, ResponseStreamEvent, ResponseUsage, ResponsesTool, ResponsesToolChoice,
};
use tokio::sync::mpsc;
use uuid::Uuid;

/// OpenAI Responses 风格聊天客户端
pub struct OpenAIResponsesChatClient;

impl OpenAIResponsesChatClient {
    pub fn new() -> Self {
        Self
    }

    /// 转换我们的 LlmRequest 到 openai-rust 的 CreateResponseRequest
    fn convert_to_openai_response_request(
        &self,
        request: &LlmRequest,
    ) -> Result<CreateResponseRequest, LlmClientError> {
        tracing::info!(
            "Converting LlmRequest: model={}, messages={}, has_tools={}, has_tool_choice={}, parallel_tool_calls={:?}",
            request.model,
            request.messages.len(),
            request.tools.is_some(),
            request.tool_choice.is_some(),
            request.parallel_tool_calls
        );

        // 转换消息为 InputItem
        // 对于 Tool 角色，需要使用 ItemFunctionCallOutput 格式传递工具调用结果
        let mut input_items: Vec<InputItem> = Vec::new();
        let mut tool_messages_count = 0;

        for msg in &request.messages {
            match msg.role {
                LlmMessageRole::Tool => {
                    // OpenAI Responses API 不支持 'tool' 角色作为普通消息
                    // 工具结果需要通过 ItemFunctionCallOutput 格式传递
                    // 参考: https://platform.openai.com/docs/api-reference/responses/create
                    if let Some(call_id) = &msg.tool_call_id {
                        // 工具调用结果使用 FunctionCallOutput 格式
                        input_items.push(InputItem::Item(Item::FunctionCallOutput(
                            ItemFunctionCallOutput {
                                item_type: "function_call_output".to_string(),
                                call_id: call_id.clone(),
                                output: msg.content.clone(), // 工具执行结果作为 JSON 字符串
                                status: Some(ItemStatus::Completed),
                            },
                        )));
                        tool_messages_count += 1;
                    } else {
                        tracing::warn!(
                            "Tool message without tool_call_id, skipping: {}",
                            msg.content
                        );
                    }
                }
                LlmMessageRole::Assistant => {
                    // Assistant 消息需要特殊处理：如果包含 tool_calls，需要转换为 Item::FunctionCall
                    // 根据 Responses API，需要先有 function_call，然后才能有 function_call_output
                    if let Some(tool_calls) = &msg.tool_calls {
                        // 如果 Assistant 消息包含 tool_calls，需要将每个 tool_call 转换为 Item::FunctionCall
                        for tool_call in tool_calls {
                            input_items.push(InputItem::Item(Item::FunctionCall(
                                ItemFunctionCall {
                                    item_type: "function_call".to_string(),
                                    call_id: tool_call.id.clone(),
                                    name: tool_call.function.name.clone(),
                                    arguments: tool_call.function.arguments.clone(),
                                    status: Some(ItemStatus::Completed),
                                },
                            )));
                        }
                    }

                    // 即使是 Assistant 消息，也作为 InputMessage 传递（如果有内容的话）
                    // 如果没有 tool_calls，则正常传递消息内容
                    if !msg.content.is_empty() || msg.tool_calls.is_none() {
                        input_items.push(InputItem::InputMessage(InputMessage {
                            role: InputMessageRole::Assistant,
                            content: InputMessageContent::Text(msg.content.clone()),
                            message_type: Some("message".to_string()),
                        }));
                    }
                }
                _ => {
                    // 其他角色（System, User）使用 InputMessage 格式
                    let role = match msg.role {
                        LlmMessageRole::System => InputMessageRole::System,
                        LlmMessageRole::User => InputMessageRole::User,
                        LlmMessageRole::Assistant => {
                            // 不应该到达这里，已在上面处理
                            continue;
                        }
                        LlmMessageRole::Tool => {
                            // 不应该到达这里，已在上面处理
                            continue;
                        }
                    };

                    input_items.push(InputItem::InputMessage(InputMessage {
                        role,
                        content: InputMessageContent::Text(msg.content.clone()),
                        message_type: Some("message".to_string()),
                    }));
                }
            }
        }

        if tool_messages_count > 0 {
            tracing::info!(
                "Converted {} Tool messages to FunctionCallOutput format",
                tool_messages_count
            );
        }

        let instructions = request
            .messages
            .iter()
            .find(|msg| msg.role == LlmMessageRole::System)
            .map(|msg| msg.content.clone());

        let mut builder = CreateResponseRequest::builder()
            .model(request.model.clone())
            .input(ResponseInput::Items(input_items));

        if let Some(instructions) = instructions {
            builder = builder.instructions(instructions);
        }

        if let Some(temperature) = request.temperature {
            builder = builder.temperature(temperature);
        }

        if let Some(max_tokens) = request.max_tokens {
            builder = builder.max_output_tokens(max_tokens);
        }

        if let Some(stream) = request.stream {
            builder = builder.stream(stream);
        }

        // 转换工具定义
        if let Some(tools) = &request.tools {
            // 如果工具数组为空，不设置 tools 参数
            if tools.is_empty() {
                tracing::warn!("Tools array is empty, skipping tools parameter");
            } else {
                tracing::info!("Converting {} tools from LlmRequest", tools.len());

                let openai_tools: Vec<ResponsesTool> = tools
                    .iter()
                    .enumerate()
                    .filter_map(|(i, tool)| {
                        // 验证工具名称不为空
                        if tool.function.name.is_empty() {
                            tracing::error!("Tool[{}] has EMPTY name, skipping!", i);
                            return None;
                        }

                        tracing::info!(
                            "Tool[{}]: name='{}' (len={}), desc_len={}, params={}",
                            i,
                            tool.function.name,
                            tool.function.name.len(),
                            tool.function.description.len(),
                            serde_json::to_string(&tool.function.parameters)
                                .unwrap_or_else(|_| "null".to_string())
                        );

                        // 使用扁平化的 Function 格式（字段直接在 enum variant 中）
                        Some(ResponsesTool::Function {
                            name: tool.function.name.clone(),
                            description: Some(tool.function.description.clone()),
                            parameters: Some(tool.function.parameters.clone()),
                            strict: None,
                        })
                    })
                    .collect();

                if openai_tools.is_empty() {
                    tracing::error!("All tools were filtered out (empty names)");
                } else {
                    // 序列化查看最终 JSON 结构
                    if let Ok(json) = serde_json::to_string_pretty(&openai_tools) {
                        tracing::info!("Serialized tools JSON:\n{}", json);
                    }

                    builder = builder.tools(openai_tools);
                }
            }
        }

        // 转换 tool_choice
        if let Some(tool_choice) = &request.tool_choice {
            let openai_tool_choice = match tool_choice {
                LlmToolChoice::Auto => ResponsesToolChoice::String("auto".to_string()),
                LlmToolChoice::None => ResponsesToolChoice::String("none".to_string()),
                LlmToolChoice::Required => ResponsesToolChoice::String("required".to_string()),
            };
            builder = builder.tool_choice(openai_tool_choice);
        }

        // 转换 parallel_tool_calls
        if let Some(parallel) = request.parallel_tool_calls {
            builder = builder.parallel_tool_calls(parallel);
        }

        builder.build().map_err(|e| {
            LlmClientError::validation(format!("Failed to build OpenAI response request: {}", e))
        })
    }

    /// 转换 openai-rust 的 Response 到我们的 LlmResponse
    fn convert_from_openai_response(&self, response: OpenAIResponse) -> LlmResponse {
        let mut choices = Vec::new();

        // 遍历所有 output items，将它们合并到一个 choice 中
        let mut content = String::new();
        let mut reasoning_text = String::new();
        let mut tool_calls = Vec::new();

        for item in &response.output {
            match item {
                OutputItem::Message {
                    content: msg_content,
                    ..
                } => {
                    // 提取消息内容
                    for output_content in msg_content {
                        if output_content.content_type == "output_text" {
                            if let Some(text) = &output_content.text {
                                content.push_str(text);
                            }
                        }
                    }
                }
                OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                    ..
                } => {
                    // 转换 function_call 为 tool_call
                    tool_calls.push(crate::types::LlmToolCall {
                        id: call_id.clone(),
                        tool_type: "function".to_string(),
                        function: crate::types::LlmToolFunction {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    });
                }
                OutputItem::Reasoning {
                    content: reasoning_content,
                    ..
                } => {
                    // 提取推理内容
                    for part in reasoning_content {
                        reasoning_text.push_str(&part.text);
                        reasoning_text.push('\n');
                    }
                }
                _ => {
                    // 其他类型暂不处理
                }
            }
        }

        // 如果有任何内容，创建一个 choice
        if !content.is_empty() || !tool_calls.is_empty() || !reasoning_text.is_empty() {
            let choice = LlmChoice {
                index: 0,
                delta: Some(LlmMessage {
                    role: LlmMessageRole::Assistant,
                    content,
                    reasoning: if reasoning_text.is_empty() {
                        None
                    } else {
                        Some(reasoning_text.trim().to_string())
                    },
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                }),
                finish_reason: if response.status == "completed" {
                    Some("stop".to_string())
                } else {
                    None
                },
            };
            choices.push(choice);
        }

        let usage = response.usage.clone().map(map_usage);

        LlmResponse {
            id: response.id,
            object: "chat.completion".to_string(),
            model: response.model,
            choices,
            usage,
        }
    }

    fn handle_stream_event(
        &self,
        event: ResponseStreamEvent,
        state: &mut StreamState,
    ) -> Option<Result<LlmChunkResponse, LlmClientError>> {
        match event {
            ResponseStreamEvent::ResponseCreated { response, .. }
            | ResponseStreamEvent::ResponseInProgress { response, .. } => {
                state.update_from_response(&response);
                None
            }
            ResponseStreamEvent::OutputTextDelta {
                delta,
                output_index,
                ..
            } => Some(Ok(state.delta_chunk(output_index as i32, delta))),
            ResponseStreamEvent::OutputItemDone {
                item, output_index, ..
            } => {
                // 处理完成的输出项
                match item {
                    OutputItem::FunctionCall {
                        call_id,
                        name,
                        arguments,
                        ..
                    } => {
                        // 发送工具调用事件
                        Some(Ok(state.tool_call_chunk(
                            output_index,
                            call_id,
                            name,
                            arguments,
                        )))
                    }
                    OutputItem::Reasoning { content, .. } => {
                        // 提取推理内容
                        let mut reasoning_text = String::new();
                        for part in content {
                            reasoning_text.push_str(&part.text);
                            reasoning_text.push('\n');
                        }
                        if !reasoning_text.is_empty() {
                            Some(Ok(state.reasoning_chunk(
                                output_index as i32,
                                reasoning_text.trim().to_string(),
                            )))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            ResponseStreamEvent::ResponseCompleted { response, .. } => {
                state.update_from_response(&response);
                let usage = response.usage.as_ref().map(map_usage_ref);
                Some(Ok(state.finish_chunk(usage)))
            }
            ResponseStreamEvent::Error { error } => Some(Err(LlmClientError::api(format!(
                "OpenAI Responses stream error: {} ({})",
                error.message, error.code
            )))),
            ResponseStreamEvent::OutputItemAdded { .. }
            | ResponseStreamEvent::ContentPartAdded { .. }
            | ResponseStreamEvent::OutputTextDone { .. }
            | ResponseStreamEvent::ContentPartDone { .. }
            | ResponseStreamEvent::Unknown => None,
        }
    }
}

#[async_trait]
impl ChatClient for OpenAIResponsesChatClient {
    async fn chat(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError> {
        tracing::info!("Sending OpenAI-style response request using openai-rust library");

        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                LlmClientError::client_initialization(format!(
                    "Failed to create OpenAI client: {e}"
                ))
            })?;

        let openai_request = self.convert_to_openai_response_request(&request)?;

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        let openai_response = openai_client
            .responses()
            .create(&openai_request)
            .await
            .map_err(|e| map_openai_error("OpenAI Responses API call failed", e))?;

        Ok(self.convert_from_openai_response(openai_response))
    }

    async fn chat_stream(
        &self,
        provider: &LlmProvider,
        mut request: LlmRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    > {
        request.stream = Some(true);

        tracing::info!("Sending OpenAI-style streaming response request using openai-rust library");

        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                LlmClientError::client_initialization(format!(
                    "Failed to create OpenAI client: {e}"
                ))
            })?;

        let openai_request = self.convert_to_openai_response_request(&request)?;

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        let (tx, mut rx) = mpsc::channel::<Result<LlmChunkResponse, LlmClientError>>(100);
        let model_name = request.model.clone();
        let handler = OpenAIResponsesChatClient::new();

        tokio::spawn(async move {
            let mut state = StreamState::new(model_name);
            let responses = openai_client.responses();
            let stream_result = responses.create_stream(&openai_request).await;

            match stream_result {
                Ok(openai_stream) => {
                    let mut openai_stream = Box::pin(openai_stream);
                    while let Some(chunk_result) = openai_stream.next().await {
                        match chunk_result {
                            Ok(chunk) => {
                                if let Some(result) =
                                    handler.handle_stream_event(chunk.event, &mut state)
                                {
                                    match result {
                                        Ok(chat_response) => {
                                            if tx.send(Ok(chat_response)).await.is_err() {
                                                break;
                                            }
                                        }
                                        Err(err) => {
                                            let _ = tx.send(Err(err)).await;
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                let _ = tx
                                    .send(Err(map_openai_error(
                                        "OpenAI Responses streaming API call failed",
                                        err,
                                    )))
                                    .await;
                                break;
                            }
                        }
                    }
                }
                Err(err) => {
                    let _ = tx
                        .send(Err(map_openai_error(
                            "OpenAI Responses streaming API call failed",
                            err,
                        )))
                        .await;
                }
            }
        });

        let response_stream = stream! {
            while let Some(result) = rx.recv().await {
                yield result;
            }
        };

        Ok(Box::new(Box::pin(response_stream))
            as Box<
                dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin,
            >)
    }

    fn api_type(&self) -> &'static str {
        "openai-responses"
    }
}

impl Default for OpenAIResponsesChatClient {
    fn default() -> Self {
        Self::new()
    }
}

struct StreamState {
    response_id: String,
    model: String,
    /// 跟踪工具调用：索引 -> (call_id, name, arguments)
    tool_calls: std::collections::HashMap<u32, (String, String, String)>,
}

impl StreamState {
    fn new(model: String) -> Self {
        Self {
            response_id: format!("response-{}", Uuid::new_v4()),
            model,
            tool_calls: std::collections::HashMap::new(),
        }
    }

    fn update_from_response(&mut self, response: &OpenAIResponse) {
        if !response.id.is_empty() {
            self.response_id = response.id.clone();
        }
        if !response.model.is_empty() {
            self.model = response.model.clone();
        }
    }

    fn delta_chunk(&self, index: i32, content: String) -> LlmChunkResponse {
        LlmChunkResponse {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: self.model.clone(),
            choices: vec![LlmChunkChoice {
                index,
                delta: Some(LlmDeltaMessage {
                    role: Some(LlmMessageRole::Assistant),
                    content: Some(content),
                    reasoning: None,
                    tool_calls: None,
                }),
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn tool_call_chunk(
        &mut self,
        index: u32,
        call_id: String,
        name: String,
        arguments: String,
    ) -> LlmChunkResponse {
        // 更新或插入工具调用状态
        self.tool_calls
            .entry(index)
            .and_modify(|(id, n, args)| {
                *id = call_id.clone();
                *n = name.clone();
                *args = arguments.clone();
            })
            .or_insert((call_id.clone(), name.clone(), arguments.clone()));

        LlmChunkResponse {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: self.model.clone(),
            choices: vec![LlmChunkChoice {
                index: 0,
                delta: Some(LlmDeltaMessage {
                    role: Some(LlmMessageRole::Assistant),
                    content: None,
                    reasoning: None,
                    tool_calls: Some(vec![crate::types::LlmDeltaToolCall {
                        index,
                        id: Some(call_id),
                        tool_type: Some("function".to_string()),
                        function: Some(crate::types::LlmDeltaToolFunction {
                            name: Some(name),
                            arguments: Some(arguments),
                        }),
                    }]),
                }),
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn reasoning_chunk(&self, index: i32, reasoning: String) -> LlmChunkResponse {
        LlmChunkResponse {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: self.model.clone(),
            choices: vec![LlmChunkChoice {
                index,
                delta: Some(LlmDeltaMessage {
                    role: Some(LlmMessageRole::Assistant),
                    content: None,
                    reasoning: Some(reasoning),
                    tool_calls: None,
                }),
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn finish_chunk(&self, usage: Option<LlmUsage>) -> LlmChunkResponse {
        LlmChunkResponse {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: self.model.clone(),
            choices: vec![LlmChunkChoice {
                index: 0,
                delta: Some(LlmDeltaMessage {
                    role: Some(LlmMessageRole::Assistant),
                    content: Some(String::new()),
                    reasoning: None,
                    tool_calls: None,
                }),
                finish_reason: Some("stop".to_string()),
            }],
            usage,
        }
    }
}

fn map_usage(usage: ResponseUsage) -> LlmUsage {
    LlmUsage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
    }
}

fn map_usage_ref(usage: &ResponseUsage) -> LlmUsage {
    map_usage(usage.clone())
}

fn map_openai_error(context: &str, error: OpenAIError) -> LlmClientError {
    match error {
        OpenAIError::ApiError(body) => {
            let message = provider_error_message(&body).unwrap_or(body);
            LlmClientError::api(format!("{context}: {message}"))
        }
        OpenAIError::Reqwest(err) => LlmClientError::transport(format!("{context}: {err}")),
        OpenAIError::JsonParser(err) => LlmClientError::unexpected(format!("{context}: {err}")),
    }
}

fn provider_error_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value
        .get("error")
        .and_then(|err| err.get("message"))
        .and_then(|msg| msg.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_client_api_type() {
        let client = OpenAIResponsesChatClient::new();
        assert_eq!(client.api_type(), "openai-responses");
    }
}
