// 前端 LLM 配置相关数据模型

use serde::{Deserialize, Serialize};

/// 前端供应商配置选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendProviderConfig {
    pub provider_type: String,
    pub type_name: String,
    pub default_name: String,
    pub default_base_url: String,
    pub icon: String,
    pub api_type: String,
    pub model_list_api_type: String,
    pub description: Option<String>,
}

/// 前端供应商配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigsResponse {
    pub providers: Vec<FrontendProviderConfig>,
    pub custom_providers: Vec<FrontendProviderConfig>,
}