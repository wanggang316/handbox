// Anthropic 模型客户端实现

use super::model_client::ModelClient;
use crate::llm_client::types::{ModelFeature, StandardModel};
use crate::models::{AppError, Provider};
use crate::services::llm_config::{get_global_llm_config, ModelExtraInfo};
use async_trait::async_trait;

/// Anthropic 模型客户端（基于本地配置）
pub struct AnthropicModelClient;

impl AnthropicModelClient {
    pub fn new() -> Self {
        Self
    }

    fn convert_model_extra_info(
        &self,
        model_id: &str,
        extra_info: &ModelExtraInfo,
    ) -> StandardModel {
        let _config = get_global_llm_config();
        StandardModel {
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
                        "text" => ModelFeature::Chat,
                        "vision" => ModelFeature::Vision,
                        "function_calling" => ModelFeature::FunctionCalling,
                        "chat" => ModelFeature::Chat,
                        "completion" => ModelFeature::Completion,
                        "embedding" => ModelFeature::Embedding,
                        "streaming" => ModelFeature::Streaming,
                        _ => ModelFeature::Chat,
                    })
                    .collect(),
            ),
        }
    }
}

#[async_trait]
impl ModelClient for AnthropicModelClient {
    async fn list_models(
        &self,
        _provider: &Provider,
        provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError> {
        // Anthropic 不提供公开的模型列表 API，返回预定义的模型列表
        let config = get_global_llm_config();
        let provider_config = config.get_provider_config(provider_type);

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
