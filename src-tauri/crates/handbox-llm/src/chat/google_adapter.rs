// Google Gemini API 客户端
// 使用 google-genai-rust SDK 进行通信

use crate::chat::ChatClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmChoice, LlmChunkChoice, LlmChunkResponse, LlmDeltaMessage, LlmGeneratedImage, LlmMessage,
    LlmMessageRole, LlmProvider, LlmRequest, LlmResponse, LlmThinkingConfig, LlmUsage,
};
use async_trait::async_trait;
use futures::StreamExt;
use google_genai_rust::types::{GenerateContentRequest, ThinkingConfig};

/// Google 风格聊天客户端
pub struct GoogleChatClient {
    // 不再使用 reqwest::Client，改用 google-genai-rust SDK
}

impl GoogleChatClient {
    pub fn new() -> Self {
        Self {}
    }

    /// 将通用请求转换为 Google SDK GenerateContentRequest
    fn convert_to_google_request(&self, request: &LlmRequest) -> GenerateContentRequest {
        use google_genai_rust::types::{Content, GenerationConfig, InlineData, Part};

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

                    // 构建 parts：文本 + 附件（如果有）
                    let mut parts = Vec::new();

                    // 添加文本内容
                    if !msg.content.is_empty() {
                        parts.push(Part::text(msg.content.clone()));
                    }

                    // 添加附件（图片等）
                    if let Some(attachments) = &msg.attachments {
                        for attachment in attachments {
                            // 将二进制数据转换为 base64
                            let base64_data = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &attachment.data,
                            );

                            let inline_data = InlineData {
                                mime_type: attachment.mime_type.clone(),
                                data: base64_data,
                                size_bytes: None,
                            };

                            parts.push(Part::inline_data(inline_data));
                        }
                    }

                    let content = Content {
                        role: Some(role.to_string()),
                        parts,
                    };
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

        let mut google_request = GenerateContentRequest::new(contents);
        google_request.system_instruction = system_instruction;
        google_request.generation_config = Some(generation_config);

        tracing::info!("Google request: {:?}", google_request);
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
            // 提取文本内容和图片
            let mut content = String::new();
            let mut reasoning_text = String::new();
            let mut generated_images = Vec::new();

