// 供应商相关数据模型

use super::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// 供应商类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ProviderType {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "google")]
    Google,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "openrouter")]
    OpenRouter,
    #[serde(rename = "custom-openai")]
    CustomOpenAI,
    #[serde(rename = "custom-anthropic")]
    CustomAnthropic,
}



/// 模型特性
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[serde(rename_all = "kebab-case")]
#[sqlx(type_name = "TEXT", rename_all = "kebab-case")]
pub enum ModelFeature {
    Text,
    Vision,
    FunctionCalling,
    Streaming,
    Reasoning,
}



/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub input_cost: Option<f32>,
    pub output_cost: Option<f32>,
    pub supported_features: Vec<ModelFeature>,
    pub enabled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

// 为 Model 的 supported_features 字段提供序列化支持
impl Model {
    pub fn features_to_json(&self) -> String {
        serde_json::to_string(&self.supported_features).unwrap_or_default()
    }
    
    pub fn features_from_json(json: &str) -> Result<Vec<ModelFeature>, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// 供应商实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: UUID,
    pub name: String,
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}



/// 带有模型的供应商实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderWithModels {
    pub id: UUID,
    pub name: String,
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub models: Vec<Model>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 供应商配置请求
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: Option<String>,
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: String,
    pub enabled: Option<bool>,
}

/// 模型列表请求
#[derive(Debug, Clone, Deserialize)]
pub struct ListModelsRequest {
    pub provider_id: UUID,
    pub force_refresh: Option<bool>,
}

/// 模型列表响应
#[derive(Debug, Clone, Serialize)]
pub struct ListModelsResponse {
    pub models: Vec<Model>,
    pub cached: bool,
    pub timestamp: Timestamp,
}

/// 供应商切换请求
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleProviderRequest {
    pub provider_id: UUID,
    pub enabled: bool,
}

/// 模型切换请求
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleModelRequest {
    pub provider_id: UUID,
    pub model_id: String,
    pub enabled: bool,
}

// === API 响应结构 ===

/// OpenAI 模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIModelsResponse {
    pub object: String,
    pub data: Vec<OpenAIModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIModel {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

/// Google AI 模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleModelsResponse {
    pub models: Vec<GoogleModel>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleModel {
    pub name: String,
    pub version: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub description: Option<String>,
    #[serde(rename = "inputTokenLimit")]
    pub input_token_limit: Option<i32>,
    #[serde(rename = "outputTokenLimit")]
    pub output_token_limit: Option<i32>,
    #[serde(rename = "supportedGenerationMethods")]
    pub supported_generation_methods: Option<Vec<String>>,
    pub temperature: Option<f32>,
    #[serde(rename = "topP")]
    pub top_p: Option<f32>,
    #[serde(rename = "topK")]
    pub top_k: Option<i32>,
    #[serde(rename = "maxTemperature")]
    pub max_temperature: Option<f32>,
    pub thinking: Option<bool>,
}

/// DeepSeek 模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct DeepSeekModelsResponse {
    pub object: String,
    pub data: Vec<DeepSeekModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeepSeekModel {
    pub id: String,
    pub object: String,
    pub owned_by: String,
}

/// OpenRouter 模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelsResponse {
    pub data: Vec<OpenRouterModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub pricing: Option<OpenRouterPricing>,
    #[serde(rename = "context_length")]
    pub context_length: Option<i32>,
    pub architecture: Option<OpenRouterArchitecture>,
    #[serde(rename = "top_provider")]
    pub top_provider: Option<OpenRouterProvider>,
    #[serde(rename = "per_request_limits")]
    pub per_request_limits: Option<OpenRouterLimits>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterPricing {
    pub prompt: Option<String>,
    pub completion: Option<String>,
    pub image: Option<String>,
    pub request: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterArchitecture {
    pub modality: Option<String>,
    pub tokenizer: Option<String>,
    #[serde(rename = "instruct_type")]
    pub instruct_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterProvider {
    #[serde(rename = "max_completion_tokens")]
    pub max_completion_tokens: Option<i32>,
    #[serde(rename = "is_moderated")]
    pub is_moderated: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterLimits {
    #[serde(rename = "prompt_tokens")]
    pub prompt_tokens: Option<String>,
    #[serde(rename = "completion_tokens")]
    pub completion_tokens: Option<String>,
}
