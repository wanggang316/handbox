// OpenAI 模型客户端实现

use super::model_client::ModelClient;
use super::oss_client::OssClient;
use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::types::{
    convert_endpoints_to_methods, extract_pricing_value, merge_pricing, LlmModel, LlmModelModality,
    LlmProvider, ModelSupplementDocument,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::collections::HashMap;
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
    config: Arc<dyn LlmConfigProvider>,
}

impl OpenAIModelClient {
    pub fn new(config: Arc<dyn LlmConfigProvider>) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    async fn load_model_supplements(
        &self,
        provider_type: &str,
    ) -> Option<HashMap<String, LlmModel>> {
        let supplement_file = self
            .config
            .get_provider_config(provider_type)
            .and_then(|cfg| cfg.supplement_file)
            .and_then(|file| {
                let trimmed = file.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });

        let supplement_file = match supplement_file {
            Some(file) => file,
            None => return None,
        };

        let client = match OssClient::from_env() {
            Ok(client) => client,
            Err(err) => {
                tracing::debug!(
                    "OSS config not available for {} supplement '{}': {}",
                    provider_type,
                    supplement_file,
                    err
                );
                return None;
            }
        };

        let content = match client.get_object_text(&supplement_file).await {
            Ok(text) => text,
            Err(err) => {
                tracing::debug!(
                    "Unable to download supplement '{}' for {}: {}",
                    supplement_file,
                    provider_type,
                    err
                );
                return None;
            }
        };

        let document: ModelSupplementDocument = match serde_json::from_str(&content) {
            Ok(doc) => doc,
            Err(err) => {
                tracing::debug!(
                    "Unable to parse supplement '{}' for {}: {}",
                    supplement_file,
                    provider_type,
                    err
                );
                return None;
            }
        };

        if document.models.is_empty() {
            return None;
        }

        let mut models = HashMap::new();

        for entry in document.models {
            for (model_id, model) in entry.into_snapshot_models() {
                models.insert(model_id, model);
            }
        }

        Some(models)
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

        let supplement_map = self.load_model_supplements(provider_type).await;

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

            if let Some(map) = &supplement_map {
                if let Some(supplement) = map.get(&model.id) {
                    merge_openai_model(&mut model, supplement, provider_type);
                }
            }

            result_models.push(model);
        }

        Ok(result_models)
    }
}

fn merge_openai_model(base: &mut LlmModel, supplement: &LlmModel, provider_type: &str) {
    if let Some(context_length) = supplement.context_length {
        base.context_length = Some(context_length);
    }

    if let Some(output_max_tokens) = supplement.output_max_tokens {
        base.output_max_tokens = Some(output_max_tokens);
    }

    let currency = supplement
        .pricing
        .as_ref()
        .and_then(|value| value.currency.as_deref())
        .or(Some("USD"));

    merge_pricing(
        &mut base.pricing,
        extract_pricing_value(&supplement.pricing, "input_text"),
        extract_pricing_value(&supplement.pricing, "output_text"),
        currency,
    );

    if let Some(features) = &supplement.supported_features {
        if !features.is_empty() {
            base.supported_features = Some(features.clone());
        }
    }

    if base
        .description
        .as_ref()
        .map(|desc| desc.trim().is_empty())
        .unwrap_or(true)
    {
        if let Some(desc) = &supplement.description {
            if !desc.trim().is_empty() {
                base.description = Some(desc.clone());
            }
        }
    }

    if let Some(url) = &supplement.url {
        if !url.trim().is_empty() {
            base.url = Some(url.clone());
        }
    }

    if let Some(input_modalities) = &supplement.input_modalities {
        if !input_modalities.is_empty() {
            base.input_modalities = Some(input_modalities.clone());
        }
    }

    if let Some(output_modalities) = &supplement.output_modalities {
        if !output_modalities.is_empty() {
            base.output_modalities = Some(output_modalities.clone());
        }
    }

    if supplement.metadata.is_some() {
        base.metadata = supplement.metadata.clone();
    }

    if supplement.default_parameters.is_some() {
        base.default_parameters = supplement.default_parameters.clone();
    }

    if supplement.max_parameters.is_some() {
        base.max_parameters = supplement.max_parameters.clone();
    }

    // 处理 supported_methods
    if let Some(methods) = &supplement.supported_methods {
        if !methods.is_empty() {
            base.supported_methods = Some(methods.clone());
        }
    } else {
        // 从 metadata 中提取 endpoints 并转换
        if let Some(metadata) = &supplement.metadata {
            if let Some(endpoints) = metadata.get("endpoints").and_then(|v| v.as_array()) {
                let endpoint_strings: Vec<String> = endpoints
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();

                if !endpoint_strings.is_empty() {
                    let prefix = provider_type;
                    let methods = convert_endpoints_to_methods(&endpoint_strings, prefix);
                    base.supported_methods = Some(methods);
                }
            }
        }
    }
}
