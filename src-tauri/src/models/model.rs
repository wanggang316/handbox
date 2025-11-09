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

/// 参数显示等级
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterLevel {
    Base,    // 基础参数，默认显示
    Advance, // 高级参数，在"高级"分组中显示
}

/// 参数组件类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterComponent {
    Slider,    // 滑块组件
    Switch,    // 开关组件
    Reasoning, // 推理/思维配置组件
}

/// 滑块组件属性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliderProps {
    pub default: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub name: String,
    pub show_toggle: Option<bool>,
}

/// 开关组件属性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchProps {
    pub default: Option<bool>,
    pub name: String,
}

/// 组件属性联合类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentProps {
    Slider(SliderProps),
    Switch(SwitchProps),
    Reasoning(ReasoningProps),
}

/// 推理配置属性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningProps {
    pub name: String,
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
    pub name: String,
    pub support: bool,
    pub component: ParameterComponent,
    pub props: ComponentProps,
    pub level: ParameterLevel,
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
    pub supported_parameters: Option<Vec<LlmModelParameter>>,
    pub supported_chat_methods: Option<Vec<ChatMethod>>,
    pub chat_method: Option<ChatMethodResponse>,
    pub enabled: bool,
    pub favorite: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl ModelResponse {
    /// 从 Model 转换为 ModelResponse
    pub fn from_model(model: Model) -> Self {
        let chat_method_responses = Self::build_chat_method_responses(&model);

        // 提取支持的聊天方法列表和推荐的方法
        let (supported_chat_methods, chat_method) = if let Some(methods) = chat_method_responses {
            let supported = methods.iter().map(|m| m.name).collect();
            // 优先选择 responses，其次退回到列表第一个
            let recommended = methods
                .iter()
                .find(|m| m.name == ChatMethod::Responses)
                .cloned()
                .or_else(|| methods.first().cloned());
            (Some(supported), recommended)
        } else {
            (None, None)
        };

        // 转换价格信息
        let pricing = model
            .pricing
            .as_ref()
            .and_then(ModelPricingResponse::from_pricing);

        // 格式化展示字段
        let display_context_length = model.context_length.map(Self::format_number);
        let display_output_max_tokens = model.output_max_tokens.map(Self::format_number);

        // 转换 supported_parameters 从 Vec<String> 到 Vec<LlmModelParameter>
        // 过滤掉 Unknown 参数
        let supported_parameters = model.supported_parameters.map(|params| {
            params
                .iter()
                .filter_map(|s| s.parse::<LlmModelParameter>().ok())
                .filter(|param| *param != LlmModelParameter::Unknown)
                .collect()
        });

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
            supported_parameters,
            supported_chat_methods,
            chat_method,
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

                // Convert Vec<String> to Vec<LlmModelParameter> for the parameters builder
                // 过滤掉 Unknown 参数
                let supported_params = model.supported_parameters.as_ref().map(|params| {
                    params
                        .iter()
                        .filter_map(|s| s.parse::<LlmModelParameter>().ok())
                        .filter(|param| *param != LlmModelParameter::Unknown)
                        .collect::<Vec<_>>()
                });

                let parameters = Self::build_method_parameters(
                    supported_params.as_ref(),
                    model.default_parameters.as_ref(),
                    model.max_parameters.as_ref(),
                    &method_config,
                    model.output_max_tokens,
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
        supported_params: Option<&Vec<LlmModelParameter>>,
        db_defaults: Option<&HashMap<String, serde_json::Value>>,
        db_max: Option<&HashMap<String, serde_json::Value>>,
        method_config: &ChatMethodConfig,
        output_max_tokens: Option<i32>,
    ) -> Option<Vec<ModelParameterResponse>> {
        let mut parameter_names: HashSet<String> = HashSet::new();

        // 1. 如果数据库有 supported_params，使用数据库的；否则使用配置的 default_supported_parameters
        if let Some(params) = supported_params {
            if !params.is_empty() {
                // 使用数据库的 supported_params
                Self::collect_support_keys(Some(params), &mut parameter_names);
            } else {
                // 数据库的 supported_params 为空，使用配置的 default_supported_parameters
                for key in &method_config.default_supported_parameters {
                    parameter_names.insert(key.clone());
                }
            }
        } else {
            // 数据库没有 supported_params，使用配置的 default_supported_parameters
            for key in &method_config.default_supported_parameters {
                parameter_names.insert(key.clone());
            }
        }

        // 2. 添加配置的额外参数（如 turn_count）
        for key in &method_config.additional_parameters {
            parameter_names.insert(key.clone());
        }

        // 3. 添加数据库中的 defaults 和 max（兼容旧数据）
        Self::collect_value_keys(db_defaults, &mut parameter_names);
        Self::collect_value_keys(db_max, &mut parameter_names);

        if parameter_names.is_empty() {
            return None;
        }

        let support_lookup = Self::build_parameter_support_lookup(supported_params, method_config);
        let mut names: Vec<String> = parameter_names.into_iter().collect();
        names.sort();

        let mut parameters = Vec::new();
        for key in names {
            // 获取参数配置
            let param_config = method_config.parameters.get(&key);

            // 如果没有参数配置，跳过该参数
            if param_config.is_none() {
                continue;
            }

            // 检查 component 字段是否存在
            let config = param_config.unwrap();
            if config.component.is_none() {
                continue;
            }

            let param_enum = key
                .parse::<LlmModelParameter>()
                .unwrap_or(LlmModelParameter::Unknown);

            // 注意：对于应用层配置（如 turn_count），它们会被解析为 Unknown
            // 但由于它们有参数配置，所以我们保留它们
            // support 字段表示该参数是否被模型支持，应用层配置永远为 false
            let support = support_lookup.contains(&key);

            // 构建组件和属性
            let (component, props, level) = Self::build_component_and_props(
                &key,
                &param_enum,
                db_defaults,
                db_max,
                Some(config),
                output_max_tokens,
            );

            parameters.push(ModelParameterResponse {
                name: key.clone(),
                support,
                component,
                props,
                level,
            });
        }

        if parameters.is_empty() {
            None
        } else {
            Some(parameters)
        }
    }

    fn collect_support_keys(
        supported_params: Option<&Vec<LlmModelParameter>>,
        target: &mut HashSet<String>,
    ) {
        if let Some(params) = supported_params {
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
        db_supported: Option<&Vec<LlmModelParameter>>,
        method_config: &ChatMethodConfig,
    ) -> HashSet<String> {
        let mut keys = HashSet::new();
        Self::collect_support_keys(db_supported, &mut keys);

        if keys.is_empty() {
            for key in &method_config.default_supported_parameters {
                keys.insert(key.clone());
            }
        }

        keys
    }

    fn build_component_and_props(
        key: &str,
        param: &LlmModelParameter,
        db_defaults: Option<&HashMap<String, serde_json::Value>>,
        db_max: Option<&HashMap<String, serde_json::Value>>,
        param_config: Option<&crate::config::llm_config::ParameterConfig>,
        output_max_tokens: Option<i32>,
    ) -> (ParameterComponent, ComponentProps, ParameterLevel) {
        // param_config 在调用前已经检查过，这里可以安全 unwrap
        let config = param_config.expect("param_config should not be None");

        // 确定组件类型 (component 字段在调用前也已经检查过)
        let component = match config.component.as_deref() {
            Some("switch") => ParameterComponent::Switch,
            Some("slider") => ParameterComponent::Slider,
            Some("reasoning") => ParameterComponent::Reasoning,
            _ => ParameterComponent::Slider, // 默认为 Slider
        };

        // 确定显示等级
        let level = match config.level.as_deref() {
            Some("base") => ParameterLevel::Base,
            Some("advance") => ParameterLevel::Advance,
            _ => ParameterLevel::Advance, // 默认高级
        };

        // 获取显示名称
        let name = config
            .name
            .clone()
            .unwrap_or_else(|| param.as_str().to_string());

        // 构建属性
        let props = match component {
            ParameterComponent::Switch => {
                // 优先使用配置中的 default，然后是数据库的值
                let default = config
                    .default
                    .as_ref()
                    .and_then(Self::parse_bool)
                    .or_else(|| Self::resolve_bool_for_key(key, db_defaults, None));
                ComponentProps::Switch(SwitchProps { default, name })
            }
            ParameterComponent::Slider => {
                // 对于 max_tokens 参数，默认值和最大值都使用模型的 output_max_tokens
                let (default, max) = if key == "max_tokens" {
                    let output_max = output_max_tokens.map(|v| v as f64);
                    (
                        // default: 优先使用模型的 output_max_tokens，然后是配置，最后是数据库
                        output_max
                            .or_else(|| config.default.as_ref().and_then(Self::parse_number))
                            .or_else(|| Self::resolve_number_for_key(key, db_defaults, None)),
                        // max: 优先使用模型的 output_max_tokens，然后是配置，最后是数据库
                        output_max
                            .or_else(|| config.max.as_ref().and_then(Self::parse_number))
                            .or_else(|| Self::resolve_number_for_key(key, db_max, None)),
                    )
                } else {
                    (
                        // default: 优先使用配置中的 default，然后是数据库的值
                        config
                            .default
                            .as_ref()
                            .and_then(Self::parse_number)
                            .or_else(|| Self::resolve_number_for_key(key, db_defaults, None)),
                        // max: 优先使用配置中的 max，然后是数据库的值
                        config
                            .max
                            .as_ref()
                            .and_then(Self::parse_number)
                            .or_else(|| Self::resolve_number_for_key(key, db_max, None)),
                    )
                };

                let step = config.step;
                let show_toggle = config.show_toggle;

                ComponentProps::Slider(SliderProps {
                    default,
                    min: Some(0.0), // 默认最小值为0
                    max,
                    step,
                    name,
                    show_toggle,
                })
            }
            ParameterComponent::Reasoning => ComponentProps::Reasoning(ReasoningProps { name }),
        };

        (component, props, level)
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

    fn resolve_bool_for_key(
        key: &str,
        primary: Option<&HashMap<String, serde_json::Value>>,
        fallback: Option<&HashMap<String, serde_json::Value>>,
    ) -> Option<bool> {
        primary
            .and_then(|map| map.get(key))
            .or_else(|| fallback.and_then(|map| map.get(key)))
            .and_then(Self::parse_bool)
    }

    fn parse_bool(value: &serde_json::Value) -> Option<bool> {
        match value {
            serde_json::Value::Bool(b) => Some(*b),
            serde_json::Value::String(text) => text.parse::<bool>().ok(),
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
