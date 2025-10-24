// 模型客户端 trait 和工厂函数

use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::model::anthropic_adapter::AnthropicModelClient;
use crate::model::google_adapter::GoogleModelClient;
use crate::model::openai_adapter::OpenAIModelClient;
use crate::model::openrouter_adapter::OpenRouterModelClient;
use crate::types::{LlmModel, LlmModelApiType, LlmProvider};
use async_trait::async_trait;
use std::sync::Arc;

/// 模型客户端 trait
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn list_models(
        &self,
        provider: &LlmProvider,
        provider_type: &str,
    ) -> Result<Vec<LlmModel>, LlmClientError>;
}

/// 模型客户端工厂
pub fn create_model_client(
    api_type: LlmModelApiType,
    config: Arc<dyn LlmConfigProvider>,
) -> Result<Box<dyn ModelClient>, LlmClientError> {
    Ok(match api_type {
        LlmModelApiType::OpenAI => Box::new(OpenAIModelClient::new(Arc::clone(&config))) as Box<_>,
        LlmModelApiType::Google => Box::new(GoogleModelClient::new(Arc::clone(&config))) as Box<_>,
        LlmModelApiType::Anthropic => {
            Box::new(AnthropicModelClient::new(Arc::clone(&config))) as Box<_>
        }
        LlmModelApiType::OpenRouter => Box::new(OpenRouterModelClient::new()) as Box<_>,
    })
}
