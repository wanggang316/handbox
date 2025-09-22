// API 功能接口提供者抽象层
// 根据不同的 api_type 处理聊天、流式响应等功能接口

use crate::models::{AppError, Provider};
use async_trait::async_trait;
use futures::StreamExt;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 聊天消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
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

/// OpenAI Completions 风格聊天客户端
pub struct OpenAIChatClient {
    // 不再需要 reqwest::Client，因为 openai-rust 库会处理 HTTP 请求
}

/// OpenAI Responses 风格聊天客户端
pub struct OpenAIResponsesChatClient {
    // 使用 openai-rust 库的 Responses API
}

impl OpenAIResponsesChatClient {
    pub fn new() -> Self {
        Self {}
    }

    /// 转换我们的 ChatRequest 到 openai-rust 的 CreateResponseRequest
    fn convert_to_openai_response_request(
        &self,
        request: &ChatRequest,
    ) -> openai_rust::types::CreateResponseRequest {
        // 将消息转换为 ResponseInput 格式
        let input_items: Vec<openai_rust::types::InputItem> = request
            .messages
            .iter()
            .map(|msg| openai_rust::types::InputItem::Message {
                role: msg.role.clone(),
                content: openai_rust::types::MessageContent::Text(msg.content.clone()),
            })
            .collect();

        // 提取系统指令
        let instructions = request
            .messages
            .iter()
            .find(|msg| msg.role == "system")
            .map(|msg| msg.content.clone());

        openai_rust::types::CreateResponseRequest {
            model: request.model.clone(),
            input: openai_rust::types::ResponseInput::Items(input_items),
            instructions,
            metadata: None,
            previous_response_id: None,
            tools: None,
            stream: request.stream,
        }
    }

    /// 转换 openai-rust 的 Response 到我们的 ChatResponse
    fn convert_from_openai_response(&self, response: openai_rust::types::Response) -> ChatResponse {
        let mut choices = Vec::new();

        // 从 output 中提取文本内容
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
                    reasoning: None, // Response API 可能有 reasoning 字段，但暂时设为 None
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

        let usage = response.usage.map(|usage| ChatUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens,
        });

        ChatResponse {
            id: response.id,
            object: "chat.completion".to_string(), // 保持兼容性
            model: response.model,
            choices,
            usage,
        }
    }

    /// 转换 openai-rust 的 ResponseStreamEvent 到我们的 ChatResponse
    fn convert_from_response_stream_event(
        &self,
        event: &openai_rust::types::ResponseStreamEvent,
        response_id: &str,
        model: &str,
    ) -> Option<ChatResponse> {
        match event {
            openai_rust::types::ResponseStreamEvent::OutputTextDelta { delta, .. } => {
                Some(ChatResponse {
                    id: response_id.to_string(),
                    object: "chat.completion.chunk".to_string(),
                    model: model.to_string(),
                    choices: vec![ChatChoice {
                        index: 0,
                        message: None,
                        delta: Some(ChatMessage {
                            role: "assistant".to_string(),
                            content: delta.clone(),
                            reasoning: None,
                        }),
                        finish_reason: None,
                    }],
                    usage: None,
                })
            }
            openai_rust::types::ResponseStreamEvent::ResponseCompleted { response, .. } => {
                Some(ChatResponse {
                    id: response_id.to_string(),
                    object: "chat.completion.chunk".to_string(),
                    model: model.to_string(),
                    choices: vec![ChatChoice {
                        index: 0,
                        message: None,
                        delta: Some(ChatMessage {
                            role: "assistant".to_string(),
                            content: "".to_string(),
                            reasoning: None,
                        }),
                        finish_reason: Some("stop".to_string()),
                    }],
                    usage: response.usage.as_ref().map(|usage| ChatUsage {
                        prompt_tokens: usage.input_tokens,
                        completion_tokens: usage.output_tokens,
                        total_tokens: usage.total_tokens,
                    }),
                })
            }
            _ => None, // 忽略其他事件类型
        }
    }
}

impl OpenAIChatClient {
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
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {}", e))
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
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {}", e))
            })?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use futures::StreamExt;
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
                            "OpenAI streaming API call failed: {}",
                            e
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
                        // 创建一个新的 OpenAIChatClient 实例来转换 chunk
                        let converter = OpenAIChatClient::new();
                        converter.convert_from_openai_chunk(chunk)
                    })
                    .map_err(|e| AppError::internal_error(&format!("Stream error: {}", e)));

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

