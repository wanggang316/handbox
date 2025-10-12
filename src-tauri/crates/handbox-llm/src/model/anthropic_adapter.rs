// Anthropic 模型客户端实现

use super::model_client::ModelClient;
use crate::config::{LlmConfigProvider, LlmModelExtraInfo};
use crate::error::LlmClientError;
use crate::types::{LlmProvider, LlmStandardModel};
use async_trait::async_trait;
use std::sync::Arc;

/// Anthropic 模型客户端（基于本地配置）
pub struct AnthropicModelClient {
    config: Arc<dyn LlmConfigProvider>,
}

impl AnthropicModelClient {
    pub fn new(config: Arc<dyn LlmConfigProvider>) -> Self {
        Self { config }
    }

    fn convert_model_extra_info(
        &self,
        model_id: &str,
        extra_info: &LlmModelExtraInfo,
    ) -> LlmStandardModel {
        LlmStandardModel {
            id: model_id.to_string(),
            name: extra_info.name.clone(),
            context_length: extra_info.context_length,
            output_token_limit: extra_info.output_token_limit,
            input_cost: extra_info.input_cost_per_1k,
            output_cost: extra_info.output_cost_per_1k,
            supported_features: if extra_info.features.is_empty() {
                None
            } else {
                Some(extra_info.features.clone())
            },
            description: extra_info.description.clone(),
            input_modalities: extra_info.input_modalities.clone(),
            output_modalities: extra_info.output_modalities.clone(),
            metadata: extra_info.metadata.clone(),
            pricing: extra_info.pricing.clone(),
        }
    }
}

#[async_trait]
impl ModelClient for AnthropicModelClient {
    async fn list_models(
        &self,
        _provider: &LlmProvider,
        provider_type: &str,
    ) -> Result<Vec<LlmStandardModel>, LlmClientError> {
        // Anthropic 不提供公开的模型列表 API，返回预定义的模型列表
        let provider_config = self.config.get_provider_config(provider_type);

        if let Some(config) = provider_config {
            if let Some(local_models) = &config.model_local {
                let mut result_models = Vec::new();
                for (model_id, model_info) in local_models {
                    result_models.push(self.convert_model_extra_info(model_id, model_info));
                }
                return Ok(result_models);
            }
        }

        Ok(vec![])
    }
}
