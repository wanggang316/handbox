// OpenAI 模型客户端实现

use super::model_client::ModelClient;
use crate::error::LlmClientError;
use crate::types::{LlmModel, LlmModelModality, LlmProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::to_value;

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
        provider: &LlmProvider,
        _provider_type: &str,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching OpenAI-style models from: {}", url);

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
            result_models.push(LlmModel {
                id: api_model.id.clone(),
                name: api_model.id.clone(),
                context_length: None,
                output_token_limit: None,
                input_cost: None,
                output_cost: None,
                supported_features: None,
                description: None,
                input_modalities: Some(vec![LlmModelModality::Text]),
                output_modalities: Some(vec![LlmModelModality::Text]),
                metadata: to_value(&api_model).ok(),
                pricing: None,
                support_parameters: Vec::new(),
                default_parameters: None,
                max_parameters: None,
            });
        }

        Ok(result_models)
    }
}
