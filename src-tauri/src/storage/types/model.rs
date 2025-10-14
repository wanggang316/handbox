use super::common::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 模型特性
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[serde(rename_all = "kebab-case")]
#[sqlx(type_name = "TEXT", rename_all = "kebab-case")]
pub enum ModelFeature {
    Reasoning,
    Tool,
}

/// 模态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ModelModality {
    Text,
    Image,
    File,
    Audio,
    Video,
}

/// 模型参数配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelParameter {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Value>,
}

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub output_token_limit: Option<i32>,
    pub input_cost: Option<f32>,
    pub output_cost: Option<f32>,
    pub supported_features: Option<Vec<ModelFeature>>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<ModelModality>>,
    pub output_modalities: Option<Vec<ModelModality>>,
    pub metadata: Option<Value>,
    pub pricing: Option<Value>,
    pub parameters: Option<Vec<ModelParameter>>,
    pub enabled: bool,
    pub favorite: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Model {
    pub fn features_to_json(&self) -> String {
        serde_json::to_string(&self.supported_features.as_ref().unwrap_or(&vec![]))
            .unwrap_or_else(|_| "[]".to_string())
    }

    pub fn features_from_json(json: &str) -> Result<Option<Vec<ModelFeature>>, serde_json::Error> {
        if json.is_empty() {
            return Ok(None);
        }

        let raw_features: Vec<String> = serde_json::from_str(json)?;
        let mut features = Vec::new();

        for feature in raw_features {
            match feature.as_str() {
                "reasoning" => features.push(ModelFeature::Reasoning),
                "tool" | "tools" | "function-calling" | "function_calling" => {
                    features.push(ModelFeature::Tool)
                }
                _ => {}
            }
        }

        if features.is_empty() {
            Ok(None)
        } else {
            Ok(Some(features))
        }
    }

    pub fn metadata_to_json(&self) -> Option<String> {
        self.metadata.as_ref().and_then(|value| {
            if value.is_null() {
                None
            } else {
                Some(value.to_string())
            }
        })
    }

    pub fn metadata_from_json(json: Option<&str>) -> Result<Option<Value>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let value = serde_json::from_str::<Value>(data)?;
                if value.is_null() {
                    Ok(None)
                } else {
                    Ok(Some(value))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn pricing_to_json(&self) -> Option<String> {
        self.pricing.as_ref().and_then(|value| {
            if value.is_null() {
                None
            } else {
                Some(value.to_string())
            }
        })
    }

    pub fn pricing_from_json(json: Option<&str>) -> Result<Option<Value>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let value = serde_json::from_str::<Value>(data)?;
                if value.is_null() {
                    Ok(None)
                } else {
                    Ok(Some(value))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn modalities_to_json(modalities: &Option<Vec<ModelModality>>) -> Option<String> {
        modalities
            .as_ref()
            .and_then(|items| serde_json::to_string(items).ok())
    }

    pub fn modalities_from_json(
        json: Option<&str>,
    ) -> Result<Option<Vec<ModelModality>>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                serde_json::from_str::<Vec<ModelModality>>(data).map(Some)
            }
            _ => Ok(None),
        }
    }

    pub fn parameters_to_json(&self) -> Option<String> {
        self.parameters.as_ref().and_then(|params| {
            if params.is_empty() {
                None
            } else {
                serde_json::to_string(params).ok()
            }
        })
    }

    pub fn parameters_from_json(
        json: Option<&str>,
    ) -> Result<Option<Vec<ModelParameter>>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let params = serde_json::from_str::<Vec<ModelParameter>>(data)?;
                if params.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(params))
                }
            }
            _ => Ok(None),
        }
    }
}
