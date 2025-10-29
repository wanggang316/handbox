use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use crate::error::LlmClientError;

/// 要从 supplement 合并的字段名称
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SupplementField {
    /// 基础信息：name
    Name,
    /// 基础信息：context_length
    ContextLength,
    /// 基础信息：output_max_tokens
    OutputMaxTokens,
    /// 基础信息：description
    Description,
    /// 基础信息：url
    Url,
    /// 定价信息
    Pricing,
    /// 支持的特性列表
    SupportedFeatures,
    /// 输入模态
    InputModalities,
    /// 输出模态
    OutputModalities,
    /// 支持的参数
    SupportParameters,
    /// 默认参数值
    DefaultParameters,
    /// 最大参数值
    MaxParameters,
    /// 支持的方法
    SupportedMethods,
    /// 元数据
    Metadata,
}

/// 检查是否应该合并指定字段
/// 如果 fields 为空，则不合并任何字段
pub fn should_merge_field(fields: &[SupplementField], field: &SupplementField) -> bool {
    !fields.is_empty() && fields.contains(field)
}

/// 模型信息
#[derive(Debug, Clone)]
pub struct LlmModel {
    pub id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub output_max_tokens: Option<i32>,
    pub supported_features: Option<Vec<String>>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<LlmModelModality>>,
    pub output_modalities: Option<Vec<LlmModelModality>>,
    pub metadata: Option<Value>,
    pub pricing: Option<ModelPricing>,
    pub url: Option<String>,
    pub support_parameters: Vec<LlmModelParameter>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
    pub supported_methods: Option<Vec<String>>,
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelPricing {
    pub currency: Option<String>,
    pub input_text: Option<f32>,
    pub output_text: Option<f32>,
}

/// 模型模态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LlmModelModality {
    Text,
    #[serde(alias = "images")]
    Image,
    Pdf,
    File,
    Audio,
    Video,
}

impl FromStr for LlmModelModality {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.trim();

        if value.eq_ignore_ascii_case("text") {
            Ok(LlmModelModality::Text)
        } else if value.eq_ignore_ascii_case("image") || value.eq_ignore_ascii_case("images") {
            Ok(LlmModelModality::Image)
        } else if value.eq_ignore_ascii_case("pdf") {
            Ok(LlmModelModality::Pdf)
        } else if value.eq_ignore_ascii_case("file") {
            Ok(LlmModelModality::File)
        } else if value.eq_ignore_ascii_case("audio") {
            Ok(LlmModelModality::Audio)
        } else if value.eq_ignore_ascii_case("video") {
            Ok(LlmModelModality::Video)
        } else {
            Err(())
        }
    }
}

/// 模型支持的参数类型
/// 通用参数定义，可被各个 adapter 使用
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmModelParameter {
    /// 工具调用支持
    Tools,
    /// 工具选择策略
    ToolChoice,

    /// 最大生成 token 数
    MaxTokens,

    /// 采样温度 (0.0-2.0)
    Temperature,
    /// Top-p 核采样
    TopP,
    /// Top-k 采样
    TopK,

    /// 推理模式支持
    Reasoning,
    /// 在响应中包含推理过程
    IncludeReasoning,

    /// 结构化输出支持
    StructuredOutputs,
    /// 响应格式控制
    ResponseFormat,

    /// 停止序列
    Stop,

    /// 频率惩罚 (-2.0 to 2.0)
    FrequencyPenalty,
    /// 存在惩罚 (-2.0 to 2.0)
    PresencePenalty,

    /// 随机种子
    Seed,

    /// 其他未知参数
    #[serde(other)]
    Unknown,
}

