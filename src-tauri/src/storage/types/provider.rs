use super::common::{Timestamp, UUID};
use super::model::Model;
use serde::{Deserialize, Serialize};

/// 供应商实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: UUID,
    pub name: String,
    pub provider_type: String,
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
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub models: Vec<Model>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}
