// Google Gemini API 客户端
// 使用 google-genai-rust SDK 进行通信

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChoice, LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmMessage, LlmMessageRole,
    LlmProvider, LlmRequest, LlmResponse, LlmThinkingConfig, LlmUsage,
};
use async_trait::async_trait;
use futures::StreamExt;
use google_genai_rust::types::ThinkingConfig;

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
        request: &LlmRequest,
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
                    let role = if msg.role == LlmMessageRole::Assistant {
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
        if let Some(top_p) = request.top_p {
            generation_config.top_p = Some(top_p);
        }
        if let Some(top_k) = request.top_k {
            generation_config.top_k = Some(top_k);
        }
        if let Some(max_tokens) = request.max_tokens {
            generation_config.max_output_tokens = Some(max_tokens);
        }
        if let Some(thinking) = request.thinking.as_ref() {
            generation_config.thinking_config = Some(Self::map_thinking_config(thinking));
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
    ) -> Result<LlmResponse, LlmClientError> {
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

            choices.push(LlmChoice {
                index: index as i32,
                delta: Some(LlmMessage {
                    role: LlmMessageRole::Assistant,
                    content,
                    reasoning: None, // Google API 不支持推理过程
                    tool_calls: None,
                    tool_call_id: None,
                }),
                finish_reason,
            });
        }

        // 转换使用统计
        let usage = google_response
            .usage_metadata
            .as_ref()
            .map(|usage| LlmUsage {
                prompt_tokens: usage.prompt_token_count.unwrap_or(0) as i32,
                completion_tokens: usage.candidates_token_count.unwrap_or(0) as i32,
                total_tokens: usage.total_token_count.unwrap_or(0) as i32,
            });

        Ok(LlmResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            model: model.to_string(),
            choices,
            usage,
        })
    }

    /// 将 Google SDK 流式响应转换为通用格式
    fn convert_google_stream_response(
        stream_response: &google_genai_rust::types::StreamGenerateContentResponse,
        response_id: &str,
        model: &str,
    ) -> Option<LlmChunkResponse> {
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

        Some(LlmChunkResponse {
            id: response_id.to_string(),
            object: "chat.completion.chunk".to_string(),
            model: model.to_string(),
            choices: vec![LlmChunkChoice {
                index: 0,
                delta: Some(LlmDeltaMessage {
                    role: Some(LlmMessageRole::Assistant),
                    content: Some(delta_content),
                    reasoning: None,
                    tool_calls: None,
                }),
                finish_reason,
            }],
            usage: None,
        })
    }
}

impl GoogleChatClient {
    fn map_thinking_config(thinking: &LlmThinkingConfig) -> ThinkingConfig {
        ThinkingConfig {
            include_thoughts: thinking.include_thoughts,
            thinking_budget: thinking.thinking_budget,
        }
    }
}

#[async_trait]
impl ChatClient for GoogleChatClient {
    async fn chat(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError> {
        tracing::info!("Sending Google-style chat request using google-genai-rust SDK");

        // 创建 Google SDK 客户端
        let google_client = google_genai_rust::Client::builder(&provider.api_key)
            .base_url(&provider.base_url)
            .build()
            .map_err(|e| {
                LlmClientError::client_initialization(format!(
                    "Failed to create Google client: {e}"
                ))
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
            .map_err(|e| LlmClientError::api(format!("Google API call failed: {e}")))?;

        // 转换响应格式
        let chat_response = self.convert_google_response(google_response, &request.model);

        chat_response
    }

    async fn chat_stream(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    > {
        tracing::info!("Sending Google-style streaming request using google-genai-rust SDK");

        // 创建 Google SDK 客户端
        let google_client = google_genai_rust::Client::builder(&provider.api_key)
            .base_url(&provider.base_url)
            .build()
            .map_err(|e| {
                LlmClientError::client_initialization(format!(
                    "Failed to create Google client: {e}"
                ))
            })?;

        // 获取模型句柄
        let model = google_client.model(&request.model);

        // 转换请求格式
        let google_request = self.convert_to_google_request(&request);

        tracing::debug!("Streaming request payload: {:?}", google_request);

        // 使用 tokio::spawn 和 mpsc 来创建一个真正的流式传输
        use tokio::sync::mpsc;
        let (tx, mut rx) = mpsc::channel::<Result<LlmChunkResponse, LlmClientError>>(100);

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
                    .map_err(|e| LlmClientError::api(format!("Stream error: {e}")));

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
                dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin,
            >)
    }

    fn api_type(&self) -> &'static str {
        "google"
    }
}

impl Default for GoogleChatClient {
    fn default() -> Self {
        Self::new()
    }
}
