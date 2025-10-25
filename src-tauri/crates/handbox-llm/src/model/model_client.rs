// 模型客户端定义和工厂函数

use crate::config::LlmConfigProvider;
use crate::error::LlmClientError;
use crate::model::anthropic_adapter::{AnthropicFetcher, AnthropicSupplementer};
use crate::model::google_adapter::{GoogleFetcher, GoogleSupplementer};
use crate::model::openai_adapter::{OpenAIFetcher, OpenAISupplementer};
use crate::model::openrouter_adapter::{OpenRouterFetcher, OpenRouterSupplementer};
use crate::model::supplement::{OssSupplementProvider, SupplementProvider};
use crate::types::{LlmModel, LlmModelApiType, LlmProvider, ModelSupplement};
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

/// 模型补充 trait - 将 supplement 数据合并到模型中
pub trait ModelSupplementer: Send + Sync {
    /// 将 supplement 数据合并到 LlmModel 中
    /// 不同 adapter 有不同的合并策略：
    /// - OpenAI: 优先使用 supplement 数据（API 返回的信息很少）
    /// - Google: 只填充缺失字段（API 返回的信息很丰富）
    /// - Anthropic: 从 supplement 构建完整模型（没有 API）
    ///
    /// 返回 Vec 是因为一个 supplement 可能展开为多个模型（snapshots）
    fn merge_supplement(&self, model: LlmModel, supplement: &ModelSupplement) -> Vec<LlmModel>;
}

/// No-op 补充器 - 当没有 supplement 文件时使用
struct NoOpSupplementer;

impl ModelSupplementer for NoOpSupplementer {
    fn merge_supplement(&self, model: LlmModel, _supplement: &ModelSupplement) -> Vec<LlmModel> {
        vec![model]
    }
}

/// 模型客户端 - 具体结构体，负责编排 fetch + supplement + merge 流程
pub struct ModelClient {
    fetcher: Box<dyn ModelFetcher>,
    supplementer: Box<dyn ModelSupplementer>,
    supplement_provider: Option<OssSupplementProvider>,
}

impl ModelClient {
    /// 创建新的 ModelClient 实例（带 supplement provider）
    pub fn new(
        fetcher: Box<dyn ModelFetcher>,
        supplementer: Box<dyn ModelSupplementer>,
        config: Arc<dyn LlmConfigProvider>,
    ) -> Self {
        Self {
            fetcher,
            supplementer,
            supplement_provider: Some(OssSupplementProvider::new(config)),
        }
    }

    /// 创建新的 ModelClient 实例（不使用 supplement）
    pub fn new_without_supplement(fetcher: Box<dyn ModelFetcher>) -> Self {
        Self {
            fetcher,
            supplementer: Box::new(NoOpSupplementer),
            supplement_provider: None,
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
            // 用于跟踪哪些 supplement 已经被使用
            let mut used_supplement_ids = std::collections::HashSet::new();

            // 3a. 对于 API 返回的模型，尝试合并 supplement
            for base_model in base_models {
                if let Some(supplement) = supplements.get(&base_model.id) {
                    used_supplement_ids.insert(base_model.id.clone());
                    result.extend(self.supplementer.merge_supplement(base_model, supplement));
                } else {
                    // 没有 supplement 的模型，直接使用 API 数据
                    result.push(base_model);
                }
            }

            // 3b. 对于没有匹配 API 模型的 supplement（如 Anthropic），单独处理
            for (id, supplement) in supplements {
                if !used_supplement_ids.contains(&id) {
                    // 创建一个空的 LlmModel 作为 base
                    let empty_base = LlmModel {
                        id: id.clone(),
                        name: id.clone(),
                        context_length: None,
                        output_max_tokens: None,
                        supported_features: None,
                        description: None,
                        input_modalities: None,
                        output_modalities: None,
                        metadata: None,
                        pricing: None,
                        url: None,
                        support_parameters: Vec::new(),
                        default_parameters: None,
                        max_parameters: None,
                        supported_methods: None,
                    };
                    result.extend(self.supplementer.merge_supplement(empty_base, &supplement));
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
    // 检查是否有 supplement_file 配置
    let has_supplement = config
        .get_provider_config(provider_type)
        .and_then(|c| c.supplement_file)
        .filter(|s| !s.is_empty())
        .is_some();

    let client = match api_type {
        LlmModelApiType::OpenAI => {
            let fetcher = Box::new(OpenAIFetcher::new()) as Box<dyn ModelFetcher>;
            if has_supplement {
                let supplementer = Box::new(OpenAISupplementer) as Box<dyn ModelSupplementer>;
                ModelClient::new(fetcher, supplementer, config)
            } else {
                ModelClient::new_without_supplement(fetcher)
            }
        }
        LlmModelApiType::Google => {
            let fetcher = Box::new(GoogleFetcher::new()) as Box<dyn ModelFetcher>;
            if has_supplement {
                let supplementer = Box::new(GoogleSupplementer) as Box<dyn ModelSupplementer>;
                ModelClient::new(fetcher, supplementer, config)
            } else {
                ModelClient::new_without_supplement(fetcher)
            }
        }
        LlmModelApiType::Anthropic => {
            let fetcher = Box::new(AnthropicFetcher) as Box<dyn ModelFetcher>;
            if has_supplement {
                let supplementer = Box::new(AnthropicSupplementer) as Box<dyn ModelSupplementer>;
                ModelClient::new(fetcher, supplementer, config)
            } else {
                ModelClient::new_without_supplement(fetcher)
            }
        }
        LlmModelApiType::OpenRouter => {
            let fetcher = Box::new(OpenRouterFetcher::new()) as Box<dyn ModelFetcher>;
            if has_supplement {
                let supplementer = Box::new(OpenRouterSupplementer) as Box<dyn ModelSupplementer>;
                ModelClient::new(fetcher, supplementer, config)
            } else {
                ModelClient::new_without_supplement(fetcher)
            }
        }
    };

    Ok(client)
}