impl LlmModelParameter {
    /// 将参数枚举转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmModelParameter::Tools => "tools",
            LlmModelParameter::ToolChoice => "tool_choice",
            LlmModelParameter::MaxTokens => "max_tokens",
            LlmModelParameter::Temperature => "temperature",
            LlmModelParameter::TopP => "top_p",
            LlmModelParameter::TopK => "top_k",
            LlmModelParameter::Reasoning => "reasoning",
            LlmModelParameter::IncludeReasoning => "include_reasoning",
            LlmModelParameter::StructuredOutputs => "structured_outputs",
            LlmModelParameter::ResponseFormat => "response_format",
            LlmModelParameter::Stop => "stop",
            LlmModelParameter::FrequencyPenalty => "frequency_penalty",
            LlmModelParameter::PresencePenalty => "presence_penalty",
            LlmModelParameter::Seed => "seed",
            LlmModelParameter::Unknown => "unknown",
        }
    }
}

impl FromStr for LlmModelParameter {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "tools" => LlmModelParameter::Tools,
            "tool_choice" => LlmModelParameter::ToolChoice,
            "max_tokens" => LlmModelParameter::MaxTokens,
            "temperature" => LlmModelParameter::Temperature,
            "top_p" => LlmModelParameter::TopP,
            "top_k" => LlmModelParameter::TopK,
            "reasoning" => LlmModelParameter::Reasoning,
            "include_reasoning" => LlmModelParameter::IncludeReasoning,
            "structured_outputs" => LlmModelParameter::StructuredOutputs,
            "response_format" => LlmModelParameter::ResponseFormat,
            "stop" => LlmModelParameter::Stop,
            "frequency_penalty" => LlmModelParameter::FrequencyPenalty,
            "presence_penalty" => LlmModelParameter::PresencePenalty,
            "seed" => LlmModelParameter::Seed,
            _ => LlmModelParameter::Unknown,
        })
    }
}

/// 模型价格信息
/// 通用价格结构，可被各个 adapter 使用
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmModelPricing {
    /// 提示词价格 (每百万 token，API 可能返回字符串格式如 "0.00001")
    #[serde(deserialize_with = "deserialize_price")]
    pub prompt: f32,

    /// 补全价格 (每百万 token，API 可能返回字符串格式如 "0.00002")
    #[serde(deserialize_with = "deserialize_price")]
    pub completion: f32,

    /// 每次请求价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub request: Option<f32>,

    /// 图片价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub image: Option<f32>,

    /// 网络搜索价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub web_search: Option<f32>,

    /// 内部推理价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub internal_reasoning: Option<f32>,

    /// 输入缓存读取价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub input_cache_read: Option<f32>,

    /// 输入缓存写入价格 (可选)
    #[serde(default, deserialize_with = "deserialize_optional_price")]
    pub input_cache_write: Option<f32>,
}

/// 价格字符串反序列化为 f32
/// API 可能返回价格为字符串格式（如 "0.00001"），需要转换为数字
fn deserialize_price<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f32>().map_err(D::Error::custom)
}

