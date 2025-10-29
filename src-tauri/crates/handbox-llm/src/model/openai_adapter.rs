// OpenAI 模型适配器实现

use super::model_client::ModelFetcher;
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

/// OpenAI 模型数据获取器
pub struct OpenAIFetcher {
    client: reqwest::Client,
}

impl OpenAIFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelFetcher for OpenAIFetcher {
    async fn fetch_base_models(
        &self,
        provider: &LlmProvider,
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

        let result_models = models_response
            .data
            .into_iter()
            .map(|api_model| LlmModel {
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
                created_at: api_model.created,
            })
            .collect();

        Ok(result_models)
    }
}
