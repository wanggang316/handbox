use crate::types::{LlmApiType, LlmModelApiType, SupplementField};

#[derive(Debug, Clone)]
pub struct LlmProviderConfig {
    pub provider_type: String,
    pub chat_api_type: LlmApiType,
    pub model_api_type: LlmModelApiType,
    pub supplement_file: Option<String>,
    /// 定义要从 supplement 合并哪些字段，空列表表示合并所有字段
    pub supplement_fields: Option<Vec<SupplementField>>,
}

pub trait LlmConfigProvider: Send + Sync {
    fn get_provider_config(&self, provider_type: &str) -> Option<LlmProviderConfig>;
}
