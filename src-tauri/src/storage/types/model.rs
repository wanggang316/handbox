use std::collections::HashMap;

use super::common::Timestamp;
use handbox_llm::types::{LlmModelParameter, ModelPricing};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 模态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ModelModality {
    Text,
    #[serde(alias = "images")]
    Image,
    Pdf,
    File,
    Audio,
    Video,
}

/// 模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub output_max_tokens: Option<i32>,
    pub supported_features: Option<Vec<String>>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<ModelModality>>,
    pub output_modalities: Option<Vec<ModelModality>>,
    pub metadata: Option<Value>,
    pub pricing: Option<ModelPricing>,
    pub url: Option<String>,
    pub support_parameters: Option<Vec<LlmModelParameter>>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
    pub supported_methods: Option<Vec<String>>,
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

    pub fn features_from_json(json: &str) -> Result<Option<Vec<String>>, serde_json::Error> {
        if json.is_empty() {
            return Ok(None);
        }

        let features: Vec<String> = serde_json::from_str(json)?;
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
        self.pricing
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok())
    }

    pub fn pricing_from_json(
        json: Option<&str>,
    ) -> Result<Option<ModelPricing>, serde_json::Error> {
        match json {
            Some(data) if !data.trim().is_empty() => {
                serde_json::from_str::<ModelPricing>(data).map(Some)
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

    pub fn support_parameters_to_json(&self) -> Option<String> {
        self.support_parameters.as_ref().and_then(|params| {
            if params.is_empty() {
                None
            } else {
                serde_json::to_string(params).ok()
            }
        })
    }

    pub fn support_parameters_from_json(
        json: Option<&str>,
    ) -> Result<Option<Vec<LlmModelParameter>>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let params = serde_json::from_str::<Vec<LlmModelParameter>>(data)?;
                if params.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(params))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn default_parameters_to_json(&self) -> Option<String> {
        self.default_parameters.as_ref().and_then(|params| {
            if params.is_empty() {
                None
            } else {
                serde_json::to_string(params).ok()
            }
        })
    }

    pub fn default_parameters_from_json(
        json: Option<&str>,
    ) -> Result<Option<HashMap<String, Value>>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let params = serde_json::from_str::<HashMap<String, Value>>(data)?;
                if params.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(params))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn max_parameters_to_json(&self) -> Option<String> {
        self.max_parameters.as_ref().and_then(|params| {
            if params.is_empty() {
                None
            } else {
                serde_json::to_string(params).ok()
            }
        })
    }

    pub fn max_parameters_from_json(
        json: Option<&str>,
    ) -> Result<Option<HashMap<String, Value>>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let params = serde_json::from_str::<HashMap<String, Value>>(data)?;
                if params.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(params))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn supported_methods_to_json(&self) -> Option<String> {
        self.supported_methods.as_ref().and_then(|methods| {
            if methods.is_empty() {
                None
            } else {
                serde_json::to_string(methods).ok()
            }
        })
    }

    pub fn supported_methods_from_json(
        json: Option<&str>,
    ) -> Result<Option<Vec<String>>, serde_json::Error> {
        match json {
            Some(data) if !data.is_empty() => {
                let methods = serde_json::from_str::<Vec<String>>(data)?;
                if methods.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(methods))
                }
            }
            _ => Ok(None),
        }
    }
}
