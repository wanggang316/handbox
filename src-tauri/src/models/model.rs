// 模型相关数据模型

use crate::config::llm_config::{get_global_llm_config, ChatMethodConfig};
use crate::storage::types::{Model, ModelModality, Timestamp, UUID};
use handbox_llm::types::{LlmModelParameter, ModelPricing};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 聊天方法枚举
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatMethod {
    Completions,
    Responses,
    GoogleGenerateContent,
}

impl ChatMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMethod::Completions => "completions",
            ChatMethod::Responses => "responses",
            ChatMethod::GoogleGenerateContent => "google_generate_content",
        }
    }

    pub fn iter() -> impl Iterator<Item = ChatMethod> {
        [
            ChatMethod::Completions,
            ChatMethod::Responses,
            ChatMethod::GoogleGenerateContent,
        ]
        .into_iter()
    }
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

/// 聊天方法详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMethodResponse {
    pub name: ChatMethod,
    pub parameters: Option<Vec<ModelParameterResponse>>,
}

/// 单个参数信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameterResponse {
    pub name: LlmModelParameter,
    pub support: bool,
    pub values: Option<ModelParameterValueResponse>,
}

/// 参数值范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameterValueResponse {
    pub default: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// 前端模型响应结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub output_max_tokens: Option<i32>,
    pub display_context_length: Option<String>,
    pub display_output_max_tokens: Option<String>,
    pub supported_features: Option<Vec<String>>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<ModelModality>>,
    pub output_modalities: Option<Vec<ModelModality>>,
    pub pricing: Option<ModelPricingResponse>,
    pub url: Option<String>,
    pub chat_methods: Option<Vec<ChatMethodResponse>>,
    pub enabled: bool,
    pub favorite: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl ModelResponse {
    /// 从 Model 转换为 ModelResponse
    pub fn from_model(model: Model) -> Self {
        let chat_methods = Self::build_chat_method_responses(&model);

        // 转换价格信息
        let pricing = model
            .pricing
            .as_ref()
            .and_then(ModelPricingResponse::from_pricing);

        // 格式化展示字段
        let display_context_length = model.context_length.map(Self::format_number);
        let display_output_max_tokens = model.output_max_tokens.map(Self::format_number);

        Self {
            id: model.id,
            provider_id: model.provider_id,
            name: model.name,
            context_length: model.context_length,
            output_max_tokens: model.output_max_tokens,
            display_context_length,
            display_output_max_tokens,
            supported_features: model.supported_features,
            description: model.description,
            input_modalities: model.input_modalities,
            output_modalities: model.output_modalities,
            pricing,
            url: model.url,
            chat_methods,
            enabled: model.enabled,
            favorite: model.favorite,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }

    /// 格式化数字为可读的字符串
    /// - 大于等于 1,000,000: 除以 1,000,000，显示两位小数 + "M"
    /// - 大于等于 1,000: 除以 1,000，显示两位小数 + "K"
    /// - 小于 1,000: 直接显示原值
    fn format_number(value: i32) -> String {
        if value >= 1_000_000 {
            let formatted = (value as f64 / 1_000_000.0 * 100.0).round() / 100.0;
            format!("{:.2}M", formatted)
        } else if value >= 1_000 {
            let formatted = (value as f64 / 1_000.0 * 100.0).round() / 100.0;
            format!("{:.2}K", formatted)
        } else {
            value.to_string()
        }
    }

    fn build_chat_method_responses(model: &Model) -> Option<Vec<ChatMethodResponse>> {
        let config = get_global_llm_config();
        let supported_methods = model.supported_methods.as_ref();

        let responses: Vec<ChatMethodResponse> = ChatMethod::iter()
            .filter_map(|method| {
                let method_supported = Self::is_method_supported(supported_methods, method);
                let method_config = config.get_chat_method_config(method.as_str());
                let parameters = Self::build_method_parameters(
                    model.support_parameters.as_ref(),
                    model.default_parameters.as_ref(),
                    model.max_parameters.as_ref(),
                    method_config,
                );

                if !method_supported {
                    return None;
                }

                Some(ChatMethodResponse {
                    name: method,
                    parameters,
                })
            })
            .collect();

        if responses.is_empty() {
            None
        } else {
            Some(responses)
        }
    }

    fn is_method_supported(methods: Option<&Vec<String>>, method: ChatMethod) -> bool {
        let Some(methods) = methods else {
            return false;
        };

        match method {
            ChatMethod::Completions => methods.iter().any(|m| m.ends_with("completions")),
            ChatMethod::Responses => methods.iter().any(|m| m.ends_with("responses")),
            ChatMethod::GoogleGenerateContent => {
                methods.iter().any(|m| m == "google_generate_content")
            }
        }
    }

    fn build_method_parameters(
        support_params: Option<&Vec<LlmModelParameter>>,
        db_defaults: Option<&HashMap<String, serde_json::Value>>,
        db_max: Option<&HashMap<String, serde_json::Value>>,
        method_config: Option<&ChatMethodConfig>,
    ) -> Option<Vec<ModelParameterResponse>> {
        let mut parameter_names: HashSet<String> = HashSet::new();
        Self::collect_support_keys(support_params, &mut parameter_names);
        Self::collect_value_keys(db_defaults, &mut parameter_names);
        Self::collect_value_keys(db_max, &mut parameter_names);

        if let Some(config) = method_config {
            Self::collect_value_keys(config.default_parameters.as_ref(), &mut parameter_names);
            Self::collect_value_keys(config.max_parameters.as_ref(), &mut parameter_names);
            if let Some(support_list) = &config.support_parameters {
                for key in support_list {
                    parameter_names.insert(key.clone());
                }
            }
        }

        if parameter_names.is_empty() {
            return None;
        }

        let support_lookup = Self::build_parameter_support_lookup(support_params, method_config);
        let mut names: Vec<String> = parameter_names.into_iter().collect();
        names.sort();

        let mut parameters = Vec::new();
        for key in names {
            let param_enum = key
                .parse::<LlmModelParameter>()
                .unwrap_or(LlmModelParameter::Unknown);

            let values =
                Self::build_parameter_value_for_key(&key, db_defaults, db_max, method_config);

            let support = support_lookup.contains(&key);
            if !support && values.is_none() {
                continue;
            }

            parameters.push(ModelParameterResponse {
                name: param_enum,
                support,
                values,
            });
        }

        if parameters.is_empty() {
            None
        } else {
            Some(parameters)
        }
    }

    fn collect_support_keys(
        support_params: Option<&Vec<LlmModelParameter>>,
        target: &mut HashSet<String>,
    ) {
        if let Some(params) = support_params {
            for param in params {
                target.insert(param.as_str().to_string());
            }
        }
    }

    fn collect_value_keys(
        values: Option<&HashMap<String, serde_json::Value>>,
        target: &mut HashSet<String>,
    ) {
        if let Some(map) = values {
            for key in map.keys() {
                target.insert(key.clone());
            }
        }
    }

    fn build_parameter_support_lookup(
        db_support: Option<&Vec<LlmModelParameter>>,
        method_config: Option<&ChatMethodConfig>,
    ) -> HashSet<String> {
        let mut keys = HashSet::new();
        Self::collect_support_keys(db_support, &mut keys);

        if keys.is_empty() {
            if let Some(config) = method_config {
                if let Some(support_list) = &config.support_parameters {
                    for key in support_list {
                        keys.insert(key.clone());
                    }
                }
            }
        }

        keys
    }

    fn build_parameter_value_for_key(
        key: &str,
        db_defaults: Option<&HashMap<String, serde_json::Value>>,
        db_max: Option<&HashMap<String, serde_json::Value>>,
        method_config: Option<&ChatMethodConfig>,
    ) -> Option<ModelParameterValueResponse> {
        let default_value = Self::resolve_number_for_key(
            key,
            db_defaults,
            method_config.and_then(|config| config.default_parameters.as_ref()),
        );

        let max_value = Self::resolve_number_for_key(
            key,
            db_max,
            method_config.and_then(|config| config.max_parameters.as_ref()),
        );

        if default_value.is_none() && max_value.is_none() {
            None
        } else {
            Some(ModelParameterValueResponse {
                default: default_value,
                min: None,
                max: max_value,
            })
        }
    }

    fn resolve_number_for_key(
        key: &str,
        primary: Option<&HashMap<String, serde_json::Value>>,
        fallback: Option<&HashMap<String, serde_json::Value>>,
    ) -> Option<f64> {
        primary
            .and_then(|map| map.get(key))
            .or_else(|| fallback.and_then(|map| map.get(key)))
            .and_then(Self::parse_number)
    }

    fn parse_number(value: &serde_json::Value) -> Option<f64> {
        match value {
            serde_json::Value::Number(num) => num.as_f64(),
            serde_json::Value::String(text) => text.parse::<f64>().ok(),
            _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number_less_than_1000() {
        assert_eq!(ModelResponse::format_number(0), "0");
        assert_eq!(ModelResponse::format_number(1), "1");
        assert_eq!(ModelResponse::format_number(999), "999");
    }

    #[test]
    fn test_format_number_thousands() {
        // 1,000 -> 1.00K
        assert_eq!(ModelResponse::format_number(1000), "1.00K");

        // 1,089 -> 1.09K (四舍五入)
        assert_eq!(ModelResponse::format_number(1089), "1.09K");

        // 1,094 -> 1.09K (四舍五入)
        assert_eq!(ModelResponse::format_number(1094), "1.09K");

        // 1,095 -> 1.10K (四舍五入)
        assert_eq!(ModelResponse::format_number(1095), "1.10K");

        // 12,345 -> 12.35K (四舍五入)
        assert_eq!(ModelResponse::format_number(12345), "12.35K");

        // 999,999 -> 1000.00K
        assert_eq!(ModelResponse::format_number(999999), "1000.00K");
    }

    #[test]
    fn test_format_number_millions() {
        // 1,000,000 -> 1.00M
        assert_eq!(ModelResponse::format_number(1_000_000), "1.00M");

        // 1,048,938 -> 1.05M (四舍五入)
        assert_eq!(ModelResponse::format_number(1_048_938), "1.05M");

        // 1,044,999 -> 1.04M (四舍五入)
        assert_eq!(ModelResponse::format_number(1_044_999), "1.04M");

        // 1,045_000 -> 1.05M (四舍五入)
        assert_eq!(ModelResponse::format_number(1_045_000), "1.05M");

        // 128,000,000 -> 128.00M
        assert_eq!(ModelResponse::format_number(128_000_000), "128.00M");

        // 2,097,152 -> 2.10M (四舍五入)
        assert_eq!(ModelResponse::format_number(2_097_152), "2.10M");
    }

    #[test]
    fn test_format_number_edge_cases() {
        // 边界值测试
        assert_eq!(ModelResponse::format_number(999), "999");
        assert_eq!(ModelResponse::format_number(1000), "1.00K");
        assert_eq!(ModelResponse::format_number(999_999), "1000.00K");
        assert_eq!(ModelResponse::format_number(1_000_000), "1.00M");
    }
}
