// OpenAI Completions API 客户端
// 使用 openai-rust SDK 进行通信

use crate::llm_client::chat::ChatClient;
use crate::llm_client::types::{
    ChatChoice, ChatMessage, ChatRequest, ChatResponse, ChatToolCall, ChatToolCallDelta,
    ChatToolChoice, ChatUsage,
};
use crate::models::{AppError, Provider};
use async_trait::async_trait;
use futures::StreamExt;
use openai_rust::types::{
    CompletionChunkResponse, CompletionRequest, CompletionResponse, DeltaToolCall, Function,
    FunctionCall, RequestMessage, Role, Tool, ToolCall as OpenAIToolCall, ToolChoice,
};

/// OpenAI Completions 风格聊天客户端
pub struct OpenAICompletionsChatClient {}

impl OpenAICompletionsChatClient {
    pub fn new() -> Self {
        Self {}
    }

    /// 转换我们的 ChatRequest 到 openai-rust 的 ChatCompletionRequest
    fn convert_to_openai_request(&self, request: &ChatRequest) -> CompletionRequest {
        let messages: Vec<RequestMessage> = request
            .messages
            .iter()
            .map(|msg| {
                let mut message = RequestMessage::new(map_role(&msg.role), msg.content.clone());

                if let Some(tool_calls) = &msg.tool_calls {
                    message.tool_calls = Some(tool_calls.iter().map(convert_tool_call).collect());
                }

                if let Some(tool_call_id) = &msg.tool_call_id {
                    message.tool_call_id = Some(tool_call_id.clone());
                }

                message
            })
            .collect();

        let tools = request.tools.as_ref().map(|tools| {
            tools
                .iter()
                .map(|tool| Tool {
                    tool_type: tool.tool_type.clone(),
                    function: Function {
                        name: tool.function.name.clone(),
                        description: tool.function.description.clone(),
                        parameters: tool.function.parameters.clone(),
                    },
                })
                .collect()
        });

        let tool_choice = request.tool_choice.as_ref().map(|choice| match choice {
            ChatToolChoice::Auto => ToolChoice::None("auto".to_string()),
            ChatToolChoice::None => ToolChoice::None("none".to_string()),
            ChatToolChoice::Required => ToolChoice::Required("required".to_string()),
        });

        CompletionRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            stream: request.stream,
            tools,
            tool_choice,
            parallel_tool_calls: request.parallel_tool_calls,
        }
    }

    /// 转换 openai-rust 的 ChatCompletionResponse 到我们的 ChatResponse
    fn convert_from_openai_response(&self, response: CompletionResponse) -> ChatResponse {
        let choices: Vec<ChatChoice> = response
            .choices
            .into_iter()
            .map(|choice| {
                let message = choice.message;
                ChatChoice {
                    index: choice.index as i32,
                    message: Some(ChatMessage {
                        role: map_role_to_string(message.role),
                        content: message.content.unwrap_or_default(),
                        reasoning: message.reasoning,
                        tool_calls: message
                            .tool_calls
                            .map(|calls| calls.into_iter().map(convert_openai_tool_call).collect()),
                        tool_call_deltas: None,
                        tool_call_id: None,
                    }),
                    delta: None,
                    tool_calls_delta: None,
                    finish_reason: Some(choice.finish_reason),
                }
            })
            .collect();

        let usage = Some(ChatUsage {
            prompt_tokens: response.usage.prompt_tokens as i32,
            completion_tokens: response.usage.completion_tokens as i32,
            total_tokens: response.usage.total_tokens as i32,
        });

        ChatResponse {
            id: response.id,
            object: response.object,
            model: response.model,
            choices,
            usage,
        }
    }

    /// 转换 openai-rust 的 ChatCompletionChunkResponse 到我们的 ChatResponse
    fn convert_from_openai_chunk(&self, chunk: CompletionChunkResponse) -> ChatResponse {
        let choices: Vec<ChatChoice> = chunk
            .choices
            .into_iter()
            .map(|choice| {
                let delta_role = choice.delta.role.map(map_role_to_string);
                let tool_call_deltas = choice.delta.tool_calls.clone().map(|calls| {
                    calls
                        .into_iter()
                        .map(convert_openai_delta_tool_call)
                        .collect()
                });

                ChatChoice {
                    index: choice.index as i32,
                    message: None,
                    delta: Some(ChatMessage {
                        role: delta_role.unwrap_or_default(),
                        content: choice.delta.content.unwrap_or_default(),
                        reasoning: choice.delta.reasoning,
                        tool_calls: None,
                        tool_call_deltas: tool_call_deltas.clone(),
                        tool_call_id: None,
                    }),
                    tool_calls_delta: tool_call_deltas,
                    finish_reason: choice.finish_reason,
                }
            })
            .collect();

        ChatResponse {
            id: chunk.id,
            object: chunk.object,
            model: chunk.model,
            choices,
            usage: None,
        }
    }
}

