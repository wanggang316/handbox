// 供应商相关数据模型

use super::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// 供应商类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[serde(rename_all = "kebab-case")]
#[sqlx(type_name = "TEXT", rename_all = "kebab-case")]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum ProviderStatus {
    Enabled,
    Disabled,
    Idle,
    Error,
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
    pub status: ProviderStatus,
    pub enabled: bool,
    pub last_probe_at: Option<Timestamp>,
    pub probe_result: Option<ProbeResult>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

// 为 Provider 的 probe_result 字段提供序列化支持  
impl Provider {
    pub fn probe_result_to_json(&self) -> Option<String> {
        self.probe_result.as_ref().map(|pr| serde_json::to_string(pr).unwrap_or_default())
    }
    
    pub fn probe_result_from_json(json: Option<String>) -> Result<Option<ProbeResult>, serde_json::Error> {
        match json {
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
            None => Ok(None),
        }
    }
}

/// 带有模型的供应商实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderWithModels {
    pub id: UUID,
    pub name: String,
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: String,
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
