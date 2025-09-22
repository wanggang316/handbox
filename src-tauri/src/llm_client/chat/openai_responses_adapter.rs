// OpenAI Responses API 客户端
// 使用 openai-rust SDK 进行通信

use crate::llm_client::chat::ChatClient;
use crate::llm_client::types::{ChatChoice, ChatMessage, ChatRequest, ChatResponse, ChatUsage};
use crate::models::{AppError, Provider};
use async_stream::stream;
use async_trait::async_trait;
use futures::StreamExt;
use openai_rust::client::Error as OpenAIError;
use openai_rust::types::{
    CreateResponseRequest, InputItem, MessageContent, Response as OpenAIResponse, ResponseInput,
    ResponseStreamEvent, ResponseUsage,
};
use tokio::sync::mpsc;
use uuid::Uuid;

/// OpenAI Responses 风格聊天客户端
pub struct OpenAIResponsesChatClient;

impl OpenAIResponsesChatClient {
    pub fn new() -> Self {
        Self
    }

    /// 转换我们的 ChatRequest 到 openai-rust 的 CreateResponseRequest
    fn convert_to_openai_response_request(&self, request: &ChatRequest) -> CreateResponseRequest {
        let input_items: Vec<InputItem> = request
            .messages
            .iter()
            .map(|msg| InputItem::Message {
                role: msg.role.clone(),
                content: MessageContent::Text(msg.content.clone()),
            })
            .collect();

        let instructions = request
            .messages
            .iter()
            .find(|msg| msg.role == "system")
            .map(|msg| msg.content.clone());

        CreateResponseRequest {
            model: request.model.clone(),
            input: ResponseInput::Items(input_items),
            instructions,
            metadata: None,
            previous_response_id: None,
            tools: None,
            stream: request.stream,
        }
    }

    /// 转换 openai-rust 的 Response 到我们的 ChatResponse
    fn convert_from_openai_response(&self, response: OpenAIResponse) -> ChatResponse {
        let mut choices = Vec::new();

        for (index, item) in response.output.iter().enumerate() {
            let mut content = String::new();
            for output_content in &item.content {
                if output_content.content_type == "output_text" {
                    if let Some(text) = &output_content.text {
                        content.push_str(text);
                    }
                }
            }

            let choice = ChatChoice {
                index: index as i32,
                message: Some(ChatMessage {
                    role: item.role.clone().unwrap_or_else(|| "assistant".to_string()),
                    content,
                    reasoning: None,
                }),
                delta: None,
                finish_reason: if response.status == "completed" {
                    Some("stop".to_string())
                } else {
                    None
                },
            };
            choices.push(choice);
        }

        let usage = response.usage.clone().map(map_usage);

        ChatResponse {
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
    ) -> Option<Result<ChatResponse, AppError>> {
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
            ResponseStreamEvent::ResponseCompleted { response, .. } => {
                state.update_from_response(&response);
                let usage = response.usage.as_ref().map(map_usage_ref);
                Some(Ok(state.finish_chunk(usage)))
            }
            ResponseStreamEvent::Error { error } => Some(Err(AppError::internal_error(&format!(
                "OpenAI Responses stream error: {} ({})",
                error.message, error.code
            )))),
            ResponseStreamEvent::OutputItemAdded { .. }
            | ResponseStreamEvent::ContentPartAdded { .. }
            | ResponseStreamEvent::OutputTextDone { .. }
            | ResponseStreamEvent::ContentPartDone { .. }
            | ResponseStreamEvent::OutputItemDone { .. }
            | ResponseStreamEvent::Unknown => None,
        }
    }
}

#[async_trait]
impl ChatClient for OpenAIResponsesChatClient {
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        tracing::info!("Sending OpenAI-style response request using openai-rust library");

        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {e}"))
            })?;

        let openai_request = self.convert_to_openai_response_request(&request);

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
        provider: &Provider,
        mut request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        request.stream = Some(true);

        tracing::info!("Sending OpenAI-style streaming response request using openai-rust library");

        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {e}"))
            })?;

        let openai_request = self.convert_to_openai_response_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        let (tx, mut rx) = mpsc::channel::<Result<ChatResponse, AppError>>(100);
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
                dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin,
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
}

impl StreamState {
    fn new(model: String) -> Self {
        Self {
            response_id: format!("response-{}", Uuid::new_v4()),
            model,
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

    fn delta_chunk(&self, index: i32, content: String) -> ChatResponse {
        ChatResponse {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: self.model.clone(),
            choices: vec![ChatChoice {
                index,
                message: None,
                delta: Some(ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    reasoning: None,
                }),
                finish_reason: None,
            }],
            usage: None,
        }
    }

    fn finish_chunk(&self, usage: Option<ChatUsage>) -> ChatResponse {
        ChatResponse {
            id: self.response_id.clone(),
            object: "chat.completion.chunk".to_string(),
            model: self.model.clone(),
            choices: vec![ChatChoice {
                index: 0,
                message: None,
                delta: Some(ChatMessage {
                    role: "assistant".to_string(),
                    content: String::new(),
                    reasoning: None,
                }),
                finish_reason: Some("stop".to_string()),
            }],
            usage,
        }
    }
}

fn map_usage(usage: ResponseUsage) -> ChatUsage {
    ChatUsage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        total_tokens: usage.total_tokens,
    }
}

fn map_usage_ref(usage: &ResponseUsage) -> ChatUsage {
    map_usage(usage.clone())
}

fn map_openai_error(context: &str, error: OpenAIError) -> AppError {
    match error {
        OpenAIError::ApiError(body) => {
            let message = provider_error_message(&body).unwrap_or(body);
            AppError::internal_error(&format!("{context}: {message}"))
        }
        OpenAIError::Reqwest(err) => AppError::internal_error(&format!("{context}: {err}")),
        OpenAIError::JsonParser(err) => AppError::internal_error(&format!("{context}: {err}")),
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
