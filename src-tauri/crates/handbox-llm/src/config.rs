use crate::types::{LlmApiType, LlmModelApiType};

#[derive(Debug, Clone)]
pub struct LlmProviderConfig {
    pub provider_type: String,
    pub chat_api_type: LlmApiType,
    pub model_api_type: LlmModelApiType,
    pub supplement_file: Option<String>,
}

pub trait LlmConfigProvider: Send + Sync {
    fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig>;
}
