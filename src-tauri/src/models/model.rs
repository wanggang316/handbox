use crate::config::llm_config::{get_global_llm_config, ChatMethodConfig};
use crate::storage::types::{Model, ModelModality, Timestamp, UUID};
use crate::models::llm_types::{LlmModelParameter, ModelPricing};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

/// Display-formatted pricing for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricingResponse {
    /// Formatted input price, e.g. "$0.4/M Tokens".
    pub input_text: Option<String>,
    /// Formatted output price, e.g. "$0.4/M Tokens".
    pub output_text: Option<String>,
}

impl ModelPricingResponse {
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

/// UI display tier for a parameter.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterLevel {
    Base,    // shown by default
    Advance, // shown in the "Advanced" group
}

/// Frontend widget used to edit a parameter.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterComponent {
    Slider,
    Switch,
    ResponsesReasoning,
    CompletionsReasoning,
    Thinking,
    OpenrouterReasoning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliderProps {
    pub default: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub name: String,
    pub show_toggle: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchProps {
    pub default: Option<bool>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentProps {
    Slider(SliderProps),
    Switch(SwitchProps),
    ResponsesReasoning(ResponsesReasoningProps),
    CompletionsReasoning(CompletionsReasoningProps),
    Thinking(ThinkingProps),
    OpenrouterReasoning(OpenrouterReasoningProps),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesReasoningProps {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_options: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_options: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionsReasoningProps {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_reasoning: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_options: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingProps {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_configs: Option<Vec<crate::config::llm_config::BudgetConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts_tip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenrouterReasoningProps {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tips: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_tips: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens_tips: Option<String>,
    /// Props resolved by the backend from `model_id` and `special_props`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMethodResponse {
    pub name: ChatMethod,
    pub parameters: Option<Vec<ModelParameterResponse>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameterResponse {
    pub name: String,
    pub support: bool,
    pub component: ParameterComponent,
    pub props: ComponentProps,
    pub level: ParameterLevel,
}

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
    pub support_tools: bool,
    pub support_image: bool,
    pub enabled: bool,
    pub favorite: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl ModelResponse {
    /// Conversion without provider-level parameter overrides.
    pub fn from_model(model: Model) -> Self {
        Self::from_model_with_provider(model, None)
    }

    /// Converts a `Model`, applying provider-level parameter overrides when
    /// `provider_type` is given.
    pub fn from_model_with_provider(model: Model, provider_type: Option<&str>) -> Self {
        let chat_method_responses = Self::build_chat_method_responses(&model, provider_type);

        let (supported_chat_methods, chat_method) = if let Some(methods) = chat_method_responses {
            let supported = methods.iter().map(|m| m.name).collect();
            // Prefer responses; fall back to the first supported method.
            let recommended = methods
                .iter()
                .find(|m| m.name == ChatMethod::Responses)
                .cloned()
                .or_else(|| methods.first().cloned());
            (Some(supported), recommended)
        } else {
            (None, None)
        };

        let pricing = model
            .pricing
            .as_ref()
            .and_then(ModelPricingResponse::from_pricing);

        let display_context_length = model.context_length.map(Self::format_number);
        let display_output_max_tokens = model.output_max_tokens.map(Self::format_number);

        let supported_parameters = model.supported_parameters.map(|params| {
            params
                .iter()
                .filter_map(|s| s.parse::<LlmModelParameter>().ok())
                .filter(|param| *param != LlmModelParameter::Unknown)
                .collect()
        });

        let support_tools = model
            .supported_features
            .as_ref()
            .map(|features| {
                features
                    .iter()
                    .any(|f| f == "function_calling" || f == "tool" || f == "tools")
            })
            .unwrap_or(false);

        let support_image = model
            .supported_features
            .as_ref()
            .map(|features| features.iter().any(|f| f == "image_generation"))
            .unwrap_or(false)
            || model
                .output_modalities
                .as_ref()
                .map(|modalities| modalities.contains(&ModelModality::Image))
                .unwrap_or(false);

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
            support_tools,
            support_image,
            enabled: model.enabled,
            favorite: model.favorite,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }

    /// Formats a count as "1.23M" / "1.23K", or the raw value below 1,000.
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

    fn build_chat_method_responses(
        model: &Model,
        provider_type: Option<&str>,
    ) -> Option<Vec<ChatMethodResponse>> {
        let config = get_global_llm_config();
        let supported_methods = model.supported_methods.as_ref();

        let responses: Vec<ChatMethodResponse> = ChatMethod::iter()
            .filter_map(|method| {
                let method_supported = Self::is_method_supported(supported_methods, method);

                let method_config = if let Some(ptype) = provider_type {
                    config.get_merged_config_with_provider(method.as_str(), ptype)
                } else {
                    config.get_chat_method_config(method.as_str())
                };

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
                    &model.id,
                    provider_type.unwrap_or(""),
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
        model_id: &str,
        provider_type: &str,
    ) -> Option<Vec<ModelParameterResponse>> {
        let mut parameter_names: HashSet<String> = HashSet::new();

        // DB supported_params wins; empty or missing falls back to the
        // config's default_supported_parameters.
        if let Some(params) = supported_params {
            if !params.is_empty() {
                Self::collect_support_keys(Some(params), &mut parameter_names);
            } else {
                for key in &method_config.default_supported_parameters {
                    parameter_names.insert(key.clone());
                }
            }
        } else {
            for key in &method_config.default_supported_parameters {
                parameter_names.insert(key.clone());
            }
        }

        // Config-defined extra parameters (e.g. turn_count).
        for key in &method_config.additional_parameters {
            parameter_names.insert(key.clone());
        }

        // Keys from DB defaults/max, for legacy data.
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
            let param_config = method_config.parameters.get(&key);

            if param_config.is_none() {
                continue;
            }

            let config = param_config.unwrap();
            if config.component.is_none() {
                continue;
            }

            let param_enum = key
                .parse::<LlmModelParameter>()
                .unwrap_or(LlmModelParameter::Unknown);

            // App-level parameters (e.g. turn_count) parse to Unknown but are
            // kept since they have a parameter config; `support` means
            // "supported by the model" and is always false for them.
            let support = support_lookup.contains(&key);

            let (component, props, level) = Self::build_component_and_props(
                &key,
                &param_enum,
                db_defaults,
                db_max,
                Some(config),
                output_max_tokens,
                model_id,
                provider_type,
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

    #[allow(clippy::too_many_arguments)]
    fn build_component_and_props(
        key: &str,
        param: &LlmModelParameter,
        db_defaults: Option<&HashMap<String, serde_json::Value>>,
        db_max: Option<&HashMap<String, serde_json::Value>>,
        param_config: Option<&crate::config::llm_config::ParameterConfig>,
        output_max_tokens: Option<i32>,
        model_id: &str,
        _provider_type: &str,
    ) -> (ParameterComponent, ComponentProps, ParameterLevel) {
        // Checked by the caller; safe to unwrap.
        let config = param_config.expect("param_config should not be None");

        let component = match config.component.as_deref() {
            Some("switch") => ParameterComponent::Switch,
            Some("slider") => ParameterComponent::Slider,
            Some("responses_reasoning") => ParameterComponent::ResponsesReasoning,
            Some("completions_reasoning") => ParameterComponent::CompletionsReasoning,
            Some("thinking") => ParameterComponent::Thinking,
            Some("openrouter_reasoning") => ParameterComponent::OpenrouterReasoning,
            _ => ParameterComponent::Slider,
        };

        let level = match config.level.as_deref() {
            Some("base") => ParameterLevel::Base,
            Some("advance") => ParameterLevel::Advance,
            _ => ParameterLevel::Advance,
        };

        let name = config
            .name
            .clone()
            .unwrap_or_else(|| param.as_str().to_string());

        let props = match component {
            ParameterComponent::Switch => {
                // Config default wins over the DB value.
                let default = config
                    .default
                    .as_ref()
                    .and_then(Self::parse_bool)
                    .or_else(|| Self::resolve_bool_for_key(key, db_defaults, None));
                ComponentProps::Switch(SwitchProps {
                    default,
                    name,
                    tips: config.tips.clone(),
                })
            }
            ParameterComponent::Slider => {
                // For max_tokens, the model's output_max_tokens takes priority
                // for both default and max; then config, then DB.
                let (default, max) = if key == "max_tokens" {
                    let output_max = output_max_tokens.map(|v| v as f64);
                    (
                        output_max
                            .or_else(|| config.default.as_ref().and_then(Self::parse_number))
                            .or_else(|| Self::resolve_number_for_key(key, db_defaults, None)),
                        output_max
                            .or_else(|| config.max.as_ref().and_then(Self::parse_number))
                            .or_else(|| Self::resolve_number_for_key(key, db_max, None)),
                    )
                } else {
                    (
                        config
                            .default
                            .as_ref()
                            .and_then(Self::parse_number)
                            .or_else(|| Self::resolve_number_for_key(key, db_defaults, None)),
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
                    min: Some(0.0),
                    max,
                    step,
                    name,
                    show_toggle,
                    tips: config.tips.clone(),
                })
            }
            ParameterComponent::ResponsesReasoning => {
                ComponentProps::ResponsesReasoning(ResponsesReasoningProps {
                    name,
                    effort_options: config.effort_options.clone(),
                    summary_options: config.summary_options.clone(),
                    tips: config.tips.clone(),
                })
            }
            ParameterComponent::CompletionsReasoning => {
                ComponentProps::CompletionsReasoning(CompletionsReasoningProps {
                    name,
                    include_reasoning: config.include_reasoning,
                    effort_options: config.effort_options.clone(),
                    tips: config.tips.clone(),
                })
            }
            ParameterComponent::Thinking => ComponentProps::Thinking(ThinkingProps {
                name,
                budget_configs: config.budget_configs.clone(),
                tips: config.tips.clone(),
                include_thoughts_tip: config.include_thoughts_tip.clone(),
                budget_tip: config.budget_tip.clone(),
            }),
            ParameterComponent::OpenrouterReasoning => {
                // Config stores effort options as HashMap<String, Vec<String>>;
                // extract a single list.
                let effort_opts = config.effort_options.as_ref().and_then(|opts| {
                    // Prefer the "common" key, else the first entry.
                    opts.get("common").or_else(|| opts.values().next()).cloned()
                });

                // OpenRouter model ids already carry the provider prefix
                // (e.g. anthropic/claude-*), so match special_props on model_id.
                let resolved_props = Self::resolve_openrouter_props(
                    config.default_props.as_ref(),
                    config.special_props.as_ref(),
                    model_id,
                );

                ComponentProps::OpenrouterReasoning(OpenrouterReasoningProps {
                    name,
                    tips: config.tips.clone(),
                    effect_tips: config.effect_tips.clone(),
                    max_tokens_tips: config.max_tokens_tips.clone(),
                    props: resolved_props,
                    effort_options: effort_opts,
                    max_tokens: config.max_tokens.clone(),
                })
            }
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

    /// Resolves which OpenRouter reasoning props to show: the first
    /// `special_props` regex matching `model_key` wins, else `default_props`.
    fn resolve_openrouter_props(
        default_props: Option<&Vec<String>>,
        special_props: Option<&HashMap<String, Vec<String>>>,
        model_key: &str,
    ) -> Option<Vec<String>> {
        if let Some(patterns) = special_props {
            for (pattern, props_list) in patterns {
                if let Ok(regex) = regex::Regex::new(pattern) {
                    if regex.is_match(model_key) {
                        return Some(props_list.clone());
                    }
                }
            }
        }

        default_props.cloned()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListModelsRequest {
    pub provider_id: UUID,
    pub refresh_from_remote: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToggleModelRequest {
    pub provider_id: UUID,
    pub model_id: String,
    pub enabled: bool,
}

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
        assert_eq!(ModelResponse::format_number(1000), "1.00K");

        // rounding
        assert_eq!(ModelResponse::format_number(1089), "1.09K");
        assert_eq!(ModelResponse::format_number(1094), "1.09K");
        assert_eq!(ModelResponse::format_number(1095), "1.10K");
        assert_eq!(ModelResponse::format_number(12345), "12.35K");

        assert_eq!(ModelResponse::format_number(999999), "1000.00K");
    }

    #[test]
    fn test_format_number_millions() {
        assert_eq!(ModelResponse::format_number(1_000_000), "1.00M");

        // rounding
        assert_eq!(ModelResponse::format_number(1_048_938), "1.05M");
        assert_eq!(ModelResponse::format_number(1_044_999), "1.04M");
        assert_eq!(ModelResponse::format_number(1_045_000), "1.05M");

        assert_eq!(ModelResponse::format_number(128_000_000), "128.00M");

        // rounding
        assert_eq!(ModelResponse::format_number(2_097_152), "2.10M");
    }

    #[test]
    fn test_format_number_edge_cases() {
        assert_eq!(ModelResponse::format_number(999), "999");
        assert_eq!(ModelResponse::format_number(1000), "1.00K");
        assert_eq!(ModelResponse::format_number(999_999), "1000.00K");
        assert_eq!(ModelResponse::format_number(1_000_000), "1.00M");
    }
}