#[async_trait]
impl ChatClient for OpenAIResponsesChatClient {
    async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        tracing::info!("Sending OpenAI-style response request using openai-rust library");

        // 创建 openai-rust 客户端
        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {}", e))
            })?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_response_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 调用 openai-rust 库的 responses API
        let openai_response = openai_client
            .responses()
            .create(&openai_request)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("OpenAI Responses API call failed: {}", e))
            })?;

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

        tracing::info!("Sending OpenAI-style streaming response request using openai-rust library");

        // 创建 openai-rust 客户端
        let openai_client = openai_rust::client::Client::builder()
            .api_key(provider.api_key.clone())
            .base_url(provider.base_url.clone())
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create OpenAI client: {}", e))
            })?;

        // 转换请求格式
        let openai_request = self.convert_to_openai_response_request(&request);

        tracing::debug!(
            "Request payload: {}",
            serde_json::to_string_pretty(&openai_request).unwrap_or_default()
        );

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use futures::StreamExt;
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::channel::<Result<ChatResponse, AppError>>(100);

        let response_id = format!("response-{}", uuid::Uuid::new_v4());
        let model_name = request.model.clone();

        // 在后台任务中处理流，将 openai_client 和 openai_request 的所有权转移进去
        tokio::spawn(async move {
            let responses = openai_client.responses();
            let openai_stream = match responses.create_stream(&openai_request).await {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = tx
                        .send(Err(AppError::internal_error(&format!(
                            "OpenAI Responses streaming API call failed: {}",
                            e
                        ))))
                        .await;
                    return;
                }
            };

            let mut openai_stream = Box::pin(openai_stream);
            while let Some(result) = openai_stream.next().await {
                let converted_result = result
                    .map(|chunk| {
                        tracing::debug!("[OpenAI Responses Stream] Received chunk: {:?}", chunk);
                        // 创建一个新的 OpenAIResponsesChatClient 实例来转换 chunk
                        let converter = OpenAIResponsesChatClient::new();
                        converter.convert_from_response_stream_event(
                            &chunk.event,
                            &response_id,
                            &model_name,
                        )
                    })
                    .map_err(|e| AppError::internal_error(&format!("Stream error: {}", e)));

                match converted_result {
                    Ok(Some(chat_response)) => {
                        if tx.send(Ok(chat_response)).await.is_err() {
                            // 接收端已关闭，退出
                            break;
                        }
                    }
                    Ok(None) => {
                        // 忽略不需要的事件
                        continue;
                    }
                    Err(e) => {
                        if tx.send(Err(e)).await.is_err() {
                            // 接收端已关闭，退出
                            break;
                        }
                    }
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
        "openai-responses"
    }
}

/// Google 风格聊天客户端
pub struct GoogleChatClient {
    // 不再使用 reqwest::Client，改用 google-genai-rust SDK
}

impl GoogleChatClient {
    pub fn new() -> Self {
        Self {}
    }

    /// 将通用请求转换为 Google SDK GenerateContentRequest
    fn convert_to_google_request(
        &self,
        request: &ChatRequest,
    ) -> google_genai_rust::types::GenerateContentRequest {
        use google_genai_rust::types::{Content, GenerationConfig};

        // 转换消息格式 - 将系统消息分离出来
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in &request.messages {
            match msg.role.as_str() {
                "system" => {
                    system_instruction = Some(Content::from_text("system", msg.content.clone()));
                }
                "user" | "assistant" => {
                    // Google API 使用 "user" 和 "model" 角色
                    let role = if msg.role == "assistant" {
                        "model"
                    } else {
                        "user"
                    };
                    let content = Content::from_text(role, msg.content.clone());
                    contents.push(content);
                }
                _ => {
                    // 其他角色默认作为用户消息
                    let content = Content::from_text("user", msg.content.clone());
                    contents.push(content);
                }
            }
        }

        // 创建生成配置
        let mut generation_config = GenerationConfig::default();
        if let Some(temperature) = request.temperature {
            generation_config.temperature = Some(temperature);
        }
        if let Some(max_tokens) = request.max_tokens {
            generation_config.max_output_tokens = Some(max_tokens);
        }

        let mut google_request = google_genai_rust::types::GenerateContentRequest::new(contents);
        google_request.system_instruction = system_instruction;
        google_request.generation_config = Some(generation_config);

        google_request
    }

    /// 将 Google SDK 响应转换为通用格式
    fn convert_google_response(
        &self,
        google_response: google_genai_rust::types::GenerateContentResponse,
        model: &str,
    ) -> Result<ChatResponse, AppError> {
        let mut choices = Vec::new();

        for (index, candidate) in google_response.candidates.iter().enumerate() {
            // 提取文本内容
            let mut content = String::new();
            if let Some(content_obj) = &candidate.content {
                for part in &content_obj.parts {
                    if let Some(text) = &part.text {
                        content.push_str(text);
                    }
                }
            }

            // 转换完成原因
            let finish_reason = candidate.finish_reason.as_ref().map(|reason| {
                match reason.as_str() {
                    "STOP" => "stop",
                    "MAX_TOKENS" => "length",
                    "SAFETY" => "content_filter",
                    "RECITATION" => "content_filter",
                    _ => "other",
                }
                .to_string()
            });

            choices.push(ChatChoice {
                index: index as i32,
                message: Some(ChatMessage {
                    role: "assistant".to_string(),
                    content,
                    reasoning: None, // Google API 不支持推理过程
                }),
                delta: None,
                finish_reason,
            });
        }

        // 转换使用统计
        let usage = google_response
            .usage_metadata
            .as_ref()
            .map(|usage| ChatUsage {
                prompt_tokens: usage.prompt_token_count.unwrap_or(0) as i32,
                completion_tokens: usage.candidates_token_count.unwrap_or(0) as i32,
                total_tokens: usage.total_token_count.unwrap_or(0) as i32,
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
        tracing::info!("Sending Google-style chat request using google-genai-rust SDK");

        // 创建 Google SDK 客户端
        let google_client = google_genai_rust::Client::builder(&provider.api_key)
            .base_url(&provider.base_url)
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create Google client: {}", e))
            })?;

        // 获取模型句柄
        let model = google_client.model(&request.model);

        // 转换请求格式
        let google_request = self.convert_to_google_request(&request);

        tracing::debug!("Request payload: {:?}", google_request);

        // 调用 Google SDK
        let google_response = model
            .generate_content(google_request)
            .await
            .map_err(|e| AppError::internal_error(&format!("Google API call failed: {}", e)))?;

        // 转换响应格式
        let chat_response = self.convert_google_response(google_response, &request.model);

        chat_response
    }

    async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        tracing::info!("Sending Google-style streaming request using google-genai-rust SDK");

        // 创建 Google SDK 客户端
        let google_client = google_genai_rust::Client::builder(&provider.api_key)
            .base_url(&provider.base_url)
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create Google client: {}", e))
            })?;

        // 获取模型句柄
        let model = google_client.model(&request.model);

        // 转换请求格式
        let google_request = self.convert_to_google_request(&request);

        tracing::debug!("Streaming request payload: {:?}", google_request);

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use tokio::sync::mpsc;
        let (tx, mut rx) = mpsc::channel::<Result<ChatResponse, AppError>>(100);

        let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let model_name = request.model.clone();

        // 在后台任务中处理流
        tokio::spawn(async move {
            let google_stream = model.stream_generate_content(google_request);

            let mut google_stream = Box::pin(google_stream);
            while let Some(result) = google_stream.next().await {
                let converted_result = result
                    .map(|chunk| {
                        tracing::debug!("[Google Stream] Received chunk: {:?}", chunk);
                        // 转换流式响应
                        GoogleChatClient::convert_google_stream_response(
                            &chunk,
                            &response_id,
                            &model_name,
                        )
                    })
                    .map_err(|e| AppError::internal_error(&format!("Stream error: {}", e)));

                match converted_result {
                    Ok(Some(chat_response)) => {
                        if tx.send(Ok(chat_response)).await.is_err() {
                            // 接收端已关闭，退出
                            break;
                        }
                    }
                    Ok(None) => {
                        // 忽略不需要的事件
                        continue;
                    }
                    Err(e) => {
                        if tx.send(Err(e)).await.is_err() {
                            // 接收端已关闭，退出
                            break;
                        }
                    }
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
        "google"
    }
}

impl GoogleChatClient {
    /// 将 Google SDK 流式响应转换为通用格式
    fn convert_google_stream_response(
        stream_response: &google_genai_rust::types::StreamGenerateContentResponse,
        response_id: &str,
        model: &str,
    ) -> Option<ChatResponse> {
        // 提取文本增量
        let mut delta_content = String::new();
        let mut finish_reason = None;

        if let Some(candidates) = &stream_response.candidates {
            for candidate in candidates {
                if let Some(content) = &candidate.content {
                    for part in &content.parts {
                        if let Some(text) = &part.text {
                            delta_content.push_str(text);
                        }
                    }
                }

                // 检查完成原因
                if let Some(reason) = &candidate.finish_reason {
                    finish_reason = Some(
                        match reason.as_str() {
                            "STOP" => "stop",
                            "MAX_TOKENS" => "length",
                            "SAFETY" => "content_filter",
                            "RECITATION" => "content_filter",
                            _ => "other",
                        }
                        .to_string(),
                    );
                }
            }
        }

        if delta_content.is_empty() && finish_reason.is_none() {
            return None;
        }

        Some(ChatResponse {
            id: response_id.to_string(),
            object: "chat.completion.chunk".to_string(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: None,
                delta: Some(ChatMessage {
                    role: "assistant".to_string(),
                    content: delta_content,
                    reasoning: None,
                }),
                finish_reason,
            }],
            usage: None,
        })
    }

    async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        tracing::info!("Sending Google-style streaming request using google-genai-rust SDK");

        // 创建 Google SDK 客户端
        let google_client = google_genai_rust::Client::builder(&provider.api_key)
            .base_url(&provider.base_url)
            .build()
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create Google client: {}", e))
            })?;

        // 获取模型句柄
        let model = google_client.model(&request.model);

        // 转换请求格式
        let google_request = self.convert_to_google_request(&request);

        tracing::debug!("Streaming request payload: {:?}", google_request);

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use tokio::sync::mpsc;
        let (tx, mut rx) = mpsc::channel::<Result<ChatResponse, AppError>>(100);

        let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let model_name = request.model.clone();

        // 在后台任务中处理流
        tokio::spawn(async move {
            let google_stream = model.stream_generate_content(google_request);

            let mut google_stream = Box::pin(google_stream);
            while let Some(result) = google_stream.next().await {
                let converted_result = result
                    .map(|chunk| {
                        tracing::debug!("[Google Stream] Received chunk: {:?}", chunk);
                        // 转换流式响应
                        GoogleChatClient::convert_google_stream_response(
                            &chunk,
                            &response_id,
                            &model_name,
                        )
                    })
                    .map_err(|e| AppError::internal_error(&format!("Stream error: {}", e)));

                match converted_result {
                    Ok(Some(chat_response)) => {
                        if tx.send(Ok(chat_response)).await.is_err() {
                            // 接收端已关闭，退出
                            break;
                        }
                    }
                    Ok(None) => {
                        // 忽略不需要的事件
                        continue;
                    }
                    Err(e) => {
                        if tx.send(Err(e)).await.is_err() {
                            // 接收端已关闭，退出
                            break;
                        }
                    }
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
                    reasoning: None, // Google API 不支持推理过程
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
                AppError::internal_error(&format!(
                    "Failed to send Anthropic streaming request: {}",
                    e
                ))
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
                                                    reasoning: None, // Anthropic API 不支持推理过程
                                                }),
                                                finish_reason: None,
                                            }],
                                            usage: None,
                                        };

                                        responses.push(chat_response);
                                        tracing::debug!(
                                            "[Anthropic Stream] Parsed content delta: '{}'",
                                            delta
                                        );
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
                                                reasoning: None, // Anthropic API 不支持推理过程
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
                dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin,
            >)
    }

    fn api_type(&self) -> &'static str {
        "anthropic"
    }
}

/// 聊天客户端工厂
pub fn create_chat_client(api_type: &str) -> Result<Box<dyn ChatClient>, AppError> {
    match api_type {
        "openai" | "openai-completions" => Ok(Box::new(OpenAIChatClient::new())),
        "openai-responses" => Ok(Box::new(OpenAIResponsesChatClient::new())),
        "google" => Ok(Box::new(GoogleChatClient::new())),
        "anthropic" => Ok(Box::new(AnthropicChatClient::new())),
        _ => Err(AppError::validation_error(&format!(
            "Unsupported API type: {}",
            api_type
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_openai_completions_client() {
        let client = create_chat_client("openai-completions").unwrap();
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_create_openai_responses_client() {
        let client = create_chat_client("openai-responses").unwrap();
        assert_eq!(client.api_type(), "openai-responses");
    }

    #[test]
    fn test_create_openai_legacy_client() {
        // Test backward compatibility with "openai"
        let client = create_chat_client("openai").unwrap();
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_create_unsupported_client() {
        let result = create_chat_client("unsupported");
        assert!(result.is_err());
    }
}
