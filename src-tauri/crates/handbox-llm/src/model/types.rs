use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use crate::error::LlmClientError;

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
    pub support_parameters: Vec<LlmModelParameter>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
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
        } else if value.eq_ignore_ascii_case("image")
            || value.eq_ignore_ascii_case("images")
        {
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
    pub url: Option<String>,
}

impl ModelSupplement {
    pub fn into_snapshot_models(self) -> Vec<(String, LlmModel)> {
        let ModelSupplement {
            model_code,
            name,
            context_length,
            output_max_tokens,
            input_cost,
            output_cost,
            supported_features,
            description,
            input_modalities,
            output_modalities,
            metadata,
            currency,
            support_parameters,
            default_parameters,
            max_parameters,
            snapshots,
            endpoints,
            url,
        } = self;

        let snapshot_ids = if snapshots.is_empty() {
            vec![model_code.clone()]
        } else {
            snapshots.clone()
        };

        let mut results = Vec::with_capacity(snapshot_ids.len());

        for snapshot in snapshot_ids.iter() {
            let snapshot_id = snapshot.clone();
            let mut metadata_map = metadata
                .as_ref()
                .and_then(|value| value.as_object().cloned())
                .unwrap_or_default();

            if let Some(url_value) = &url {
                metadata_map.insert("url".to_string(), Value::String(url_value.clone()));
            }

            if !snapshots.is_empty() {
                metadata_map.insert(
                    "snapshots".to_string(),
                    Value::Array(
                        snapshots
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    ),
                );
            }

            if !endpoints.is_empty() {
                metadata_map.insert(
                    "endpoints".to_string(),
                    Value::Array(endpoints.iter().cloned().map(Value::String).collect()),
                );
            }

            if !snapshot_ids.is_empty() {
                metadata_map.insert(
                    "resolved_snapshot".to_string(),
                    Value::String(snapshot_id.clone()),
                );
            }

            if let Some(currency_value) = currency.as_ref() {
                metadata_map.insert(
                    "currency".to_string(),
                    Value::String(currency_value.clone()),
                );
            }

            metadata_map.insert(
                "model_code".to_string(),
                Value::String(model_code.clone()),
            );

            let mut model = LlmModel {
                id: snapshot_id.clone(),
                name: name
                    .clone()
                    .unwrap_or_else(|| snapshot_id.clone()),
                context_length,
                output_max_tokens,
                supported_features: supported_features.clone(),
                description: description.clone(),
                input_modalities: input_modalities.clone(),
                output_modalities: output_modalities.clone(),
                metadata: if metadata_map.is_empty() {
                    None
                } else {
                    Some(Value::Object(metadata_map))
                },
                pricing: None,
                support_parameters: support_parameters.clone(),
                default_parameters: default_parameters.clone(),
                max_parameters: max_parameters.clone(),
            };

            merge_pricing(
                &mut model.pricing,
                input_cost,
                output_cost,
                currency.as_deref().or(Some("USD")),
            );

            results.push((snapshot_id, model));
        }

        results
    }
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

    if current.currency.is_none() && current.input_text.is_none() && current.output_text.is_none()
    {
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
