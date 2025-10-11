// 供应商相关数据模型

use crate::storage::types::{Model, Timestamp, UUID};
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

/// 模型收藏切换请求
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleModelFavoriteRequest {
    pub provider_id: UUID,
    pub model_id: String,
    pub favorite: bool,
}
