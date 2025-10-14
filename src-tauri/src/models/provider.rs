// 供应商相关数据模型

use crate::storage::types::UUID;
use serde::Deserialize;

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
