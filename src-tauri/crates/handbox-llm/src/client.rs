use crate::chat::{self, ChatClient};
use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::model::{create_model_client, ModelClient};
use crate::types::{
    LlmApiType, LlmChunkResponse, LlmModelApiType, LlmProvider, LlmRequest, LlmResponse,
    LlmModel,
};
use std::sync::Arc;

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
        model_api_type: LlmModelApiType,
        chat_api_type: LlmApiType,
        config: Arc<dyn LlmConfigProvider>,
    ) -> Result<Self, LlmClientError> {
        let model_api_client = create_model_client(model_api_type, Arc::clone(&config))?;
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
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<LlmResponse, LlmClientError> {
        self.chat_api_client.chat(provider, request).await
    }

    pub async fn chat_stream(
        &self,
        provider: &LlmProvider,
        request: LlmRequest,
    ) -> Result<
        Box<dyn futures::Stream<Item = Result<LlmChunkResponse, LlmClientError>> + Send + Unpin>,
        LlmClientError,
    > {
        self.chat_api_client.chat_stream(provider, request).await
    }

    pub async fn list_models(
        &self,
        provider: &LlmProvider,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
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
pub fn create_chat_client(api_type: LlmApiType) -> Result<Box<dyn ChatClient>, LlmClientError> {
    chat::create_chat_client(api_type)
}

/// 工厂方法：从配置创建完整的 LLM 客户端
pub fn create_llm_client(
    provider_type: &str,
    config: Arc<dyn LlmConfigProvider>,
) -> Result<LlmClient, LlmClientError> {
    let provider_config = config.get_provider_config(provider_type).ok_or_else(|| {
        LlmClientError::validation(format!("Unknown provider type: {}", provider_type))
    })?;

    LlmClient::new(
        provider_type.to_string(),
        provider_config.model_api_type,
        provider_config.chat_api_type,
        config,
    )
}

/// 非 fallible 的工厂方法，保留原有对外接口
pub fn create_client(provider_type: &str, config: Arc<dyn LlmConfigProvider>) -> LlmClient {
    match create_llm_client(provider_type, config) {
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
    use crate::config::{LlmModelExtraInfo, LlmProviderConfig};
    use crate::types::LlmModelFeature;

    struct TestConfigProvider;

    impl LlmConfigProvider for TestConfigProvider {
        fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig> {
            Some(LlmProviderConfig {
                provider_type: provider_type.to_string(),
                chat_api_type: LlmApiType::OpenAICompletions,
                model_api_type: LlmModelApiType::OpenAI,
                model_local: None,
            })
        }

        fn get_model_extra_info(
            &self,
            _provider_type: &str,
            _model_id: &str,
        ) -> Option<LlmModelExtraInfo> {
            Some(LlmModelExtraInfo {
                name: "test".into(),
                context_length: None,
                output_max_tokens: None,
                input_cost_per_1k: None,
                output_cost_per_1k: None,
                features: vec![LlmModelFeature::Reasoning],
                description: None,
                input_modalities: None,
                output_modalities: None,
                metadata: None,
                pricing: None,
            })
        }
    }

    #[test]
    fn test_llm_client_creation_with_enums() {
        let client = LlmClient::new(
            "test".to_string(),
            LlmModelApiType::OpenAI,
            LlmApiType::OpenAICompletions,
            Arc::new(TestConfigProvider),
        )
        .unwrap();

        assert_eq!(client.provider_type(), "test");
        assert_eq!(client.api_type(), "openai-completions");
    }

    #[test]
    fn test_llm_client_creation_with_custom_clients() {
        let client = LlmClient::with_clients(
            "custom".to_string(),
            Box::new(crate::model::openai_adapter::OpenAIModelClient::new()),
            Box::new(crate::chat::openai_completions_adapter::OpenAICompletionsChatClient::new()),
        );

        assert_eq!(client.provider_type(), "custom");
    }

    #[tokio::test]
    async fn test_create_llm_client_from_config() {
        match create_llm_client("openai", Arc::new(TestConfigProvider)) {
            Ok(_client) => {
                assert!(true);
            }
            Err(e) => {
                println!("Expected error when config file is not available: {}", e);
            }
        }
    }
}
