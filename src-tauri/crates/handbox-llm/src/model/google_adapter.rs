// Google 模型客户端实现

use super::model_client::ModelClient;
use crate::error::LlmClientError;
use crate::types::{LlmModel, LlmModelFeature, LlmModelParameter, LlmProvider};
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
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching Google models from: {}", url);

        let mut result_models = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut request = self
                .client
                .get(&url)
                .header("Content-Type", "application/json")
                .query(&[("key", provider.api_key.as_str())]);

            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let response = request.send().await.map_err(|e| {
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

            let GoogleModelsResponse {
                models,
                next_page_token,
            } = models_response;

            for api_model in models {
                let full_name = match api_model.get("name").and_then(|v| v.as_str()) {
                    Some(value) => value,
                    None => continue,
                };

                let model_id = full_name
                    .strip_prefix("models/")
                    .unwrap_or(full_name)
                    .to_string();

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

                let supports_reasoning = api_model
                    .get("thinking")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);

                let supported_features = if supports_reasoning {
                    Some(vec![LlmModelFeature::Reasoning])
                } else {
                    None
                };

                let (support_parameters, default_parameters, max_parameters) =
                    parse_google_parameters(&api_model);

                result_models.push(LlmModel {
                    id: model_id,
                    name: display_name,
                    context_length,
                    output_token_limit,
                    input_cost: None,
                    output_cost: None,
                    supported_features,
                    description,
                    input_modalities: None,
                    output_modalities: None,
                    metadata: Some(api_model),
                    pricing: None,
                    support_parameters,
                    default_parameters,
                    max_parameters,
                });
            }

            match next_page_token {
                Some(token) if !token.is_empty() => {
                    page_token = Some(token);
                }
                _ => break,
            }
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

fn parse_google_parameters(
    api_model: &Value,
) -> (
    Vec<LlmModelParameter>,
    Option<std::collections::HashMap<String, serde_json::Value>>,
    Option<std::collections::HashMap<String, serde_json::Value>>,
) {
    fn push_param(list: &mut Vec<LlmModelParameter>, param: LlmModelParameter) {
        if !list.contains(&param) {
            list.push(param);
        }
    }

    let mut support_params = Vec::new();
    let mut default_params_map = std::collections::HashMap::new();
    let mut max_params_map = std::collections::HashMap::new();

    // 解析 temperature
    if let Some(temp) = api_model.get("temperature").and_then(|v| v.as_f64()) {
        push_param(&mut support_params, LlmModelParameter::Temperature);
        default_params_map.insert("temperature".to_string(), serde_json::json!(temp));
    }
    if let Some(max_temp) = api_model.get("maxTemperature").and_then(|v| v.as_f64()) {
        max_params_map.insert("temperature".to_string(), serde_json::json!(max_temp));
    }

    // 解析 topP
    if let Some(top_p) = api_model.get("topP").and_then(|v| v.as_f64()) {
        push_param(&mut support_params, LlmModelParameter::TopP);
        default_params_map.insert("top_p".to_string(), serde_json::json!(top_p));
    }

    // 解析 topK
    if let Some(top_k) = api_model.get("topK").and_then(|v| v.as_i64()) {
        push_param(&mut support_params, LlmModelParameter::TopK);
        default_params_map.insert("top_k".to_string(), serde_json::json!(top_k));
    }

    let default_result = if default_params_map.is_empty() {
        None
    } else {
        Some(default_params_map)
    };

    let max_result = if max_params_map.is_empty() {
        None
    } else {
        Some(max_params_map)
    };

    (support_params, default_result, max_result)
}
