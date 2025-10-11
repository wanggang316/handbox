// 聊天相关数据模型

use serde::{Deserialize, Serialize};

/// 模型参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelParameters {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub context_length: Option<i32>,
    pub stream: Option<bool>,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            context_length: Some(4096),
            stream: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_parameters_default_values() {
        let params = ModelParameters::default();

        assert_eq!(params.temperature, Some(0.7));
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.max_tokens, Some(2048));
        assert_eq!(params.context_length, Some(4096));
        assert_eq!(params.stream, Some(true));
    }
}
