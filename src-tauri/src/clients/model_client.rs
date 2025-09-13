// 模型列表提供者抽象层
// 根据不同的 model_list_api_type 动态获取模型列表

use super::llm_client::StandardModel;
use crate::models::{AppError, ModelFeature, Provider};
use crate::services::llm_config::{get_global_llm_config, ModelExtraInfo};
use async_trait::async_trait;
use serde::Deserialize;

/// 模型客户端 trait
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn list_models(
        &self,
        provider: &Provider,
        provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError>;
}

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
                supported_features: Some(vec![ModelFeature::Text]),
            });
        }

        Ok(result_models)
    }
}

/// OpenAI + Local 增强模型客户端
pub struct OpenAIWithLocalProvider {
    openai_provider: OpenAIModelClient,
}

impl OpenAIWithLocalProvider {
    pub fn new() -> Self {
        Self {
            openai_provider: OpenAIModelClient::new(),
        }
    }

    fn enhance_with_local_info(
        &self,
        mut models: Vec<StandardModel>,
        provider_type: &str,
    ) -> Vec<StandardModel> {
        let config = get_global_llm_config();

        for model in &mut models {
            if let Some(extra_info) = config.get_model_extra_info(provider_type, &model.id) {
                *model = self.convert_model_extra_info(&model.id, extra_info);
            }
        }

        models
    }

    fn convert_model_extra_info(
        &self,
        model_id: &str,
        extra_info: &ModelExtraInfo,
    ) -> StandardModel {
        let config = get_global_llm_config();
        StandardModel {
            id: model_id.to_string(),
            name: extra_info.name.clone(),
            context_length: extra_info.context_length,
            input_cost: extra_info.input_cost_per_1k,
            output_cost: extra_info.output_cost_per_1k,
            supported_features: Some(config.convert_features(&extra_info.features)),
        }
    }
}

#[async_trait]
impl ModelClient for OpenAIWithLocalProvider {
    async fn list_models(
        &self,
        provider: &Provider,
        provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError> {
        let models = self
            .openai_provider
            .list_models(provider, provider_type)
            .await?;
        Ok(self.enhance_with_local_info(models, provider_type))
    }
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
                supported_features: Some(vec![ModelFeature::Text]),
            });
        }

        Ok(result_models)
    }
}

/// Anthropic 模型客户端（基于本地配置）
pub struct AnthropicModelClient;

impl AnthropicModelClient {
    pub fn new() -> Self {
        Self
    }

    fn convert_model_extra_info(
        &self,
        model_id: &str,
        extra_info: &ModelExtraInfo,
    ) -> StandardModel {
        let config = get_global_llm_config();
        StandardModel {
            id: model_id.to_string(),
            name: extra_info.name.clone(),
            context_length: extra_info.context_length,
            input_cost: extra_info.input_cost_per_1k,
            output_cost: extra_info.output_cost_per_1k,
            supported_features: Some(config.convert_features(&extra_info.features)),
        }
    }
}

#[async_trait]
impl ModelClient for AnthropicModelClient {
    async fn list_models(
        &self,
        _provider: &Provider,
        provider_type: &str,
    ) -> Result<Vec<StandardModel>, AppError> {
        // Anthropic 不提供公开的模型列表 API，返回预定义的模型列表
        let config = get_global_llm_config();
        let provider_config = config.get_provider_config(provider_type);

        if let Some(config) = provider_config {
            if let Some(local_models) = &config.model_local {
                let mut result_models = Vec::new();
                for (model_id, model_info) in local_models {
                    result_models.push(self.convert_model_extra_info(model_id, model_info));
                }
                return Ok(result_models);
            }
        }

        Ok(vec![])
    }
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
                supported_features: Some(vec![ModelFeature::Text]),
            });
        }

        Ok(result_models)
    }
}

/// 模型客户端工厂
pub fn create_model_client(api_type: &str) -> Result<Box<dyn ModelClient>, AppError> {
    match api_type {
        "openai" => Ok(Box::new(OpenAIModelClient::new())),
        "openai+local" => Ok(Box::new(OpenAIWithLocalProvider::new())),
        "google" => Ok(Box::new(GoogleModelClient::new())),
        "anthropic" => Ok(Box::new(AnthropicModelClient::new())),
        "openrouter" => Ok(Box::new(OpenRouterModelClient::new())),
        _ => Err(AppError::validation_error(&format!(
            "Unsupported model list API type: {}",
            api_type
        ))),
    }
}
