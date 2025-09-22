// OpenAI 模型客户端实现

use super::model_client::ModelClient;
use crate::llm_client::types::{ModelFeature, StandardModel};
use crate::models::{AppError, Provider};
use async_trait::async_trait;
use serde::Deserialize;

/// OpenAI 风格的模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIModelsResponse {
    pub object: String,
    pub data: Vec<OpenAIModelData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIModelData {
    pub id: String,
    pub object: String,
    pub created: Option<i64>,
    pub owned_by: Option<String>,
}

/// OpenAI 风格模型客户端
pub struct OpenAIModelClient {
    client: reqwest::Client,
}

impl OpenAIModelClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelClient for OpenAIModelClient {
    async fn list_models(
        &self,
        provider: &Provider,
        _provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching OpenAI-style models from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to fetch models: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: OpenAIModelsResponse = response
            .json()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to parse response: {}", e)))?;

        let mut result_models = Vec::new();

        for api_model in models_response.data {
            result_models.push(StandardModel {
                id: api_model.id.clone(),
                name: api_model.id.clone(),
                context_length: None,
                input_cost: None,
                output_cost: None,
                supported_features: Some(vec![ModelFeature::Chat]),
            });
        }

        Ok(result_models)
    }
}
