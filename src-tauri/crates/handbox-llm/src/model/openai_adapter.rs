// OpenAI 模型客户端实现

use super::model_client::ModelClient;
use super::oss_client::OssClient;
use crate::error::LlmClientError;
use crate::types::{
    extract_pricing_value, merge_pricing, LlmModel, LlmModelModality, LlmProvider,
    ModelSupplementDocument,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::to_value;
use std::collections::HashMap;

const OPENAI_SUPPLEMENT_OBJECT_KEY: &str = "openai_models.json";

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

        let supplement_map = load_openai_model_supplements().await;

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
                support_parameters: Vec::new(),
                default_parameters: None,
                max_parameters: None,
            };

            if let Some(map) = &supplement_map {
                if let Some(supplement) = map.get(&model.id) {
                    merge_openai_model(&mut model, supplement);
                }
            }

            result_models.push(model);
        }

        Ok(result_models)
    }
}

async fn load_openai_model_supplements() -> Option<HashMap<String, LlmModel>> {
    let client = match OssClient::from_env() {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!(
                "Failed to load OSS configuration for OpenAI supplements: {}",
                err
            );
            return None;
        }
    };

    let content = match client.get_object_text(OPENAI_SUPPLEMENT_OBJECT_KEY).await {
        Ok(text) => text,
        Err(err) => {
            tracing::warn!(
                "Failed to download OpenAI supplement models from OSS: {}",
                err
            );
            return None;
        }
    };

    let document: ModelSupplementDocument = match serde_json::from_str(&content) {
        Ok(doc) => doc,
        Err(err) => {
            tracing::warn!(
                "Failed to parse OpenAI supplement models from OSS JSON: {}",
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

fn merge_openai_model(base: &mut LlmModel, supplement: &LlmModel) {
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
}
