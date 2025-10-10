// OpenAI + Local 增强模型客户端实现

use super::model_client::ModelClient;
use super::openai_adapter::OpenAIModelClient;
use crate::config::llm_config::{get_global_llm_config, ModelExtraInfo};
use crate::llm_client::types::{LlmModelFeature, LlmStandardModel};
use crate::models::{AppError, Provider};
use async_trait::async_trait;

/// OpenAI + Local 增强模型客户端
pub struct OpenAIWithLocalProvider {
    openai_provider: OpenAIModelClient,
}

impl OpenAIWithLocalProvider {
    pub fn new() -> Self {
        Self {
            openai_provider: OpenAIModelClient::new(),
        }
    }

    fn enhance_with_local_info(
        &self,
        mut models: Vec<LlmStandardModel>,
        provider_type: &str,
    ) -> Vec<LlmStandardModel> {
        let config = get_global_llm_config();

        for model in &mut models {
            if let Some(extra_info) = config.get_model_extra_info(provider_type, &model.id) {
                *model = self.convert_model_extra_info(&model.id, extra_info);
            }
        }

        models
    }

    fn convert_model_extra_info(
        &self,
        model_id: &str,
        extra_info: &ModelExtraInfo,
    ) -> LlmStandardModel {
        let _config = get_global_llm_config();
        LlmStandardModel {
            id: model_id.to_string(),
            name: extra_info.name.clone(),
            context_length: extra_info.context_length,
            input_cost: extra_info.input_cost_per_1k,
            output_cost: extra_info.output_cost_per_1k,
            supported_features: Some(
                extra_info
                    .features
                    .iter()
                    .map(|f| match f.as_str() {
                        "text" => LlmModelFeature::Chat,
                        "vision" => LlmModelFeature::Vision,
                        "function_calling" => LlmModelFeature::FunctionCalling,
                        "chat" => LlmModelFeature::Chat,
                        "completion" => LlmModelFeature::Completion,
                        "embedding" => LlmModelFeature::Embedding,
                        "streaming" => LlmModelFeature::Streaming,
                        _ => LlmModelFeature::Chat,
                    })
                    .collect(),
            ),
        }
    }
}

#[async_trait]
impl ModelClient for OpenAIWithLocalProvider {
    async fn list_models(
        &self,
        provider: &Provider,
        provider_type: &str,
    ) -> Result<Vec<LlmStandardModel>, AppError> {
        let models = self
            .openai_provider
            .list_models(provider, provider_type)
            .await?;
        Ok(self.enhance_with_local_info(models, provider_type))
    }
}
