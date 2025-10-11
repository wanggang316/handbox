use super::common::{Timestamp, UUID};
use serde::{Deserialize, Serialize};

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
    pub supported_features: Option<Vec<ModelFeature>>,
    pub enabled: bool,
    pub favorite: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Model {
    pub fn features_to_json(&self) -> String {
        serde_json::to_string(&self.supported_features.as_ref().unwrap_or(&vec![]))
            .unwrap_or_default()
    }

    pub fn features_from_json(json: &str) -> Result<Option<Vec<ModelFeature>>, serde_json::Error> {
        if json.is_empty() {
            Ok(None)
        } else {
            serde_json::from_str::<Vec<ModelFeature>>(json).map(Some)
        }
    }
}

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
