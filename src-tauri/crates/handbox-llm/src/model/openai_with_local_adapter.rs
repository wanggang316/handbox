// OpenAI + Local 增强模型客户端实现

use super::model_client::ModelClient;
use super::openai_adapter::OpenAIModelClient;
use crate::config::{LlmConfigProvider, LlmModelExtraInfo};
use crate::error::LlmClientError;
use crate::types::{LlmProvider, LlmStandardModel};
use async_trait::async_trait;
use std::sync::Arc;

/// OpenAI + Local 增强模型客户端
pub struct OpenAIWithLocalProvider {
    openai_provider: OpenAIModelClient,
    config: Arc<dyn LlmConfigProvider>,
}

impl OpenAIWithLocalProvider {
    pub fn new(config: Arc<dyn LlmConfigProvider>) -> Self {
        Self {
            openai_provider: OpenAIModelClient::new(),
            config,
        }
    }

    fn enhance_with_local_info(
        &self,
        mut models: Vec<LlmStandardModel>,
        provider_type: &str,
    ) -> Vec<LlmStandardModel> {
        for model in &mut models {
            if let Some(extra_info) = self.config.get_model_extra_info(provider_type, &model.id) {
                *model = self.convert_model_extra_info(&model.id, &extra_info);
            }
        }

        models
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
            parameters: None,
        }
    }
}

#[async_trait]
impl ModelClient for OpenAIWithLocalProvider {
    async fn list_models(
        &self,
        provider: &LlmProvider,
        provider_type: &str,
    ) -> Result<Vec<LlmStandardModel>, LlmClientError> {
        let models = self
            .openai_provider
            .list_models(provider, provider_type)
            .await?;
        Ok(self.enhance_with_local_info(models, provider_type))
    }
}
