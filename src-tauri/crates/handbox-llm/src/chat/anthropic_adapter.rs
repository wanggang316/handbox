// Anthropic Claude API 客户端
// 使用 reqwest 进行 HTTP 通信

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChoice, LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmMessage, LlmMessageRole,
    LlmProvider, LlmRequest, LlmResponse, LlmUsage,
};
use async_trait::async_trait;
use futures::TryStreamExt;
use serde_json::Value;

/// Anthropic 风格聊天客户端
pub struct AnthropicChatClient {
    client: reqwest::Client,
}

impl AnthropicChatClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 将通用请求转换为 Anthropic API 格式
    fn convert_to_anthropic_request(&self, request: &LlmRequest) -> Value {
        // Anthropic API 需要将系统消息分离
        let mut system_message = String::new();
        let mut user_messages = Vec::new();

        for msg in &request.messages {
            match msg.role.as_str() {
                "system" => {
                    system_message = msg.content.clone();
                }
                "user" | "assistant" => {
                    user_messages.push(serde_json::json!({
                        "role": msg.role,
                        "content": msg.content
                    }));
                }
                _ => {}
            }
        }

        let mut req_body = serde_json::json!({
            "model": request.model,
            "messages": user_messages,
        });

        if !system_message.is_empty() {
            req_body["system"] = system_message.into();
        }

        if let Some(temperature) = request.temperature {
            req_body["temperature"] = temperature.into();
        }

        if let Some(max_tokens) = request.max_tokens {
            req_body["max_tokens"] = max_tokens.into();
        }

        req_body
    }

    /// 将 Anthropic 响应转换为通用格式
    fn convert_anthropic_response(
        &self,
        anthropic_response: Value,
        model: &str,
    ) -> Result<LlmResponse, LlmClientError> {
        let content = anthropic_response["content"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                LlmClientError::unexpected("Invalid Anthropic API response format".to_string())
            })?
            .to_string();

        let finish_reason = anthropic_response["stop_reason"]
            .as_str()
            .map(|reason| match reason {
                "end_turn" => "stop",
                "max_tokens" => "length",
                _ => "other",
            })
            .map(String::from);

        let usage = anthropic_response["usage"]
            .as_object()
            .map(|usage_obj| LlmUsage {
                prompt_tokens: usage_obj["input_tokens"].as_i64().unwrap_or(0) as i32,
                completion_tokens: usage_obj["output_tokens"].as_i64().unwrap_or(0) as i32,
                total_tokens: (usage_obj["input_tokens"].as_i64().unwrap_or(0)
                    + usage_obj["output_tokens"].as_i64().unwrap_or(0))
                    as i32,
            });

        Ok(LlmResponse {
            id: anthropic_response["id"].as_str().unwrap_or("").to_string(),
            object: "chat.completion".to_string(),
            model: model.to_string(),
            choices: vec![LlmChoice {
                index: 0,
                delta: Some(LlmMessage {
                    role: LlmMessageRole::Assistant,
                    content,
                    reasoning: None, // Anthropic API 不支持推理过程
                    tool_calls: None,
                    tool_call_id: None,
                }),
                finish_reason,
            }],
            usage,
        })
    }
}

#[async_trait]
impl ChatClient for AnthropicChatClient {
    async fn chat(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError> {
        let url = format!("{}/messages", provider.base_url);
        let req_body = self.convert_to_anthropic_request(&request);

        tracing::info!("Sending Anthropic-style chat request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &provider.api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| {
                LlmClientError::transport(format!("Failed to send Anthropic chat request: {e}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmClientError::api(format!(
                "Anthropic API returned error {status}: {error_text}"
            )));
        }

        let anthropic_response: Value = response.json().await.map_err(|e| {
            LlmClientError::unexpected(format!("Failed to parse Anthropic response: {e}"))
        })?;

        self.convert_anthropic_response(anthropic_response, &request.model)
    }

    async fn chat_stream(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    > {
        let url = format!("{}/messages", provider.base_url);
        let mut req_body = self.convert_to_anthropic_request(&request);
        req_body["stream"] = true.into(); // 启用流式

        tracing::info!("Sending Anthropic-style streaming request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("x-api-key", &provider.api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| {
                LlmClientError::transport(format!(
                    "Failed to send Anthropic streaming request: {e}"
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmClientError::api(format!(
                "Anthropic API returned error {status}: {error_text}"
            )));
        }

        // 真正解析Anthropic Claude流式响应
        tracing::info!("[Anthropic] Successfully initiated streaming request to Claude API");

        let model_name = request.model.clone();
        let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());

        // Anthropic使用SSE格式的流式响应
        let stream = response
            .bytes_stream()
            .map_err(|e| LlmClientError::network(format!("Stream error: {e}")))
            .map_ok(move |bytes| {
                let text = String::from_utf8_lossy(&bytes);
                let mut responses = Vec::new();

                // 解析Anthropic SSE格式
                for line in text.lines() {
                    let line = line.trim();

                    // Anthropic SSE事件格式: "event: content_block_delta", "data: {...}"
                    if line.starts_with("data: ") {
                        let json_str = &line[6..];
                        if json_str == "[DONE]" {
                            break;
                        }

                        if let Ok(anthropic_event) =
                            serde_json::from_str::<serde_json::Value>(json_str)
                        {
                            // Anthropic事件类型：content_block_delta包含文本增量
                            if let Some(event_type) =
                                anthropic_event.get("type").and_then(|t| t.as_str())
                            {
                                if event_type == "content_block_delta" {
                                    if let Some(delta) = anthropic_event
                                        .get("delta")
                                        .and_then(|delta| delta.get("text"))
                                        .and_then(|text| text.as_str())
                                    {
                                        let chat_response = LlmChunkResponse {
                                            id: response_id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            model: model_name.clone(),
                                            choices: vec![LlmChunkChoice {
                                                index: 0,
                                                delta: Some(LlmDeltaMessage {
                                                    role: Some(LlmMessageRole::Assistant),
                                                    content: Some(delta.to_string()),
                                                    reasoning: None, // Anthropic API 不支持推理过程
                                                    tool_calls: None,
                                                }),
                                                finish_reason: None,
                                            }],
                                            usage: None,
                                        };

                                        responses.push(chat_response);
                                        tracing::debug!(
                                            "[Anthropic Stream] Parsed content delta: '{delta}'"
                                        );
                                    }
                                } else if event_type == "message_stop" {
                                    // 流结束事件
                                    let chat_response = LlmChunkResponse {
                                        id: response_id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        model: model_name.clone(),
                                        choices: vec![LlmChunkChoice {
                                            index: 0,
                                            delta: Some(LlmDeltaMessage {
                                                role: Some(LlmMessageRole::Assistant),
                                                content: Some("".to_string()),
                                                reasoning: None, // Anthropic API 不支持推理过程
                                                tool_calls: None,
                                            }),
                                            finish_reason: Some("stop".to_string()),
                                        }],
                                        usage: None,
                                    };

                                    responses.push(chat_response);
                                    tracing::debug!("[Anthropic Stream] Stream completed");
                                }
                            }
                        } else {
                            tracing::warn!(
                                "[Anthropic Stream] Failed to parse SSE data: {}",
                                json_str
                            );
                        }
                    }
                }

                futures::stream::iter(responses.into_iter().map(Ok))
            })
            .try_flatten();

        Ok(Box::new(Box::pin(stream))
            as Box<
                dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin,
            >)
    }

    fn api_type(&self) -> &'static str {
        "anthropic"
    }
}

impl Default for AnthropicChatClient {
    fn default() -> Self {
        Self::new()
    }
}
