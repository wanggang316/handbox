// 模型相关数据模型

use crate::storage::types::{Model, Timestamp, UUID};
use serde::{Deserialize, Serialize};

/// 模型列表请求
#[derive(Debug, Clone, Deserialize)]
pub struct ListModelsRequest {
    pub provider_id: UUID,
    pub refresh_from_remote: Option<bool>,
}

/// 模型列表响应
#[derive(Debug, Clone, Serialize)]
pub struct ListModelsResponse {
    pub models: Vec<Model>,
    pub cached: bool,
    pub timestamp: Timestamp,
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
