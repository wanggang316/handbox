// OpenRouter 模型客户端实现

use std::collections::HashMap;

use super::model_client::ModelClient;
use crate::error::LlmClientError;
use crate::types::{
    LlmModel, LlmModelFeature, LlmModelModality, LlmModelParameter, LlmModelPricing, LlmProvider,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;

/// OpenRouter 模型列表响应
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelsResponse {
    pub data: Vec<OpenRouterModel>,
}

/// OpenRouter 完整模型定义
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModel {
    /// 模型唯一标识符
    pub id: String,

    /// 规范的模型别名
    #[serde(default)]
    pub canonical_slug: Option<String>,

    /// 模型显示名称
    pub name: String,

    /// 创建时间 (Unix 时间戳)
    #[serde(default)]
    pub created: Option<i64>,

    /// 模型描述
    #[serde(default)]
    pub description: Option<String>,

    /// 上下文窗口长度
    pub context_length: i32,

    /// 架构信息
    #[serde(default)]
    pub architecture: Option<OpenRouterArchitecture>,

    /// 价格信息
    pub pricing: LlmModelPricing,

    /// 顶级提供商信息
    #[serde(default)]
    pub top_provider: Option<OpenRouterTopProvider>,

    /// 每个请求的限制
    #[serde(default)]
    pub per_request_limits: Option<Value>,

    /// 支持的参数列表
    #[serde(default)]
    pub supported_parameters: Option<Vec<LlmModelParameter>>,

    /// 默认参数配置 - 直接的 key-value 映射，如 {"temperature": 1.0, "top_p": 0.9}
    #[serde(default)]
    pub default_parameters: Option<HashMap<String, serde_json::Value>>,

    /// 最大参数配置 - 直接的 key-value 映射，如 {"temperature": 2.0, "top_p": 1.0}
    #[serde(default)]
    pub max_parameters: Option<HashMap<String, serde_json::Value>>,
}

/// OpenRouter 模型架构信息
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterArchitecture {
    /// 输入模态
    #[serde(default)]
    pub input_modalities: Option<Vec<String>>,

    /// 输出模态
    #[serde(default)]
    pub output_modalities: Option<Vec<String>>,

    /// 分词器类型
    #[serde(default)]
    pub tokenizer: Option<String>,

    /// 指令类型
    #[serde(default)]
    pub instruct_type: Option<String>,
}

/// OpenRouter 顶级提供商信息
#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterTopProvider {
    /// 上下文长度
    #[serde(default)]
    pub context_length: Option<i32>,

    /// 最大补全 token 数
    #[serde(default)]
    pub max_completion_tokens: Option<i32>,

    /// 是否经过审核
    #[serde(default)]
    pub is_moderated: Option<bool>,
}

impl OpenRouterModel {
    /// 转换为标准模型格式
    pub fn to_llm_model(self) -> LlmModel {
        let OpenRouterModel {
            id,
            canonical_slug: _,
            name,
            created: _,
            description,
            context_length,
            architecture,
            pricing: pricing_info,
            top_provider,
            per_request_limits: _,
            supported_parameters,
            default_parameters,
            max_parameters,
        } = self;

        let output_token_limit = top_provider.as_ref().and_then(|p| p.max_completion_tokens);

        let input_modalities = architecture
            .as_ref()
            .and_then(|arch| arch.input_modalities.as_ref())
            .map(|modalities| {
                modalities
                    .iter()
                    .filter_map(|s| s.parse::<LlmModelModality>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        let output_modalities = architecture
            .as_ref()
            .and_then(|arch| arch.output_modalities.as_ref())
            .map(|modalities| {
                modalities
                    .iter()
                    .filter_map(|s| s.parse::<LlmModelModality>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty());

        let mut support_parameters = Vec::new();
        if let Some(params) = supported_parameters {
            for param in params {
                if !support_parameters.contains(&param) {
                    support_parameters.push(param);
                }
            }
        }

        let supported_features = parse_features_from_params(&support_parameters);

        // 将 pricing 转换为 JSON Value
        let pricing = serde_json::to_value(&pricing_info).ok();

        // 构建参数列表
        let default_parameters = clone_non_empty_map(&default_parameters);
        let max_parameters = clone_non_empty_map(&max_parameters);

        LlmModel {
            id,
            name,
            context_length: Some(context_length),
            output_token_limit,
            input_cost: Some(pricing_info.prompt),
            output_cost: Some(pricing_info.completion),
            supported_features,
            description,
            input_modalities,
            output_modalities,
            metadata: None, // 不再需要存储原始数据
            pricing,
            support_parameters,
            default_parameters,
            max_parameters,
        }
    }
}

/// 从支持的参数列表中解析功能
fn parse_features_from_params(params: &[LlmModelParameter]) -> Option<Vec<LlmModelFeature>> {
    let mut features = Vec::new();

    for param in params {
        match param {
            // 工具调用相关参数
            LlmModelParameter::Tools => {
                if !features.contains(&LlmModelFeature::Tool) {
                    features.push(LlmModelFeature::Tool);
                }
            }
            // 推理相关参数
            LlmModelParameter::Reasoning => {
                if !features.contains(&LlmModelFeature::Reasoning) {
                    features.push(LlmModelFeature::Reasoning);
                }
            }
            // 其他参数暂不处理
            _ => {}
        }
    }

    if features.is_empty() {
        None
    } else {
        Some(features)
    }
}

/// 从 default_parameters、max_parameters 和 supported_parameters 构建参数列表
/// 返回非空的参数映射
fn clone_non_empty_map(
    params: &Option<HashMap<String, serde_json::Value>>,
) -> Option<HashMap<String, serde_json::Value>> {
    params.as_ref().filter(|map| !map.is_empty()).cloned()
}

/// OpenRouter 模型客户端
pub struct OpenRouterModelClient {
    client: reqwest::Client,
}

impl OpenRouterModelClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl ModelClient for OpenRouterModelClient {
    async fn list_models(
        &self,
        provider: &LlmProvider,
        _provider_type: &str,
    ) -> Result<Vec<LlmModel>, LlmClientError> {
        let url = format!("{}/models", provider.base_url);
        tracing::info!("Fetching OpenRouter models from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                LlmClientError::transport(format!("Failed to fetch OpenRouter models: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmClientError::api(format!(
                "OpenRouter API returned error {}: {}",
                status, error_text
            )));
        }

        let models_response: OpenRouterModelsResponse = response.json().await.map_err(|e| {
            LlmClientError::unexpected(format!("Failed to parse OpenRouter response: {}", e))
        })?;

        // 直接转换为标准模型
        let standard_models = models_response
            .data
            .into_iter()
            .map(|model| model.to_llm_model())
            .collect();

        Ok(standard_models)
    }
}
