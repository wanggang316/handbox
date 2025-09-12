// API 功能接口提供者抽象层
// 根据不同的 api_type 处理聊天、流式响应等功能接口

use crate::models::{AppError, Provider};
use async_trait::async_trait;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 聊天消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// 聊天请求结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
}

/// 聊天响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: i32,
    pub message: Option<ChatMessage>,
    pub delta: Option<ChatMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// 聊天客户端 trait
#[async_trait]
pub trait ChatClient: Send + Sync {
    /// 发送聊天请求
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError>;

    /// 发送流式聊天请求
    async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    >;

    /// 获取 API 类型名称
    fn api_type(&self) -> &'static str;
}

/// OpenAI 风格聊天客户端
pub struct OpenAIChatClient {
    // 不再需要 reqwest::Client，因为 openai-rust 库会处理 HTTP 请求
}

impl OpenAIChatClient {
    pub fn new() -> Self {
        Self {}
    }

    /// 转换我们的 ChatRequest 到 openai-rust 的 ChatCompletionRequest
    fn convert_to_openai_request(&self, request: &ChatRequest) -> openai_rust::types::ChatCompletionRequest {
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
                    role: choice.delta.role.map(|role| match role {
                        openai_rust::types::Role::System => "system".to_string(),
                        openai_rust::types::Role::User => "user".to_string(),
                        openai_rust::types::Role::Assistant => "assistant".to_string(),
                    }).unwrap_or_default(),
                    content: choice.delta.content.unwrap_or_default(),
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
impl ChatClient for OpenAIChatClient {
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
            .map_err(|e| AppError::internal_error(&format!("Failed to create OpenAI client: {}", e)))?;

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
            .map_err(|e| AppError::internal_error(&format!("OpenAI API call failed: {}", e)))?;

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
            .map_err(|e| AppError::internal_error(&format!("Failed to create OpenAI client: {}", e)))?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use tokio::sync::mpsc;
        use futures::StreamExt;
        
        let (tx, mut rx) = mpsc::channel::<Result<ChatResponse, AppError>>(100);
        
        // 在后台任务中处理流，将 openai_client 和 openai_request 的所有权转移进去
        tokio::spawn(async move {
            let completions = openai_client.completions();
            let openai_stream = match completions
                .create_stream(&openai_request)
                .await
            {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = tx.send(Err(AppError::internal_error(&format!("OpenAI streaming API call failed: {}", e)))).await;
                    return;
                }
            };
            
            let mut openai_stream = Box::pin(openai_stream);
            while let Some(result) = openai_stream.next().await {
                let converted_result = result.map(|chunk| {
                    tracing::debug!("[OpenAI Stream] Received chunk: {:?}", chunk);
                    // 创建一个新的 OpenAIChatClient 实例来转换 chunk
                    let converter = OpenAIChatClient::new();
                    converter.convert_from_openai_chunk(chunk)
                }).map_err(|e| AppError::internal_error(&format!("Stream error: {}", e)));
                
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
        
        Ok(Box::new(Box::pin(converted_stream)) as Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>)
    }

    fn api_type(&self) -> &'static str {
        "openai"
    }
}

/// Google 风格聊天客户端
pub struct GoogleChatClient {
    client: reqwest::Client,
}

impl GoogleChatClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 将通用请求转换为 Google API 格式
    fn convert_to_google_request(&self, request: &ChatRequest) -> Value {
        // 转换消息格式
        let contents: Vec<Value> = request
            .messages
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": if msg.role == "assistant" { "model" } else { "user" },
                    "parts": [{"text": msg.content}]
                })
            })
            .collect();

        let mut req_body = serde_json::json!({
            "contents": contents,
        });

        // 设置生成配置
        let mut generation_config = serde_json::json!({});

        if let Some(temperature) = request.temperature {
            generation_config["temperature"] = temperature.into();
        }

        if let Some(max_tokens) = request.max_tokens {
            generation_config["maxOutputTokens"] = max_tokens.into();
        }

        if !generation_config.as_object().unwrap().is_empty() {
            req_body["generationConfig"] = generation_config;
        }

        req_body
    }

    /// 将 Google 响应转换为通用格式
    fn convert_google_response(
        &self,
        google_response: Value,
        model: &str,
    ) -> Result<ChatResponse, AppError> {
        let candidates = google_response["candidates"]
            .as_array()
            .ok_or_else(|| AppError::internal_error("Invalid Google API response format"))?;

        let mut choices = Vec::new();

        for (index, candidate) in candidates.iter().enumerate() {
            let content = candidate["content"]["parts"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let finish_reason = candidate["finishReason"]
                .as_str()
                .map(|reason| match reason {
                    "STOP" => "stop",
                    "MAX_TOKENS" => "length",
                    _ => "other",
                })
                .map(String::from);

            choices.push(ChatChoice {
                index: index as i32,
                message: Some(ChatMessage {
                    role: "assistant".to_string(),
                    content,
                }),
                delta: None,
                finish_reason,
            });
        }

        // Google API 通常不返回使用统计，可以从响应中提取或设为 None
        let usage = google_response["usageMetadata"]
            .as_object()
            .map(|usage_obj| ChatUsage {
                prompt_tokens: usage_obj["promptTokenCount"].as_i64().unwrap_or(0) as i32,
                completion_tokens: usage_obj["candidatesTokenCount"].as_i64().unwrap_or(0) as i32,
                total_tokens: usage_obj["totalTokenCount"].as_i64().unwrap_or(0) as i32,
            });

        Ok(ChatResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            model: model.to_string(),
            choices,
            usage,
        })
    }
}

