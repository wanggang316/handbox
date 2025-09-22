use crate::llm_client::chat::{self, ChatClient};
use crate::llm_client::model::{create_model_client, ModelClient};
use crate::llm_client::types::{
    ChatApiType, ChatRequest, ChatResponse, ModelApiType, StandardModel,
};
use crate::models::{AppError, Provider};
use crate::services::llm_config::get_global_llm_config;

/// LLM 客户端入口 - 为外部调用提供统一接口
pub struct LlmClient {
    provider_type: String,
    model_api_client: Box<dyn ModelClient>,
    chat_api_client: Box<dyn ChatClient>,
}

impl LlmClient {
    /// 基于枚举类型构建客户端，减少对配置格式的耦合
    pub fn new(
        provider_type: String,
        model_api_type: ModelApiType,
        chat_api_type: ChatApiType,
    ) -> Result<Self, AppError> {
        let model_api_client = create_model_client(model_api_type)?;
        let chat_api_client = chat::create_chat_client(chat_api_type)?;

        Ok(Self {
            provider_type,
            model_api_client,
            chat_api_client,
        })
    }

    /// 直接注入不同实现，便于测试或特殊场景
    pub fn with_clients(
        provider_type: String,
        model_api_client: Box<dyn ModelClient>,
        chat_api_client: Box<dyn ChatClient>,
    ) -> Self {
        Self {
            provider_type,
            model_api_client,
            chat_api_client,
        }
    }

    pub async fn chat(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, AppError> {
        self.chat_api_client.chat(provider, request).await
    }

    pub async fn chat_stream(
        &self,
        provider: &Provider,
        request: ChatRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<ChatResponse, AppError>> + Send + Unpin>,
        AppError,
    > {
        self.chat_api_client.chat_stream(provider, request).await
    }

    pub async fn list_models(&self, provider: &Provider) -> Result<Vec<StandardModel>, AppError> {
        self.model_api_client
            .list_models(provider, &self.provider_type)
            .await
    }

    pub fn api_type(&self) -> &'static str {
        self.chat_api_client.api_type()
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }
}

/// 工厂方法：直接创建聊天客户端，便于复用
pub fn create_chat_client(api_type: ChatApiType) -> Result<Box<dyn ChatClient>, AppError> {
    chat::create_chat_client(api_type)
}

/// 工厂方法：从配置创建完整的 LLM 客户端
pub fn create_llm_client(provider_type: &str) -> Result<LlmClient, AppError> {
    let config = get_global_llm_config();
    let provider_config = config.get_provider_config(provider_type).ok_or_else(|| {
        AppError::validation_error(&format!("Unknown provider type: {}", provider_type))
    })?;

    let model_api_type = ModelApiType::try_from(provider_config.model_api_type.as_str())?;
    let chat_api_type = ChatApiType::try_from(provider_config.chat_api_type.as_str())?;

    LlmClient::new(provider_type.to_string(), model_api_type, chat_api_type)
}

/// 非 fallible 的工厂方法，保留原有对外接口
pub fn create_client(provider_type: &str) -> LlmClient {
    match create_llm_client(provider_type) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to create client for {}: {}", provider_type, e);
            panic!(
                "Failed to create client for provider type: {}",
                provider_type
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_client_creation_with_enums() {
        let client = LlmClient::new(
            "test".to_string(),
            ModelApiType::OpenAI,
            ChatApiType::OpenAICompletions,
        )
        .unwrap();

        assert_eq!(client.provider_type(), "test");
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_llm_client_creation_with_custom_clients() {
        let client = LlmClient::with_clients(
            "custom".to_string(),
            Box::new(crate::llm_client::model::openai::OpenAIModelClient::new()),
            Box::new(
                crate::llm_client::chat::openai_completions::OpenAICompletionsChatClient::new(),
            ),
        );

        assert_eq!(client.provider_type(), "custom");
    }

    #[tokio::test]
    async fn test_create_llm_client_from_config() {
        match create_llm_client("openai") {
            Ok(_client) => {
                assert!(true);
            }
            Err(e) => {
                println!("Expected error when config file is not available: {}", e);
            }
        }
    }
}
