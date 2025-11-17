// Anthropic 模型适配器实现

use super::model_client::ModelFetcher;
use crate::error::LlmClientError;
use crate::types::{LlmModel, LlmProvider};
use async_trait::async_trait;

/// Anthropic 模型数据获取器（没有公开 API）
pub struct AnthropicFetcher;

#[async_trait]
impl ModelFetcher for AnthropicFetcher {
    /// Anthropic 没有公开的模型列表 API，返回空 Vec
    async fn fetch_base_models(
        &self,
        _provider: &LlmProvider,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        Ok(Vec::new())
    }
}