            for part in candidate.iter_parts() {
                // 详细记录每个 part 的结构
                tracing::info!(
                    "[GoogleChatClient] Part: has_text={}, has_inline_data={}, has_generated_image={}, has_file_data={}, has_media_file_uri={}",
                    part.text.is_some(),
                    part.inline_data.is_some(),
                    part.generated_image.is_some(),
                    part.file_data.is_some(),
                    part.media_file_uri.is_some()
                );

                if let Some(text) = part.text.as_ref() {
                    if Self::is_reasoning_part(part) {
                        Self::append_reasoning(&mut reasoning_text, text);
                    } else {
                        content.push_str(text);
                    }
                }

                // 提取生成的图片 - 从 inline_data 中获取（gemini-2.5-flash-image 返回方式）
                if let Some(inline_data) = &part.inline_data {
                    tracing::info!(
                        "[GoogleChatClient] Found inline_data in part, mime_type: {}, has_data: {}, data_length: {}",
                        inline_data.mime_type,
                        !inline_data.data.is_empty(),
                        inline_data.data.len()
                    );
                    // 只处理图片类型的 inline_data
                    if inline_data.mime_type.starts_with("image/") {
                        generated_images.push(LlmGeneratedImage {
                            mime_type: inline_data.mime_type.clone(),
                            data: inline_data.data.clone(),
                        });
                    }
                }

                // 也检查 generated_image 字段（以防有些模型使用这个字段）
                if let Some(generated_image) = &part.generated_image {
                    tracing::info!(
                        "[GoogleChatClient] Found generated_image in part, mime_type: {:?}, has_data: {}",
                        generated_image.mime_type,
                        generated_image.data.is_some()
                    );
                    if let Some(ref data) = generated_image.data {
                        let mime_type = generated_image.mime_type.clone().unwrap_or_else(|| {
                            "image/png".to_string() // Google 默认生成 PNG
                        });
                        tracing::info!(
                            "[GoogleChatClient] Pushing generated image: mime_type={}, data_length={}",
                            mime_type,
                            data.len()
                        );
                        generated_images.push(LlmGeneratedImage {
                            mime_type,
                            data: data.clone(),
                        });
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

            let has_images = !generated_images.is_empty();
            tracing::info!(
                "[GoogleChatClient] Creating LlmChoice: index={}, content_len={}, has_images={}, image_count={}",
                index,
                content.len(),
                has_images,
                generated_images.len()
            );

            choices.push(LlmChoice {
                index: index as i32,
                delta: Some(LlmMessage {
                    role: LlmMessageRole::Assistant,
                    content,
                    reasoning: Self::normalize_reasoning(reasoning_text),
                    tool_calls: None,
                    tool_call_id: None,
                    attachments: None,
                }),
                finish_reason,
                generated_images: if generated_images.is_empty() {
                    None
                } else {
                    Some(generated_images)
                },
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
        // 提取文本增量和图片
        let mut delta_content = String::new();
        let mut finish_reason = None;
        let mut reasoning_text = String::new();
        let mut generated_images = Vec::new();

        if let Some(candidates) = &stream_response.candidates {
            for candidate in candidates {
                // 提取文本内容
                for part in candidate.iter_parts() {
                    // 详细记录每个 part 的结构
                    tracing::info!(
                        "[GoogleChatClient Stream] Part: has_text={}, has_inline_data={}, has_generated_image={}, has_file_data={}, has_media_file_uri={}",
                        part.text.is_some(),
                        part.inline_data.is_some(),
                        part.generated_image.is_some(),
                        part.file_data.is_some(),
                        part.media_file_uri.is_some()
                    );

                    if let Some(text) = part.text.as_ref() {
                        if Self::is_reasoning_part(part) {
                            Self::append_reasoning(&mut reasoning_text, text);
                        } else {
                            delta_content.push_str(text);
                        }
                    }

                    // 提取生成的图片 - 从 inline_data 中获取（gemini-2.5-flash-image 返回方式）
                    if let Some(inline_data) = &part.inline_data {
                        tracing::info!(
                            "[GoogleChatClient Stream] Found inline_data in part, mime_type: {}, has_data: {}, data_length: {}",
                            inline_data.mime_type,
                            !inline_data.data.is_empty(),
                            inline_data.data.len()
                        );
                        // 只处理图片类型的 inline_data
                        if inline_data.mime_type.starts_with("image/") {
                            generated_images.push(LlmGeneratedImage {
                                mime_type: inline_data.mime_type.clone(),
                                data: inline_data.data.clone(),
                            });
                        }
                    }

                    // 也检查 generated_image 字段（以防有些模型使用这个字段）
                    if let Some(generated_image) = &part.generated_image {
                        tracing::info!(
                            "[GoogleChatClient Stream] Found generated_image in stream part, mime_type: {:?}, has_data: {}",
                            generated_image.mime_type,
                            generated_image.data.is_some()
                        );
                        if let Some(ref data) = generated_image.data {
                            let mime_type = generated_image
                                .mime_type
                                .clone()
                                .unwrap_or_else(|| "image/png".to_string());
                            tracing::info!(
                                "[GoogleChatClient Stream] Adding image: mime_type={}, data_length={}",
                                mime_type,
                                data.len()
                            );
                            generated_images.push(LlmGeneratedImage {
                                mime_type,
                                data: data.clone(),
                            });
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

        if delta_content.is_empty()
            && reasoning_text.is_empty()
            && finish_reason.is_none()
            && generated_images.is_empty()
        {
            return None;
        }

        tracing::info!(
            "[GoogleChatClient Stream] Creating chunk response: has_content={}, has_reasoning={}, has_finish={}, image_count={}",
            !delta_content.is_empty(),
            !reasoning_text.is_empty(),
            finish_reason.is_some(),
            generated_images.len()
        );

        Some(LlmChunkResponse {
            id: response_id.to_string(),
            object: "chat.completion.chunk".to_string(),
            model: model.to_string(),
            choices: vec![LlmChunkChoice {
                index: 0,
                delta: Some(LlmDeltaMessage {
                    role: Some(LlmMessageRole::Assistant),
                    content: if delta_content.is_empty() {
                        None
                    } else {
                        Some(delta_content)
                    },
                    reasoning: Self::normalize_reasoning(reasoning_text),
                    tool_calls: None,
                }),
                finish_reason,
                generated_images: if generated_images.is_empty() {
                    None
                } else {
                    Some(generated_images)
                },
            }],
            usage: None,
        })
    }

    fn append_reasoning(buffer: &mut String, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(trimmed);
    }

    fn is_reasoning_part(part: &google_genai_rust::types::Part) -> bool {
        part.thought.unwrap_or(false) || part.thought_signature.is_some()
    }

    fn normalize_reasoning(reasoning: String) -> Option<String> {
        let trimmed = reasoning.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
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
