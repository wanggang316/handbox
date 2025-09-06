// API 功能接口提供者抽象层
// 根据不同的 api_type 处理聊天、流式响应等功能接口

use crate::models::{AppError, Provider};
use async_trait::async_trait;
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

/// API 功能接口提供者 trait
#[async_trait]
pub trait ApiProvider: Send + Sync {
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

/// OpenAI 风格 API 提供者
pub struct OpenAIApiProvider {
    client: reqwest::Client,
}

impl OpenAIApiProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// 构建 OpenAI 风格的请求体
    fn build_openai_request(&self, request: &ChatRequest) -> Value {
        let mut req_body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(temperature) = request.temperature {
            req_body["temperature"] = temperature.into();
        }

        if let Some(max_tokens) = request.max_tokens {
            req_body["max_tokens"] = max_tokens.into();
        }

        if let Some(stream) = request.stream {
            req_body["stream"] = stream.into();
        }

        req_body
    }
}

#[async_trait]
impl ApiProvider for OpenAIApiProvider {
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        let url = format!("{}/chat/completions", provider.base_url);
        let req_body = self.build_openai_request(&request);

        tracing::info!("Sending OpenAI-style chat request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to send chat request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "API returned error {}: {}",
                status, error_text
            )));
        }

        let chat_response: ChatResponse = response.json().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to parse chat response: {}", e))
        })?;

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

        let url = format!("{}/chat/completions", provider.base_url);
        let req_body = self.build_openai_request(&request);

        tracing::info!("Sending OpenAI-style streaming chat request to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .json(&req_body)
            .send()
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to send streaming chat request: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "API returned error {}: {}",
                status, error_text
            )));
        }

        // 这里需要实现 SSE 流解析逻辑
        // 为了简化，先返回一个错误，表示流式功能待实现
        Err(AppError::internal_error("Streaming not yet implemented"))
    }

    fn api_type(&self) -> &'static str {
        "openai"
    }
}

/// Google 风格 API 提供者
pub struct GoogleApiProvider {
    client: reqwest::Client,
}

impl GoogleApiProvider {
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
impl ApiProvider for GoogleApiProvider {
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
        _provider: &Provider,
        _request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        // Google 流式 API 实现待完成
        Err(AppError::internal_error(
            "Google streaming not yet implemented",
        ))
    }

    fn api_type(&self) -> &'static str {
        "google"
    }
}

/// Anthropic 风格 API 提供者
pub struct AnthropicApiProvider {
    client: reqwest::Client,
}

impl AnthropicApiProvider {
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
impl ApiProvider for AnthropicApiProvider {
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
        _provider: &Provider,
        _request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        // Anthropic 流式 API 实现待完成
        Err(AppError::internal_error(
            "Anthropic streaming not yet implemented",
        ))
    }

    fn api_type(&self) -> &'static str {
        "anthropic"
    }
}

/// API 提供者工厂
pub fn create_api_provider(api_type: &str) -> Result<Box<dyn ApiProvider>, AppError> {
    match api_type {
        "openai" => Ok(Box::new(OpenAIApiProvider::new())),
        "google" => Ok(Box::new(GoogleApiProvider::new())),
        "anthropic" => Ok(Box::new(AnthropicApiProvider::new())),
        _ => Err(AppError::validation_error(&format!(
            "Unsupported API type: {}",
            api_type
        ))),
    }
}
