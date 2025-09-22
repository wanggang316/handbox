// Google 模型客户端实现

use super::model_client::ModelClient;
use crate::llm_client::types::{ModelFeature, StandardModel};
use crate::models::{AppError, Provider};
use async_trait::async_trait;
use serde::Deserialize;

/// Google 风格的模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleModelsResponse {
    pub models: Vec<GoogleModelData>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleModelData {
    pub name: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "inputTokenLimit")]
    pub input_token_limit: Option<i32>,
}

/// Google 模型客户端
pub struct GoogleModelClient {
    client: reqwest::Client,
}

impl GoogleModelClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelClient for GoogleModelClient {
    async fn list_models(
        &self,
        provider: &Provider,
        _provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching Google models from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Content-Type", "application/json")
            .query(&[("key", &provider.api_key)])
            .send()
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to fetch Google models: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AppError::internal_error(&format!(
                "Google API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: GoogleModelsResponse = response.json().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to parse Google response: {}", e))
        })?;

        let mut result_models = Vec::new();

        for api_model in models_response.models {
            // 解析 Google 模型名称 (格式: models/gemini-pro)
            let model_id = api_model
                .name
                .strip_prefix("models/")
                .unwrap_or(&api_model.name)
                .to_string();

            result_models.push(StandardModel {
                id: model_id.clone(),
                name: api_model.display_name,
                context_length: api_model.input_token_limit,
                input_cost: None,
                output_cost: None,
                supported_features: Some(vec![ModelFeature::Chat]),
            });
        }

        Ok(result_models)
    }
}
