use crate::types::{LlmApiType, LlmModelApiType, LlmModelFeature, LlmModelModality};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct LlmProviderConfig {
    pub provider_type: String,
    pub chat_api_type: LlmApiType,
    pub model_api_type: LlmModelApiType,
    pub model_local: Option<HashMap<String, LlmModelExtraInfo>>,
}

#[derive(Debug, Clone)]
pub struct LlmModelExtraInfo {
    pub name: String,
    pub context_length: Option<i32>,
    pub output_max_tokens: Option<i32>,
    pub input_cost_per_1k: Option<f32>,
    pub output_cost_per_1k: Option<f32>,
    pub features: Vec<LlmModelFeature>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<LlmModelModality>>,
    pub output_modalities: Option<Vec<LlmModelModality>>,
    pub metadata: Option<Value>,
    pub pricing: Option<Value>,
}

pub trait LlmConfigProvider: Send + Sync {
    fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig>;

    fn get_model_extra_info(
        &self,
        provider_type: &str,
        model_id: &str,
    ) -> Option<LlmModelExtraInfo>;
}
