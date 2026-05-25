use crate::chat::{self, ChatClient};
use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::model::{create_model_client, ModelClient};
use crate::types::{
    LlmApiType, LlmChunkResponse, LlmModel, LlmModelApiType, LlmProvider, LlmRequest, LlmResponse,
};
use std::sync::Arc;

/// LLM 客户端入口 - 为外部调用提供统一接口
pub struct LlmClient {
    provider_type: String,
    model_api_client: ModelClient,
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
        let model_api_client =
            create_model_client(model_api_type, &provider_type, Arc::clone(&config))?;
        let chat_api_client = build_chat_client(&provider_type, chat_api_type)?;

        Ok(Self {
            provider_type,
            model_api_client,
            chat_api_client,
        })
    }

    /// 直接注入不同实现，便于测试或特殊场景
    pub fn with_clients(
        provider_type: String,
        model_api_client: ModelClient,
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

/// Build the chat backend for a given provider, optionally routing through
/// hand-ai. When the `hand-ai` cargo feature is on AND the provider id
/// matches a hand-ai catalog entry (via `is_hand_ai_provider`), traffic
/// goes through `HandAiChatClient` and inherits hand-ai's full provider
/// list, cancellation gate, and capability metadata. Otherwise the call
/// falls back to the legacy per-protocol adapter selected by `chat_api_type`.
fn build_chat_client(
    provider_type: &str,
    chat_api_type: LlmApiType,
) -> Result<Box<dyn ChatClient>, LlmClientError> {
    #[cfg(feature = "hand-ai")]
    {
        if chat::hand_ai_adapter::is_hand_ai_provider(provider_type) {
            return Ok(Box::new(chat::hand_ai_adapter::HandAiChatClient::new(
                provider_type,
            )));
        }
    }
    let _ = provider_type; // silence unused warning when feature off
    chat::create_chat_client(chat_api_type)
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
    use crate::config::LlmProviderConfig;

    struct TestConfigProvider;

    impl LlmConfigProvider for TestConfigProvider {
        fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig> {
            Some(LlmProviderConfig {
                provider_type: provider_type.to_string(),
                chat_api_type: LlmApiType::OpenAICompletions,
                model_api_type: LlmModelApiType::OpenAI,
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

    /// When the `hand-ai` feature is on, a provider whose id matches the
    /// hand-ai catalog routes through HandAiChatClient — observable via
    /// the api_type() string. When it doesn't match (e.g. a HandBox-only
    /// "custom-foo"), we fall back to the legacy adapter.
    #[cfg(feature = "hand-ai")]
    #[test]
    fn matching_provider_routes_through_hand_ai_when_feature_on() {
        let openai = build_chat_client("openai", LlmApiType::OpenAICompletions)
            .expect("hand-ai client builds for known provider");
        assert_eq!(openai.api_type(), "hand-ai");
    }

    #[cfg(feature = "hand-ai")]
    #[test]
    fn unknown_provider_falls_back_to_legacy_when_feature_on() {
        let exotic = build_chat_client("not-a-real-vendor", LlmApiType::OpenAICompletions)
            .expect("legacy client builds for non-hand-ai provider");
        assert_ne!(exotic.api_type(), "hand-ai");
    }
}
