// 模型相关数据模型

use crate::storage::types::{Model, ModelModality, Timestamp, UUID};
use handbox_llm::types::{LlmModelParameter, ModelPricing};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 聊天方法枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMethod {
    Completions,
    Responses,
    GoogleGenerateContent,
}

/// 前端友好的价格信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricingResponse {
    /// 输入价格（格式化字符串，如 "$0.4/M Tokens"）
    pub input_text: Option<String>,
    /// 输出价格（格式化字符串，如 "$0.4/M Tokens"）
    pub output_text: Option<String>,
}

impl ModelPricingResponse {
    /// 从 ModelPricing 转换为前端友好的格式
    pub fn from_pricing(pricing: &ModelPricing) -> Option<Self> {
        let currency_symbol = pricing
            .currency
            .as_ref()
            .map(|c| match c.as_str() {
                "USD" => "$",
                _ => c.as_str(),
            })
            .unwrap_or("$");

        let input_text = pricing
            .input_text
            .map(|price| format!("{}{}/M Tokens", currency_symbol, price));

        let output_text = pricing
            .output_text
            .map(|price| format!("{}{}/M Tokens", currency_symbol, price));

        if input_text.is_none() && output_text.is_none() {
            None
        } else {
            Some(Self {
                input_text,
                output_text,
            })
        }
    }
}

/// 前端模型响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub output_max_tokens: Option<i32>,
    pub supported_features: Option<Vec<String>>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<ModelModality>>,
    pub output_modalities: Option<Vec<ModelModality>>,
    pub pricing: Option<ModelPricingResponse>,
    pub url: Option<String>,
    pub support_parameters: Option<Vec<LlmModelParameter>>,
    pub default_parameters: Option<HashMap<String, serde_json::Value>>,
    pub max_parameters: Option<HashMap<String, serde_json::Value>>,
    pub chat_methods: Option<Vec<ChatMethod>>,
    pub enabled: bool,
    pub favorite: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl ModelResponse {
    /// 从 Model 转换为 ModelResponse
    pub fn from_model(model: Model) -> Self {
        // 根据 supported_methods 推导 chat_methods
        let chat_methods = model.supported_methods.as_ref().and_then(|methods| {
            let mut result = Vec::new();

            // 检查是否包含 completions 相关方法
            if methods
                .iter()
                .any(|m| m == "completions" || m == "openai_chat_completions")
            {
                result.push(ChatMethod::Completions);
            }

            // 检查是否包含 responses 相关方法
            if methods.iter().any(|m| m == "openai_responses") {
                result.push(ChatMethod::Responses);
            }

            // 检查是否包含 google_generate_content 方法
            if methods.iter().any(|m| m == "google_generate_content") {
                result.push(ChatMethod::GoogleGenerateContent);
            }

            if result.is_empty() {
                None
            } else {
                Some(result)
            }
        });

        // 转换价格信息
        let pricing = model
            .pricing
            .as_ref()
            .and_then(ModelPricingResponse::from_pricing);

        Self {
            id: model.id,
            provider_id: model.provider_id,
            name: model.name,
            context_length: model.context_length,
            output_max_tokens: model.output_max_tokens,
            supported_features: model.supported_features,
            description: model.description,
            input_modalities: model.input_modalities,
            output_modalities: model.output_modalities,
            pricing,
            url: model.url,
            support_parameters: model.support_parameters,
            default_parameters: model.default_parameters,
            max_parameters: model.max_parameters,
            chat_methods,
            enabled: model.enabled,
            favorite: model.favorite,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// 模型列表请求
#[derive(Debug, Clone, Deserialize)]
pub struct ListModelsRequest {
    pub provider_id: UUID,
    pub refresh_from_remote: Option<bool>,
}

/// 模型切换请求
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleModelRequest {
    pub provider_id: UUID,
    pub model_id: String,
    pub enabled: bool,
}

/// 模型收藏切换请求
#[derive(Debug, Clone, Deserialize)]
pub struct ToggleModelFavoriteRequest {
    pub provider_id: UUID,
    pub model_id: String,
    pub favorite: bool,
}
