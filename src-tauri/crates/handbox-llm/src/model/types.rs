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
    pub output_token_limit: Option<i32>,
    pub input_cost: Option<f32>,
    pub output_cost: Option<f32>,
    pub supported_features: Option<Vec<LlmModelFeature>>,
    pub description: Option<String>,
    pub input_modalities: Option<Vec<LlmModelModality>>,
    pub output_modalities: Option<Vec<LlmModelModality>>,
    pub metadata: Option<Value>,
    pub pricing: Option<Value>,
    pub support_parameters: Vec<LlmModelParameter>,
    pub default_parameters: Option<HashMap<String, Value>>,
    pub max_parameters: Option<HashMap<String, Value>>,
}

/// 模型功能枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum LlmModelFeature {
    Reasoning,
    Tool,
}

/// 模型模态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LlmModelModality {
    Text,
    Image,
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
        } else if value.eq_ignore_ascii_case("image") {
            Ok(LlmModelModality::Image)
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

/// 模型 API 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmModelApiType {
    OpenAI,
    OpenAIWithLocal,
    Google,
    Anthropic,
    OpenRouter,
}

impl LlmModelApiType {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmModelApiType::OpenAI => "openai",
            LlmModelApiType::OpenAIWithLocal => "openai+local",
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
            "openai+local" => Ok(LlmModelApiType::OpenAIWithLocal),
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
