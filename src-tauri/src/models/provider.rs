// 供应商相关数据模型

use super::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// 供应商类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
    DeepSeek,
    OpenRouter,
    CustomOpenAI,
    CustomAnthropic,
}

/// 供应商状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderStatus {
    Active,
    Inactive,
    Error,
    Testing,
}

/// 模型特性
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelFeature {
    Text,
    Vision,
    FunctionCalling,
    Streaming,
    Reasoning,
}

/// 探活结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub success: bool,
    pub latency: Option<i64>,
    pub error: Option<String>,
    pub timestamp: Timestamp,
}

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub context_length: Option<i32>,
    pub input_cost: Option<f32>,
    pub output_cost: Option<f32>,
    pub supported_features: Vec<ModelFeature>,
    pub enabled: bool,
}

/// 供应商实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: UUID,
    pub name: String,
    pub provider_type: ProviderType,
    pub base_url: String,
    pub status: ProviderStatus,
    pub enabled: bool,
    pub models: Vec<Model>,
    pub last_probe_at: Option<Timestamp>,
    pub probe_result: Option<ProbeResult>,
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