/// 可选价格字符串反序列化为 Option<f32>
/// 支持 null、缺失字段或空字符串
fn deserialize_optional_price<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let opt: Option<String> = Option::deserialize(deserializer)?;
    match opt {
        Some(s) if !s.is_empty() => s.parse::<f32>().map(Some).map_err(D::Error::custom),
        _ => Ok(None),
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ModelSupplementSource {
    #[serde(default)]
    pub scraped_at: Option<String>,
    #[serde(default)]
    pub total_models: Option<u32>,
    #[serde(default)]
    pub detailed_models: Option<u32>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub index_url: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ModelSupplementMetadata {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub source: Option<ModelSupplementSource>,
    #[serde(default)]
    pub total_models: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ModelSupplementDocument {
    #[serde(default)]
    pub metadata: Option<ModelSupplementMetadata>,
    #[serde(default)]
    pub models: Vec<ModelSupplement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelSupplement {
    #[serde(rename = "model_code")]
    pub model_code: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub context_length: Option<i32>,
    #[serde(default)]
    pub output_max_tokens: Option<i32>,
    #[serde(default)]
    pub input_cost: Option<f32>,
    #[serde(default)]
    pub output_cost: Option<f32>,
    #[serde(default)]
    pub supported_features: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_modalities: Option<Vec<LlmModelModality>>,
    #[serde(default)]
    pub output_modalities: Option<Vec<LlmModelModality>>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub support_parameters: Vec<LlmModelParameter>,
    #[serde(default)]
    pub default_parameters: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub max_parameters: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub snapshots: Vec<String>,
    #[serde(default)]
    pub endpoints: Vec<String>,
    #[serde(default)]
    pub supported_methods: Option<Vec<String>>,
    #[serde(default)]
    pub url: Option<String>,
}

pub fn merge_pricing(
    pricing: &mut Option<ModelPricing>,
    input_text: Option<f32>,
    output_text: Option<f32>,
    currency: Option<&str>,
) {
    if input_text.is_none() && output_text.is_none() && currency.is_none() {
        return;
    }

    let mut current = pricing.take().unwrap_or_default();

    if let Some(curr) = currency {
        if !curr.is_empty() {
            current.currency = Some(curr.to_string());
        }
    }

    if let Some(input) = input_text {
        current.input_text = Some(input);
    }

    if let Some(output) = output_text {
        current.output_text = Some(output);
    }

    if current.currency.is_none() && current.input_text.is_none() && current.output_text.is_none() {
        *pricing = None;
    } else {
        *pricing = Some(current);
    }
}

pub fn extract_pricing_value(pricing: &Option<ModelPricing>, key: &str) -> Option<f32> {
    pricing.as_ref().and_then(|value| match key {
        "input_text" => value.input_text,
        "output_text" => value.output_text,
        _ => None,
    })
}

/// 将字符串转换为 snake_case 格式
/// 例如: "chatCompletions" -> "chat_completions", "Generate Content" -> "generate_content"
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_is_lowercase = false;

    for (i, ch) in s.chars().enumerate() {
        if ch.is_whitespace() {
            // 空格转换为下划线
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            prev_is_lowercase = false;
        } else if ch.is_uppercase() {
            if i > 0 && prev_is_lowercase {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
            prev_is_lowercase = false;
        } else {
            result.push(ch);
            prev_is_lowercase = ch.is_lowercase();
        }
    }

    result
}

/// 将 endpoints 转换为 supported_methods 格式
/// 添加指定前缀并转换为 snake_case
pub fn convert_endpoints_to_methods(endpoints: &[String], prefix: &str) -> Vec<String> {
    endpoints
        .iter()
        .map(|endpoint| {
            let snake = to_snake_case(endpoint);
            format!("{}_{}", prefix, snake)
        })
        .collect()
}

/// 统一的 supplement 合并函数
/// 如果 supplement_fields 中配置了字段，就使用 supplement_file 文件中的字段值
pub fn merge_supplement(
    mut model: LlmModel,
    supplement: &ModelSupplement,
    fields: &[SupplementField],
    provider_type: &str,
) -> LlmModel {
    // 合并 name 字段
    if should_merge_field(fields, &SupplementField::Name) {
        if let Some(ref name) = supplement.name {
            if !name.trim().is_empty() {
                model.name = name.clone();
            }
        }
    }

    // 合并 context_length 字段
    if should_merge_field(fields, &SupplementField::ContextLength) {
        if let Some(context_length) = supplement.context_length {
            model.context_length = Some(context_length);
        }
    }

    // 合并 output_max_tokens 字段
    if should_merge_field(fields, &SupplementField::OutputMaxTokens) {
        if let Some(output_max_tokens) = supplement.output_max_tokens {
            model.output_max_tokens = Some(output_max_tokens);
        }
    }

    // 合并 description 字段
    if should_merge_field(fields, &SupplementField::Description) {
        if let Some(ref description) = supplement.description {
            if !description.trim().is_empty() {
                model.description = Some(description.clone());
            }
        }
    }

    // 合并 url 字段
    if should_merge_field(fields, &SupplementField::Url) {
        if let Some(ref url) = supplement.url {
            if !url.trim().is_empty() {
                model.url = Some(url.clone());
            }
        }
    }

    // 合并定价信息
    if should_merge_field(fields, &SupplementField::Pricing) {
        let currency = supplement.currency.as_deref().or(Some("USD"));
        merge_pricing(
            &mut model.pricing,
            supplement.input_cost,
            supplement.output_cost,
            currency,
        );
    }

    // 合并特性列表
    if should_merge_field(fields, &SupplementField::SupportedFeatures) {
        if let Some(ref features) = supplement.supported_features {
            if !features.is_empty() {
                model.supported_features = Some(features.clone());
            }
        }
    }

    // 合并输入模态
    if should_merge_field(fields, &SupplementField::InputModalities) {
        if let Some(ref modalities) = supplement.input_modalities {
            if !modalities.is_empty() {
                model.input_modalities = Some(modalities.clone());
            }
        }
    }

    // 合并输出模态
    if should_merge_field(fields, &SupplementField::OutputModalities) {
        if let Some(ref modalities) = supplement.output_modalities {
            if !modalities.is_empty() {
                model.output_modalities = Some(modalities.clone());
            }
        }
    }

    // 合并支持的参数
    if should_merge_field(fields, &SupplementField::SupportParameters) {
        if !supplement.support_parameters.is_empty() {
            model.support_parameters = supplement.support_parameters.clone();
        }
    }

    // 合并默认参数
    if should_merge_field(fields, &SupplementField::DefaultParameters) {
        if let Some(ref params) = supplement.default_parameters {
            if !params.is_empty() {
                model.default_parameters = Some(params.clone());
            }
        }
    }

    // 合并最大参数
    if should_merge_field(fields, &SupplementField::MaxParameters) {
        if let Some(ref params) = supplement.max_parameters {
            if !params.is_empty() {
                model.max_parameters = Some(params.clone());
            }
        }
    }

    // 合并支持的方法
    if should_merge_field(fields, &SupplementField::SupportedMethods) {
        if let Some(ref methods) = supplement.supported_methods {
            if !methods.is_empty() {
                model.supported_methods = Some(methods.clone());
            }
        } else if !supplement.endpoints.is_empty() {
            let provider_prefix = provider_type.to_lowercase();
            let methods = convert_endpoints_to_methods(&supplement.endpoints, &provider_prefix);
            model.supported_methods = Some(methods);
        }
    }

    // 合并元数据
    if should_merge_field(fields, &SupplementField::Metadata) {
        let mut metadata_map = supplement
            .metadata
            .as_ref()
            .and_then(|value| value.as_object().cloned())
            .or_else(|| {
                model
                    .metadata
                    .as_ref()
                    .and_then(|value| value.as_object().cloned())
            })
            .unwrap_or_default();

        // 添加 endpoints 到 metadata
        if !supplement.endpoints.is_empty() {
            metadata_map.insert(
                "endpoints".to_string(),
                Value::Array(
                    supplement
                        .endpoints
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
        }

        // 添加 URL 到 metadata
        if let Some(ref url_value) = supplement.url {
            metadata_map.insert("url".to_string(), Value::String(url_value.clone()));
        }

        // 添加 currency 到 metadata
        if let Some(ref currency_value) = supplement.currency {
            metadata_map.insert(
                "currency".to_string(),
                Value::String(currency_value.clone()),
            );
        }

        if !metadata_map.is_empty() {
            model.metadata = Some(Value::Object(metadata_map));
        }
    }

    model
}
/// 模型 API 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmModelApiType {
    OpenAI,
    Google,
    Anthropic,
    OpenRouter,
}

impl LlmModelApiType {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmModelApiType::OpenAI => "openai",
            LlmModelApiType::Google => "google",
            LlmModelApiType::Anthropic => "anthropic",
            LlmModelApiType::OpenRouter => "openrouter",
        }
    }
}

impl Display for LlmModelApiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LlmModelApiType {
    type Err = LlmClientError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai" => Ok(LlmModelApiType::OpenAI),
            "google" => Ok(LlmModelApiType::Google),
            "anthropic" => Ok(LlmModelApiType::Anthropic),
            "openrouter" => Ok(LlmModelApiType::OpenRouter),
            other => Err(LlmClientError::validation(format!(
                "Unsupported model API type: {}",
                other
            ))),
        }
    }
}

impl TryFrom<&str> for LlmModelApiType {
    type Error = LlmClientError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        LlmModelApiType::from_str(value)
    }
}
