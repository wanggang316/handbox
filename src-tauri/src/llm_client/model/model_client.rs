// 模型客户端 trait 和工厂函数

use crate::llm_client::types::{ModelApiType, StandardModel};
use crate::models::{AppError, Provider};
use async_trait::async_trait;

/// 模型客户端 trait
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn list_models(
        &self,
        provider: &Provider,
        provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError>;
}

/// 模型客户端工厂
pub fn create_model_client(api_type: ModelApiType) -> Result<Box<dyn ModelClient>, AppError> {
    Ok(match api_type {
        ModelApiType::OpenAI => {
            Box::new(crate::llm_client::model::openai_adapter::OpenAIModelClient::new()) as Box<_>
        }
        ModelApiType::OpenAIWithLocal => Box::new(
            crate::llm_client::model::openai_with_local_adapter::OpenAIWithLocalProvider::new(),
        ) as Box<_>,
        ModelApiType::Google => {
            Box::new(crate::llm_client::model::google_adapter::GoogleModelClient::new()) as Box<_>
        }
        ModelApiType::Anthropic => {
            Box::new(crate::llm_client::model::anthropic_adapter::AnthropicModelClient::new())
                as Box<_>
        }
        ModelApiType::OpenRouter => {
            Box::new(crate::llm_client::model::openrouter_adapter::OpenRouterModelClient::new())
                as Box<_>
        }
    })
}
