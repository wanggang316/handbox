// 前端 LLM 配置相关数据模型

use serde::{Deserialize, Serialize};

/// 供应商配置选项（供前端使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: String,
    pub type_name: String,
    pub default_name: String,
    pub default_base_url: String,
    pub icon: String,
    pub chat_api_type: String,
    pub model_api_type: String,
    pub description: Option<String>,
}

/// 前端供应商配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigsResponse {
    pub providers: Vec<ProviderConfig>,
    pub custom_providers: Vec<ProviderConfig>,
}
