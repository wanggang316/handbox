// 模型客户端 trait 和工厂函数

use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::types::{LlmModelApiType, LlmProvider, LlmModel};
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
        LlmModelApiType::OpenAI => {
            Box::new(crate::model::openai_adapter::OpenAIModelClient::new()) as Box<_>
        }
        LlmModelApiType::OpenAIWithLocal => Box::new(
            crate::model::openai_with_local_adapter::OpenAIWithLocalProvider::new(Arc::clone(
                &config,
            )),
        ) as Box<_>,
        LlmModelApiType::Google => {
            Box::new(crate::model::google_adapter::GoogleModelClient::new()) as Box<_>
        }
        LlmModelApiType::Anthropic => Box::new(
            crate::model::anthropic_adapter::AnthropicModelClient::new(Arc::clone(&config)),
        ) as Box<_>,
        LlmModelApiType::OpenRouter => {
            Box::new(crate::model::openrouter_adapter::OpenRouterModelClient::new()) as Box<_>
        }
    })
}
