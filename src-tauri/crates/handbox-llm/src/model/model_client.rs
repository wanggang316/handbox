// 模型客户端定义和工厂函数

use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::model::anthropic_adapter::AnthropicFetcher;
use crate::model::google_adapter::GoogleFetcher;
use crate::model::openai_adapter::OpenAIFetcher;
use crate::model::openrouter_adapter::OpenRouterFetcher;
use crate::model::supplement::{OssSupplementProvider, SupplementProvider};
use crate::types::{merge_supplement, LlmModel, LlmModelApiType, LlmProvider, SupplementField};
use async_trait::async_trait;
use std::sync::Arc;

/// 模型数据获取 trait - 从 API 获取基础模型数据
#[async_trait]
pub trait ModelFetcher: Send + Sync {
    /// 从提供商 API 获取基础模型列表
    /// 对于没有公开 API 的提供商（如 Anthropic），返回空 Vec
    async fn fetch_base_models(
        &self,
        provider: &LlmProvider,
    ) -> Result<Vec<LlmModel>, LlmClientError>;
}

/// 模型客户端 - 具体结构体，负责编排 fetch + supplement + merge 流程
pub struct ModelClient {
    fetcher: Box<dyn ModelFetcher>,
    supplement_provider: Option<OssSupplementProvider>,
    supplement_fields: Option<Vec<SupplementField>>,
}

impl ModelClient {
    /// 创建新的 ModelClient 实例
    pub fn new(
        fetcher: Box<dyn ModelFetcher>,
        config: Arc<dyn LlmConfigProvider>,
        provider_type: &str,
    ) -> Self {
        // 从配置中获取 supplement_fields
        let supplement_fields = config
            .get_provider_config(provider_type)
            .and_then(|c| c.supplement_fields);

        Self {
            fetcher,
            supplement_provider: Some(OssSupplementProvider::new(config)),
            supplement_fields,
        }
    }

    /// 获取模型列表的完整流程
    /// 编排：fetch API models → load supplements → merge
    pub async fn list_models(
        &self,
        provider: &LlmProvider,
        provider_type: &str,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        // Phase 1: 从 API 获取基础模型
        let base_models = self.fetcher.fetch_base_models(provider).await?;

        // Phase 2: 加载 supplement 数据
        let supplement_map = if let Some(ref provider) = self.supplement_provider {
            provider.load_supplements(provider_type).await?
        } else {
            None
        };

        // Phase 3: 合并数据
        let mut result = Vec::new();

        if let Some(supplements) = supplement_map {
            // 获取 supplement_fields，如果没有则使用空列表（合并所有字段）
            let fields = self.supplement_fields.as_deref().unwrap_or(&[]);

            // 对于 API 返回的模型，尝试合并 supplement
            for base_model in base_models {
                if let Some(supplement) = supplements.get(&base_model.id) {
                    result.push(merge_supplement(
                        base_model,
                        supplement,
                        fields,
                        provider_type,
                    ));
                } else {
                    // 没有 supplement 的模型，直接使用 API 数据
                    result.push(base_model);
                }
            }
        } else {
            // 没有 supplement 的情况，直接返回 API 数据
            result = base_models;
        }

        Ok(result)
    }
}

/// 模型客户端工厂函数
pub fn create_model_client(
    api_type: LlmModelApiType,
    provider_type: &str,
    config: Arc<dyn LlmConfigProvider>,
) -> Result<ModelClient, LlmClientError> {
    let fetcher: Box<dyn ModelFetcher> = match api_type {
        LlmModelApiType::OpenAI => Box::new(OpenAIFetcher::new()),
        LlmModelApiType::Google => Box::new(GoogleFetcher::new()),
        LlmModelApiType::Anthropic => Box::new(AnthropicFetcher),
        LlmModelApiType::OpenRouter => Box::new(OpenRouterFetcher::new()),
    };

    Ok(ModelClient::new(fetcher, config, provider_type))
}
