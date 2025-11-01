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
use regex::Regex;
use std::sync::Arc;

/// 辅助函数：移除模型 ID 中的版本后缀
/// 匹配并移除以下后缀：
/// - `-preview`
/// - `-preview-数字` (至少1个数字，如 -preview-1234)
/// - `-preview-数字-数字` (如 -preview-09-2025)
/// - `-preview-数字-数字-数字` (支持更多段，如 -preview-1-2-3)
/// - `-exp`
/// - `-exp-数字` (至少1个数字)
/// - `-exp-数字-数字` (如 -exp-09-2025)
/// - `-latest`
fn strip_model_version_suffix(model_id: &str) -> Option<String> {
    // 匹配 -preview 或 -exp 后跟可选的一个或多个 -数字 段
    // (?:-\d+)+ 表示匹配一个或多个 "-数字" 段
    let pattern = r"(-preview(?:-\d+)*|-exp(?:-\d+)*|-latest)$";
    let re = Regex::new(pattern).ok()?;

    if re.is_match(model_id) {
        Some(re.replace(model_id, "").to_string())
    } else {
        None
    }
}

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
                    // 直接找到 supplement，进行合并
                    result.push(merge_supplement(
                        base_model,
                        supplement,
                        fields,
                        provider_type,
                    ));
                } else if let Some(stripped_id) = strip_model_version_suffix(&base_model.id) {
                    // 尝试去掉版本后缀后再查找
                    if let Some(supplement) = supplements.get(&stripped_id) {
                        result.push(merge_supplement(
                            base_model,
                            supplement,
                            fields,
                            provider_type,
                        ));
                    } else {
                        // 去掉后缀后仍找不到，使用 API 数据
                        result.push(base_model);
                    }
                } else {
                    // 没有版本后缀且找不到 supplement，直接使用 API 数据
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_model_version_suffix() {
        // 测试 -preview 后缀
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview"),
            Some("gpt-4".to_string())
        );

        // 测试 -preview-数字 后缀
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-1234"),
            Some("gpt-4".to_string())
        );

        // 测试 -preview-日期 后缀 (YYYYMMDD)
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-20240101"),
            Some("gpt-4".to_string())
        );

        // 测试 -preview-月-年 格式 (MM-YYYY)
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-09-2025"),
            Some("gpt-4".to_string())
        );

        // 测试 -preview-多段数字格式
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-1-2-3"),
            Some("gpt-4".to_string())
        );

        // 测试 -exp 后缀
        assert_eq!(
            strip_model_version_suffix("claude-3-exp"),
            Some("claude-3".to_string())
        );

        // 测试 -exp-数字 后缀
        assert_eq!(
            strip_model_version_suffix("claude-3-exp-5678"),
            Some("claude-3".to_string())
        );

        // 测试 -exp-多段数字格式
        assert_eq!(
            strip_model_version_suffix("claude-3-exp-01-2025"),
            Some("claude-3".to_string())
        );

        // 测试 -latest 后缀
        assert_eq!(
            strip_model_version_suffix("gpt-4-latest"),
            Some("gpt-4".to_string())
        );

        // 测试没有后缀的情况
        assert_eq!(strip_model_version_suffix("gpt-4-turbo"), None);

        // 测试普通模型名称
        assert_eq!(strip_model_version_suffix("gpt-4"), None);

        // 测试复杂的模型名称
        assert_eq!(
            strip_model_version_suffix("gpt-4-turbo-preview"),
            Some("gpt-4-turbo".to_string())
        );

        // 测试多个破折号的情况
        assert_eq!(
            strip_model_version_suffix("claude-3-5-sonnet-latest"),
            Some("claude-3-5-sonnet".to_string())
        );

        // 测试两位数字的情况
        assert_eq!(
            strip_model_version_suffix("gpt-4-preview-09"),
            Some("gpt-4".to_string())
        );
    }
}
