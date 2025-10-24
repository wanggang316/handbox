// OpenAI 模型客户端实现

use super::model_client::ModelClient;
use super::supplement::{OssSupplementProvider, SupplementProvider};
use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::types::{
    convert_endpoints_to_methods, merge_pricing, LlmModel, LlmModelModality, LlmProvider,
    ModelSupplement,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::sync::Arc;

/// OpenAI 风格的模型列表响应
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIModelsResponse {
    pub object: String,
    pub data: Vec<OpenAIModelData>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAIModelData {
    pub id: String,
    pub object: String,
    pub created: Option<i64>,
    pub owned_by: Option<String>,
}

/// OpenAI 风格模型客户端
pub struct OpenAIModelClient {
    client: reqwest::Client,
    supplement_provider: OssSupplementProvider,
}

impl OpenAIModelClient {
    pub fn new(config: Arc<dyn LlmConfigProvider>) -> Self {
        Self {
            client: reqwest::Client::new(),
            supplement_provider: OssSupplementProvider::new(config),
        }
    }

    /// Merge supplement data into base model (OpenAI-specific logic)
    fn merge_supplement(
        &self,
        model: &mut LlmModel,
        supplement: &ModelSupplement,
        provider_type: &str,
    ) {
        // Merge basic fields - prefer supplement if present
        if let Some(context_length) = supplement.context_length {
            model.context_length = Some(context_length);
        }
        if let Some(output_max_tokens) = supplement.output_max_tokens {
            model.output_max_tokens = Some(output_max_tokens);
        }
        if let Some(ref description) = supplement.description {
            if !description.trim().is_empty() {
                model.description = Some(description.clone());
            }
        }
        if let Some(ref url) = supplement.url {
            if !url.trim().is_empty() {
                model.url = Some(url.clone());
            }
        }

        // Merge collections - prefer supplement if present
        if let Some(ref features) = supplement.supported_features {
            if !features.is_empty() {
                model.supported_features = Some(features.clone());
            }
        }
        if let Some(ref modalities) = supplement.input_modalities {
            if !modalities.is_empty() {
                model.input_modalities = Some(modalities.clone());
            }
        }
        if let Some(ref modalities) = supplement.output_modalities {
            if !modalities.is_empty() {
                model.output_modalities = Some(modalities.clone());
            }
        }

        // Merge parameters
        if !supplement.support_parameters.is_empty() {
            model.support_parameters = supplement.support_parameters.clone();
        }
        if let Some(ref params) = supplement.default_parameters {
            if !params.is_empty() {
                model.default_parameters = Some(params.clone());
            }
        }
        if let Some(ref params) = supplement.max_parameters {
            if !params.is_empty() {
                model.max_parameters = Some(params.clone());
            }
        }

        // Merge metadata
        if supplement.metadata.is_some() {
            model.metadata = supplement.metadata.clone();
        }

        // Merge pricing
        let currency = supplement.currency.as_deref().or(Some("USD"));
        merge_pricing(
            &mut model.pricing,
            supplement.input_cost,
            supplement.output_cost,
            currency,
        );

        // Merge supported_methods - prefer supplement if present, otherwise convert from endpoints
        if let Some(ref methods) = supplement.supported_methods {
            if !methods.is_empty() {
                model.supported_methods = Some(methods.clone());
            }
        } else if !supplement.endpoints.is_empty() {
            // Convert endpoints to supported_methods for OpenAI
            let methods = convert_endpoints_to_methods(&supplement.endpoints, provider_type);
            model.supported_methods = Some(methods);
        }
    }
}

#[async_trait]
impl ModelClient for OpenAIModelClient {
    async fn list_models(
        &self,
        provider: &LlmProvider,
        provider_type: &str,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching OpenAI-style models from: {}", url);

        // Load supplements
        let supplement_map = self
            .supplement_provider
            .load_supplements(provider_type)
            .await?;

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| LlmClientError::transport(format!("Failed to fetch models: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmClientError::api(format!(
                "API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: OpenAIModelsResponse = response
            .json()
            .await
            .map_err(|e| LlmClientError::unexpected(format!("Failed to parse response: {}", e)))?;

        let mut result_models = Vec::new();

        for api_model in models_response.data {
            let mut model = LlmModel {
                id: api_model.id.clone(),
                name: api_model.id.clone(),
                context_length: None,
                output_max_tokens: None,
                supported_features: None,
                description: None,
                input_modalities: Some(vec![LlmModelModality::Text]),
                output_modalities: Some(vec![LlmModelModality::Text]),
                metadata: to_value(&api_model).ok(),
                pricing: None,
                url: None,
                support_parameters: Vec::new(),
                default_parameters: None,
                max_parameters: None,
                supported_methods: None,
            };

            // Merge supplement data
            if let Some(ref map) = supplement_map {
                if let Some(supplement) = map.get(&model.id) {
                    self.merge_supplement(&mut model, supplement, provider_type);
                }
            }

            result_models.push(model);
        }

        Ok(result_models)
    }
}
