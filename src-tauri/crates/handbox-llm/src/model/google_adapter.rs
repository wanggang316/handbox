// Google 模型适配器实现

use super::model_client::ModelFetcher;
use crate::error::LlmClientError;
use crate::types::{convert_endpoints_to_methods, LlmModel, LlmModelModality, LlmProvider};
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

/// Google 模型数据获取器
pub struct GoogleFetcher {
    client: reqwest::Client,
}

impl GoogleFetcher {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelFetcher for GoogleFetcher {
    async fn fetch_base_models(
        &self,
        provider: &LlmProvider,
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
                if !supports_generate_content(api_model.get("supportedGenerationMethods")) {
                    continue;
                }

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
                let output_max_tokens = parse_i32_field(api_model.get("outputTokenLimit"));
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

                let supports_reasoning = api_model
                    .get("thinking")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);

                let supported_features = if supports_reasoning {
                    Some(vec!["reasoning".to_string()])
                } else {
                    None
                };

                let (support_parameters, default_parameters, max_parameters) =
                    parse_google_parameters(&api_model);

                // 提取 supportedGenerationMethods 并转换为 supported_methods
                let supported_methods = api_model
                    .get("supportedGenerationMethods")
                    .and_then(|v| v.as_array())
                    .map(|methods| {
                        let method_strings: Vec<String> = methods
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();

                        if method_strings.is_empty() {
                            None
                        } else {
                            Some(convert_endpoints_to_methods(&method_strings, "google"))
                        }
                    })
                    .flatten();

                let model = LlmModel {
                    id: model_id,
                    name: display_name,
                    context_length,
                    output_max_tokens,
                    supported_features,
                    description,
                    input_modalities,
                    output_modalities,
                    metadata: Some(api_model),
                    pricing: None,
                    url: None,
                    support_parameters,
                    default_parameters,
                    max_parameters,
                    supported_methods,
                };

                result_models.push(model);
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

fn supports_generate_content(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str())
            .any(|name| name == "generateContent"),
        Some(Value::String(name)) => name == "generateContent",
        _ => false,
    }
}

fn parse_modalities_field(value: Option<&Value>) -> Option<Vec<LlmModelModality>> {
    let mut result = Vec::new();

    match value {
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    if let Ok(modality) = name.parse::<LlmModelModality>() {
                        if !result.contains(&modality) {
                            result.push(modality);
                        }
                    }
                }
            }
        }
        Some(Value::String(name)) => {
            if let Ok(modality) = name.parse::<LlmModelModality>() {
                result.push(modality);
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

fn parse_google_parameters(
    api_model: &Value,
) -> (
    Vec<crate::types::LlmModelParameter>,
    Option<std::collections::HashMap<String, serde_json::Value>>,
    Option<std::collections::HashMap<String, serde_json::Value>>,
) {
    use crate::types::LlmModelParameter;

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

        let max_temp = api_model
            .get("maxTemperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(2.0);
        max_params_map.insert("temperature".to_string(), serde_json::json!(max_temp));
    } else if let Some(max_temp) = api_model.get("maxTemperature").and_then(|v| v.as_f64()) {
        push_param(&mut support_params, LlmModelParameter::Temperature);
        max_params_map.insert("temperature".to_string(), serde_json::json!(max_temp));
    }

    // 解析 topP
    if let Some(top_p) = api_model.get("topP").and_then(|v| v.as_f64()) {
        push_param(&mut support_params, LlmModelParameter::TopP);
        default_params_map.insert("top_p".to_string(), serde_json::json!(top_p));
        max_params_map.insert("top_p".to_string(), serde_json::json!(1.0));
    }

    // 解析 topK
    if let Some(top_k) = api_model.get("topK").and_then(|v| v.as_i64()) {
        push_param(&mut support_params, LlmModelParameter::TopK);
        default_params_map.insert("top_k".to_string(), serde_json::json!(top_k));
    }

    for param in [LlmModelParameter::MaxTokens, LlmModelParameter::Stop] {
        push_param(&mut support_params, param);
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
