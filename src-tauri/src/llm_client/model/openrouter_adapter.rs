// OpenRouter 模型客户端实现

use super::model_client::ModelClient;
use crate::llm_client::types::{ModelFeature, StandardModel};
use crate::models::{AppError, Provider};
use async_trait::async_trait;
use serde::Deserialize;

/// OpenRouter 风格的模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelsResponse {
    pub data: Vec<OpenRouterModelData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelData {
    pub id: String,
    pub name: Option<String>,
    pub context_length: Option<i32>,
    pub pricing: Option<OpenRouterPricing>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterPricing {
    pub prompt: Option<String>,
    pub completion: Option<String>,
}

/// OpenRouter 模型客户端
pub struct OpenRouterModelClient {
    client: reqwest::Client,
}

impl OpenRouterModelClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelClient for OpenRouterModelClient {
    async fn list_models(
        &self,
        provider: &Provider,
        _provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching OpenRouter models from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to fetch OpenRouter models: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "OpenRouter API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: OpenRouterModelsResponse = response.json().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to parse OpenRouter response: {}", e))
        })?;

        let mut result_models = Vec::new();

        for api_model in models_response.data {
            result_models.push(StandardModel {
                id: api_model.id.clone(),
                name: api_model
                    .name
                    .clone()
                    .unwrap_or_else(|| api_model.id.clone()),
                context_length: api_model.context_length,
                input_cost: api_model
                    .pricing
                    .as_ref()
                    .and_then(|p| p.prompt.as_ref())
                    .and_then(|s| s.parse().ok()),
                output_cost: api_model
                    .pricing
                    .as_ref()
                    .and_then(|p| p.completion.as_ref())
                    .and_then(|s| s.parse().ok()),
                supported_features: Some(vec![ModelFeature::Chat]),
            });
        }

        Ok(result_models)
    }
}