#[async_trait]
impl ChatClient for GoogleChatClient {
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        // Google API URL 格式不同，需要包含模型名称
        let url = format!(
            "{}/models/{}:generateContent",
            provider.base_url, request.model
        );
        let req_body = self.convert_to_google_request(&request);

        tracing::info!("Sending Google-style chat request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .query(&[("key", &provider.api_key)])
            .json(&req_body)
            .send()
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to send Google chat request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "Google API returned error {}: {}",
                status, error_text
            )));
        }

        let google_response: Value = response.json().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to parse Google response: {}", e))
        })?;

        self.convert_google_response(google_response, &request.model)
    }



    async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        // Google API流式URL格式
        let url = format!(
            "{}/models/{}:streamGenerateContent",
            provider.base_url, request.model
        );
        let req_body = self.convert_to_google_request(&request);

        tracing::info!("Sending Google-style streaming request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .query(&[("key", &provider.api_key)])
            .json(&req_body)
            .send()
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to send Google streaming request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "Google API returned error {}: {}",
                status, error_text
            )));
        }

        // 真正解析Google Gemini流式响应
        tracing::info!("[Google] Successfully initiated streaming request to Google Gemini API");
        
        let model_name = request.model.clone();
        let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        
        // Google Gemini 流式响应是多个连续的JSON对象，使用状态机解析
        use std::sync::Arc;
        use std::sync::Mutex;
        
        let accumulated_buffer = Arc::new(Mutex::new(String::new()));
        
        let stream = response
            .bytes_stream()
            .map_err(|e| AppError::network_error(&format!("Stream error: {}", e)))
            .map_ok(move |bytes| {
                let text = String::from_utf8_lossy(&bytes);
                let mut responses = Vec::new();
                
                // 将新数据添加到缓冲区
                let buffer_content = {
                    let mut buffer = accumulated_buffer.lock().unwrap();
                    buffer.push_str(&text);
                    buffer.clone()
                };
                
                // 解析缓冲区中的完整JSON对象
                let mut processed_length = 0;
                
                // 从日志看，Google发送的是连续的JSON对象，中间可能有逗号分隔
                // 尝试找到完整的JSON对象边界
                let mut start_idx = 0;
                let mut brace_count = 0;
                let mut in_string = false;
                let mut escape_next = false;
                
                for (i, ch) in buffer_content.char_indices() {
                    if escape_next {
                        escape_next = false;
                        continue;
                    }
                    
                    match ch {
                        '\\' if in_string => escape_next = true,
                        '"' => in_string = !in_string,
                        '{' if !in_string => {
                            if brace_count == 0 {
                                start_idx = i;
                            }
                            brace_count += 1;
                        },
                        '}' if !in_string => {
                            brace_count -= 1;
                            if brace_count == 0 {
                                // 找到完整的JSON对象
                                let json_str = &buffer_content[start_idx..=i];
                                if let Ok(gemini_response) = serde_json::from_str::<serde_json::Value>(json_str) {
                                    // 解析Google Gemini响应格式
                                    if let Some(candidates) = gemini_response.get("candidates")
                                        .and_then(|c| c.as_array()) {
                                        
                                        for candidate in candidates {
                                            if let Some(content) = candidate.get("content")
                                                .and_then(|content| content.get("parts"))
                                                .and_then(|parts| parts.as_array())
                                                .and_then(|parts| parts.first())
                                                .and_then(|part| part.get("text"))
                                                .and_then(|text| text.as_str()) {
                                                
                                                let finish_reason = candidate.get("finishReason")
                                                    .and_then(|f| f.as_str())
                                                    .map(|f| f.to_lowercase());
                                                
                                                let chat_response = ChatResponse {
                                                    id: response_id.clone(),
                                                    object: "chat.completion.chunk".to_string(),
                                                    model: model_name.clone(),
                                                    choices: vec![ChatChoice {
                                                        index: 0,
                                                        message: None,
                                                        delta: Some(ChatMessage {
                                                            role: "assistant".to_string(),
                                                            content: content.to_string(),
                                                        }),
                                                        finish_reason,
                                                    }],
                                                    usage: None,
                                                };
                                                
                                                responses.push(chat_response);
                                                tracing::debug!("[Google Stream] Parsed content chunk: '{}'", content);
                                            }
                                        }
                                    }
                                }
                                processed_length = i + 1;
                            }
                        },
                        _ => {}
                    }
                }
                
                // 清除已处理的数据
                if processed_length > 0 {
                    let mut buffer = accumulated_buffer.lock().unwrap();
                    let remaining = buffer_content[processed_length..].trim_start_matches(|c| c == ',' || c == ' ' || c == '\n' || c == '\r');
                    *buffer = remaining.to_string();
                }
                
                futures::stream::iter(responses.into_iter().map(Ok))
            })
            .try_flatten();
            
        Ok(Box::new(Box::pin(stream)) as Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>)
    }

    fn api_type(&self) -> &'static str {
        "google"
    }
}

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
    fn convert_to_anthropic_request(&self, request: &ChatRequest) -> Value {
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
    ) -> Result<ChatResponse, AppError> {
        let content = anthropic_response["content"][0]["text"]
            .as_str()
            .ok_or_else(|| AppError::internal_error("Invalid Anthropic API response format"))?
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
            .map(|usage_obj| ChatUsage {
                prompt_tokens: usage_obj["input_tokens"].as_i64().unwrap_or(0) as i32,
                completion_tokens: usage_obj["output_tokens"].as_i64().unwrap_or(0) as i32,
                total_tokens: (usage_obj["input_tokens"].as_i64().unwrap_or(0)
                    + usage_obj["output_tokens"].as_i64().unwrap_or(0))
                    as i32,
            });

        Ok(ChatResponse {
            id: anthropic_response["id"].as_str().unwrap_or("").to_string(),
            object: "chat.completion".to_string(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: Some(ChatMessage {
                    role: "assistant".to_string(),
                    content,
                }),
                delta: None,
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
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
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
                AppError::internal_error(&format!("Failed to send Anthropic chat request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "Anthropic API returned error {}: {}",
                status, error_text
            )));
        }

        let anthropic_response: Value = response.json().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to parse Anthropic response: {}", e))
        })?;

        self.convert_anthropic_response(anthropic_response, &request.model)
    }


    async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
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
                AppError::internal_error(&format!("Failed to send Anthropic streaming request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "Anthropic API returned error {}: {}",
                status, error_text
            )));
        }

        // 真正解析Anthropic Claude流式响应
        tracing::info!("[Anthropic] Successfully initiated streaming request to Claude API");
        
        let model_name = request.model.clone();
        let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        
        // Anthropic使用SSE格式的流式响应
        let stream = response
            .bytes_stream()
            .map_err(|e| AppError::network_error(&format!("Stream error: {}", e)))
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
                        
                        if let Ok(anthropic_event) = serde_json::from_str::<serde_json::Value>(json_str) {
                            // Anthropic事件类型：content_block_delta包含文本增量
                            if let Some(event_type) = anthropic_event.get("type").and_then(|t| t.as_str()) {
                                if event_type == "content_block_delta" {
                                    if let Some(delta) = anthropic_event.get("delta")
                                        .and_then(|delta| delta.get("text"))
                                        .and_then(|text| text.as_str()) {
                                        
                                        let chat_response = ChatResponse {
                                            id: response_id.clone(),
                                            object: "chat.completion.chunk".to_string(),
                                            model: model_name.clone(),
                                            choices: vec![ChatChoice {
                                                index: 0,
                                                message: None,
                                                delta: Some(ChatMessage {
                                                    role: "assistant".to_string(),
                                                    content: delta.to_string(),
                                                }),
                                                finish_reason: None,
                                            }],
                                            usage: None,
                                        };
                                        
                                        responses.push(chat_response);
                                        tracing::debug!("[Anthropic Stream] Parsed content delta: '{}'", delta);
                                    }
                                } else if event_type == "message_stop" {
                                    // 流结束事件
                                    let chat_response = ChatResponse {
                                        id: response_id.clone(),
                                        object: "chat.completion.chunk".to_string(),
                                        model: model_name.clone(),
                                        choices: vec![ChatChoice {
                                            index: 0,
                                            message: None,
                                            delta: Some(ChatMessage {
                                                role: "assistant".to_string(),
                                                content: "".to_string(),
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
                            tracing::warn!("[Anthropic Stream] Failed to parse SSE data: {}", json_str);
                        }
                    }
                }
                
                futures::stream::iter(responses.into_iter().map(Ok))
            })
            .try_flatten();
            
        Ok(Box::new(Box::pin(stream)) as Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>)
    }

    fn api_type(&self) -> &'static str {
        "anthropic"
    }
}

/// 聊天客户端工厂
pub fn create_chat_client(api_type: &str) -> Result<Box<dyn ChatClient>, AppError> {
    match api_type {
        "openai" => Ok(Box::new(OpenAIChatClient::new())),
        "google" => Ok(Box::new(GoogleChatClient::new())),
        "anthropic" => Ok(Box::new(AnthropicChatClient::new())),
        _ => Err(AppError::validation_error(&format!(
            "Unsupported API type: {}",
            api_type
        ))),
    }
}
