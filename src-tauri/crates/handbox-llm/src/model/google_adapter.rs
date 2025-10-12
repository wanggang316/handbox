// Google 模型客户端实现

use super::model_client::ModelClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmModelFeature,
    LlmModelModality,
    LlmProvider,
    LlmStandardModel,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

/// Google 风格的模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleModelsResponse {
    pub models: Vec<Value>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
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
        provider: &LlmProvider,
        _provider_type: &str,
    ) -> Result<Vec<LlmStandardModel>, LlmClientError> {
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
                LlmClientError::transport(format!("Failed to fetch Google models: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmClientError::api(format!(
                "Google API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: GoogleModelsResponse = response.json().await.map_err(|e| {
            LlmClientError::unexpected(format!("Failed to parse Google response: {}", e))
        })?;

        let mut result_models = Vec::new();

        for api_model in models_response.models {
            let full_name = match api_model.get("name").and_then(|v| v.as_str()) {
                Some(value) => value,
                None => continue,
            };

            let model_id = full_name.strip_prefix("models/").unwrap_or(full_name).to_string();

            let display_name = api_model
                .get("displayName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| model_id.clone());

            let context_length = parse_i32_field(api_model.get("inputTokenLimit"));
            let output_token_limit = parse_i32_field(api_model.get("outputTokenLimit"));
            let description = api_model
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let input_modalities = parse_modalities_field(
                api_model
                    .get("inputModalities")
                    .or_else(|| api_model.get("input_modalities")),
            );

            let output_modalities = parse_modalities_field(
                api_model
                    .get("outputModalities")
                    .or_else(|| api_model.get("output_modalities")),
            );

            let supported_features = parse_google_features(
                api_model.get("supportedGenerationMethods"),
            );

            result_models.push(LlmStandardModel {
                id: model_id,
                name: display_name,
                context_length,
                output_token_limit,
                input_cost: None,
                output_cost: None,
                supported_features,
                description,
                input_modalities,
                output_modalities,
                metadata: Some(api_model.clone()),
                pricing: None,
            });
        }

        Ok(result_models)
    }
}

fn parse_i32_field(value: Option<&Value>) -> Option<i32> {
    value.and_then(|v| match v {
        Value::Number(num) => num.as_i64().and_then(|raw| {
            if raw >= i32::MIN as i64 && raw <= i32::MAX as i64 {
                Some(raw as i32)
            } else {
                None
            }
        }),
        Value::String(text) => text.parse::<i32>().ok(),
        _ => None,
    })
}

fn parse_modalities_field(value: Option<&Value>) -> Option<Vec<LlmModelModality>> {
    fn push_modality(list: &mut Vec<LlmModelModality>, modality: LlmModelModality) {
        if !list.contains(&modality) {
            list.push(modality);
        }
    }

    fn map_modality(name: &str) -> Option<LlmModelModality> {
        match name {
            "text" => Some(LlmModelModality::Text),
            "image" => Some(LlmModelModality::Image),
            "file" => Some(LlmModelModality::File),
            "audio" => Some(LlmModelModality::Audio),
            "video" => Some(LlmModelModality::Video),
            _ => None,
        }
    }

    let mut result = Vec::new();

    match value {
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    if let Some(modality) = map_modality(name) {
                        push_modality(&mut result, modality);
                    }
                }
            }
        }
        Some(Value::String(name)) => {
            if let Some(modality) = map_modality(name) {
                push_modality(&mut result, modality);
            }
        }
        _ => {}
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn parse_google_features(value: Option<&Value>) -> Option<Vec<LlmModelFeature>> {
    let mut features = Vec::new();

    match value {
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    match name {
                        "reasoning" => features.push(LlmModelFeature::Reasoning),
                        "tool" | "toolUse" | "functionCall" => {
                            features.push(LlmModelFeature::Tool)
                        }
                        _ => {}
                    }
                }
            }
        }
        Some(Value::String(name)) => match name.as_str() {
            "reasoning" => features.push(LlmModelFeature::Reasoning),
            "tool" | "toolUse" | "functionCall" => features.push(LlmModelFeature::Tool),
            _ => {}
        },
        _ => {}
    }

    if features.is_empty() {
        None
    } else {
        features.dedup();
        Some(features)
    }
}
