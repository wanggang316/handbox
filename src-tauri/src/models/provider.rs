// 供应商相关数据模型

use crate::models::ModelResponse;
use crate::storage::types::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// 添加供应商请求
#[derive(Debug, Clone, Deserialize)]
pub struct AddProviderRequest {
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: Option<bool>,
}

/// 供应商切换请求
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleProviderRequest {
    pub provider_id: UUID,
    pub enabled: bool,
}

/// 带有模型的供应商响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderWithModels {
    pub id: UUID,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub models: Vec<ModelResponse>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