#[async_trait]
impl ChatClient for OpenAICompletionsChatClient {
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        tracing::info!("Sending OpenAI-style chat request using openai-rust library");

        // 创建 openai-rust 客户端
        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {e}"))
            })?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 调用 openai-rust 库
        let openai_response = openai_client
            .completions()
            .create(&openai_request)
            .await
            .map_err(|e| AppError::internal_error(&format!("OpenAI API call failed: {e}")))?;

        // 转换响应格式
        let chat_response = self.convert_from_openai_response(openai_response);

        Ok(chat_response)
    }

    async fn chat_stream(
        &self,
        provider: &Provider,
        mut request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        // 启用流式响应
        request.stream = Some(true);

        tracing::info!("Sending OpenAI-style streaming chat request using openai-rust library");

        // 创建 openai-rust 客户端
        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {e}"))
            })?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel::<Result<ChatResponse, AppError>>(100);

        // 在后台任务中处理流，将 openai_client 和 openai_request 的所有权转移进去
        tokio::spawn(async move {
            let completions = openai_client.completions();
            let openai_stream = match completions.create_stream(&openai_request).await {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = tx
                        .send(Err(AppError::internal_error(&format!(
                            "OpenAI streaming API call failed: {e}"
                        ))))
                        .await;
                    return;
                }
            };

            let mut openai_stream = Box::pin(openai_stream);
            while let Some(result) = openai_stream.next().await {
                let converted_result = result
                    .map(|chunk| {
                        tracing::debug!("[OpenAI Stream] Received chunk: {:?}", chunk);
                        // 创建一个新的 OpenAICompletionsChatClient 实例来转换 chunk
                        let converter = OpenAICompletionsChatClient::new();
                        converter.convert_from_openai_chunk(chunk)
                    })
                    .map_err(|e| AppError::internal_error(&format!("Stream error: {e}")));

                if tx.send(converted_result).await.is_err() {
                    // 接收端已关闭，退出
                    break;
                }
            }
        });

        // 将接收端转换为流
        let converted_stream = async_stream::stream! {
            while let Some(result) = rx.recv().await {
                yield result;
            }
        };

        Ok(Box::new(Box::pin(converted_stream))
            as Box<
                dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin,
            >)
    }

    fn api_type(&self) -> &'static str {
        "openai-completions"
    }
}

impl Default for OpenAICompletionsChatClient {
    fn default() -> Self {
        Self::new()
    }
}

fn map_role(role: &str) -> Role {
    match role {
        "system" => Role::System,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => Role::User,
    }
}

fn map_role_to_string(role: Role) -> String {
    match role {
        Role::System => "system".to_string(),
        Role::Assistant => "assistant".to_string(),
        Role::Tool => "tool".to_string(),
        Role::User => "user".to_string(),
    }
}

fn convert_tool_call(call: &ChatToolCall) -> OpenAIToolCall {
    OpenAIToolCall {
        id: call.id.clone(),
        tool_type: call
            .tool_type
            .clone()
            .unwrap_or_else(|| "function".to_string()),
        function: FunctionCall {
            name: call.name.clone(),
            arguments: call.arguments.clone(),
        },
    }
}

fn convert_openai_tool_call(call: OpenAIToolCall) -> ChatToolCall {
    ChatToolCall {
        id: call.id,
        tool_type: Some(call.tool_type),
        name: call.function.name,
        arguments: call.function.arguments,
    }
}

fn convert_openai_delta_tool_call(call: DeltaToolCall) -> ChatToolCallDelta {
    ChatToolCallDelta {
        index: call.index,
        id: call.id,
        tool_type: call.tool_type,
        name: call.function.as_ref().and_then(|f| f.name.clone()),
        arguments: call.function.and_then(|f| f.arguments),
    }
}
