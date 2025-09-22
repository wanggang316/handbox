// OpenAI Completions API 客户端
// 使用 openai-rust SDK 进行通信

use crate::llm_client::chat::ChatClient;
use crate::llm_client::types::{ChatChoice, ChatMessage, ChatRequest, ChatResponse, ChatUsage};
use crate::models::{AppError, Provider};
use async_trait::async_trait;
use futures::StreamExt;

/// OpenAI Completions 风格聊天客户端
pub struct OpenAICompletionsChatClient {}

impl OpenAICompletionsChatClient {
    pub fn new() -> Self {
        Self {}
    }

    /// 转换我们的 ChatRequest 到 openai-rust 的 ChatCompletionRequest
    fn convert_to_openai_request(
        &self,
        request: &ChatRequest,
    ) -> openai_rust::types::ChatCompletionRequest {
        let messages: Vec<openai_rust::types::ChatMessage> = request
            .messages
            .iter()
            .map(|msg| openai_rust::types::ChatMessage {
                role: match msg.role.as_str() {
                    "system" => openai_rust::types::Role::System,
                    "user" => openai_rust::types::Role::User,
                    "assistant" => openai_rust::types::Role::Assistant,
                    _ => openai_rust::types::Role::User, // 默认值
                },
                content: msg.content.clone(),
            })
            .collect();

        openai_rust::types::ChatCompletionRequest {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            stream: request.stream,
        }
    }

    /// 转换 openai-rust 的 ChatCompletionResponse 到我们的 ChatResponse
    fn convert_from_openai_response(
        &self,
        response: openai_rust::types::ChatCompletionResponse,
    ) -> ChatResponse {
        let choices: Vec<ChatChoice> = response
            .choices
            .into_iter()
            .map(|choice| ChatChoice {
                index: choice.index as i32,
                message: Some(ChatMessage {
                    role: match choice.message.role {
                        openai_rust::types::Role::System => "system".to_string(),
                        openai_rust::types::Role::User => "user".to_string(),
                        openai_rust::types::Role::Assistant => "assistant".to_string(),
                    },
                    content: choice.message.content,
                    reasoning: choice.message.reasoning,
                }),
                delta: None,
                finish_reason: Some(choice.finish_reason),
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
    fn convert_from_openai_chunk(
        &self,
        chunk: openai_rust::types::ChatCompletionChunkResponse,
    ) -> ChatResponse {
        let choices: Vec<ChatChoice> = chunk
            .choices
            .into_iter()
            .map(|choice| ChatChoice {
                index: choice.index as i32,
                message: None,
                delta: Some(ChatMessage {
                    role: choice
                        .delta
                        .role
                        .map(|role| match role {
                            openai_rust::types::Role::System => "system".to_string(),
                            openai_rust::types::Role::User => "user".to_string(),
                            openai_rust::types::Role::Assistant => "assistant".to_string(),
                        })
                        .unwrap_or_default(),
                    content: choice.delta.content.unwrap_or_default(),
                    reasoning: choice.delta.reasoning,
                }),
                finish_reason: choice.finish_reason,
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
