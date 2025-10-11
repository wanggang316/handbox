// OpenAI Completions API 客户端
// 使用 openai-rust SDK 进行通信

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChoice, LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmDeltaToolCall,
    LlmDeltaToolFunction, LlmMessage, LlmMessageRole, LlmProvider, LlmRequest, LlmResponse,
    LlmToolCall, LlmToolChoice, LlmToolFunction, LlmUsage,
};
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

    /// 转换我们的 LlmRequest 到 openai-rust 的 ChatCompletionRequest
    fn convert_to_openai_request(&self, request: &LlmRequest) -> CompletionRequest {
        let messages: Vec<RequestMessage> = request
            .messages
            .iter()
            .map(|msg| {
                let mut message = RequestMessage::new(map_role(&msg.role), msg.content.clone());

                if let Some(tool_calls) = &msg.tool_calls {
                    message.tool_calls =
                        Some(tool_calls.iter().filter_map(convert_tool_call).collect());
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
            LlmToolChoice::Auto => ToolChoice::None("auto".to_string()),
            LlmToolChoice::None => ToolChoice::None("none".to_string()),
            LlmToolChoice::Required => ToolChoice::Required("required".to_string()),
        });

        CompletionRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            stream: request.stream,
            tools,
            tool_choice,
            parallel_tool_calls: request.parallel_tool_calls,
            ..Default::default()
        }
    }

    /// 转换 openai-rust 的 CompletionResponse 到我们的 LlmResponse
    fn convert_from_openai_response(&self, response: CompletionResponse) -> LlmResponse {
        let choices: Vec<LlmChoice> = response
            .choices
            .into_iter()
            .map(|choice| {
                let message = choice.message;
                LlmChoice {
                    index: choice.index as i32,
                    delta: Some(LlmMessage {
                        role: map_role_to_chat_message_role(message.role),
                        content: message.content.unwrap_or_default(),
                        reasoning: message.reasoning,
                        tool_calls: message
                            .tool_calls
                            .map(|calls| calls.into_iter().map(convert_openai_tool_call).collect()),
                        tool_call_id: None,
                    }),
                    finish_reason: Some(choice.finish_reason),
                }
            })
            .collect();

        let usage = Some(LlmUsage {
            prompt_tokens: response.usage.prompt_tokens as i32,
            completion_tokens: response.usage.completion_tokens as i32,
            total_tokens: response.usage.total_tokens as i32,
        });

        LlmResponse {
            id: response.id,
            object: response.object,
            model: response.model,
            choices,
            usage,
        }
    }

    /// 转换 openai-rust 的 ChatCompletionChunkResponse 到我们的 LlmChunkResponse
    fn convert_from_openai_chunk(&self, chunk: CompletionChunkResponse) -> LlmChunkResponse {
        let choices: Vec<LlmChunkChoice> = chunk
            .choices
            .into_iter()
            .map(|choice| {
                let delta_role = choice.delta.role.map(map_role_to_chat_message_role);
                let tool_calls = choice.delta.tool_calls.clone().map(|calls| {
                    calls
                        .into_iter()
                        .map(convert_openai_delta_tool_call)
                        .collect::<Vec<LlmDeltaToolCall>>()
                });

                LlmChunkChoice {
                    index: choice.index as i32,
                    delta: Some(LlmDeltaMessage {
                        role: delta_role,
                        content: choice.delta.content.clone(),
                        reasoning: choice.delta.reasoning.clone(),
                        tool_calls,
                    }),
                    finish_reason: choice.finish_reason,
                }
            })
            .collect();

        LlmChunkResponse {
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
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError> {
        tracing::info!("Sending OpenAI-style chat request using openai-rust library");

        // 创建 openai-rust 客户端
        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                LlmClientError::client_initialization(format!(
                    "Failed to create OpenAI client: {e}"
                ))
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
            .map_err(|e| LlmClientError::api(format!("OpenAI API call failed: {e}")))?;

        // 转换响应格式
        let chat_response = self.convert_from_openai_response(openai_response);

        Ok(chat_response)
    }

    async fn chat_stream(
        &self,
        provider: &LlmProvider,
        mut request: LlmRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
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
                LlmClientError::client_initialization(format!(
                    "Failed to create OpenAI client: {e}"
                ))
            })?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel::<Result<LlmChunkResponse, LlmClientError>>(100);

        // 在后台任务中处理流，将 openai_client 和 openai_request 的所有权转移进去
        tokio::spawn(async move {
            let completions = openai_client.completions();
            let openai_stream = match completions.create_stream(&openai_request).await {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = tx
                        .send(Err(LlmClientError::api(format!(
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
                    .map_err(|e| LlmClientError::api(format!("Stream error: {e}")));

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
                dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin,
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

fn map_role(role: &LlmMessageRole) -> Role {
    match role {
        LlmMessageRole::System => Role::System,
        LlmMessageRole::Assistant => Role::Assistant,
        LlmMessageRole::Tool => Role::Tool,
        LlmMessageRole::User => Role::User,
    }
}

fn map_role_to_chat_message_role(role: Role) -> LlmMessageRole {
    match role {
        Role::System => LlmMessageRole::System,
        Role::Assistant => LlmMessageRole::Assistant,
        Role::Tool => LlmMessageRole::Tool,
        Role::User => LlmMessageRole::User,
    }
}

fn convert_tool_call(call: &LlmToolCall) -> Option<OpenAIToolCall> {
    Some(OpenAIToolCall {
        id: call.id.clone(),
        tool_type: call.tool_type.clone(),
        function: FunctionCall {
            name: call.function.name.clone(),
            arguments: call.function.arguments.clone(),
        },
    })
}

fn convert_openai_tool_call(call: OpenAIToolCall) -> LlmToolCall {
    LlmToolCall {
        id: call.id,
        tool_type: call.tool_type.clone(),
        function: LlmToolFunction {
            name: call.function.name.clone(),
            arguments: call.function.arguments.clone(),
        },
        execution_mode: Default::default(),
        execution_status: Default::default(),
        result: None,
    }
}

fn convert_openai_delta_tool_call(call: DeltaToolCall) -> LlmDeltaToolCall {
    LlmDeltaToolCall {
        index: call.index,
        id: call.id,
        tool_type: call.tool_type.clone(),
        function: call.function.as_ref().map(|f| LlmDeltaToolFunction {
            name: f.name.clone(),
            arguments: f.arguments.clone(),
        }),
    }
}
