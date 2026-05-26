// Local copies of leaf types previously owned by the `handbox-llm` internal
// crate. Per the dissolve-handbox-llm plan (M1-T1), these definitions are
// verbatim copies. Serde representations are DB-bound (HandBox SQLite stores
// serialized JSON of `LlmMessageRole` / `LlmToolCall` in TEXT columns) and
// MUST NOT change. Subsequent tasks (M1-T2 / M1-T3) will rewrite imports to
// point here; M3 will delete the upstream crate.

use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

// Copied from handbox-llm/src/chat/types.rs:54-93.
// Serde repr is DB-bound — must not change.
/// 聊天消息角色枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl LlmMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmMessageRole::System => "system",
            LlmMessageRole::User => "user",
            LlmMessageRole::Assistant => "assistant",
            LlmMessageRole::Tool => "tool",
        }
    }
}

impl Display for LlmMessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for LlmMessageRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(LlmMessageRole::System),
            "user" => Ok(LlmMessageRole::User),
            "assistant" => Ok(LlmMessageRole::Assistant),
            "tool" => Ok(LlmMessageRole::Tool),
            other => Err(format!("Invalid LlmMessageRole: {}", other)),
        }
    }
}

// Copied from handbox-llm/src/chat/types.rs:47-52.
// Serde repr is DB-bound — must not change.
/// 通用-工具函数信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolFunction {
    pub name: String,
    pub arguments: String,
}

// Copied from handbox-llm/src/chat/types.rs:37-44.
// Serde repr is DB-bound — must not change.
/// 通用-工具调用信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmToolFunction,
}

// Copied from handbox-llm/src/chat/types.rs:14-20.
// Serde repr is DB-bound — must not change.
/// 消息附件（图片、文件等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessageAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

// Copied from handbox-llm/src/chat/types.rs:240-247.
// Serde repr is DB-bound — must not change.
/// 通用的推理强度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
}

// Copied from handbox-llm/src/chat/types.rs:250-256.
// Serde repr is DB-bound — must not change.
/// 推理总结级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmReasoningSummary {
    Auto,
    Concise,
    Detailed,
}

// Copied from handbox-llm/src/chat/types.rs:259-266.
// Serde repr is DB-bound — must not change.
/// Completions API 推理配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmReasoningEffortConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<LlmReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_reasoning: Option<bool>,
}

// Copied from handbox-llm/src/chat/types.rs:229-237.
// Serde repr is DB-bound — must not change.
/// Responses API 推理配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmResponsesReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<LlmReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<LlmReasoningSummary>,
}

// Copied from handbox-llm/src/chat/types.rs:269-276.
// Serde repr is DB-bound — must not change.
/// Google 思维配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
}

// Copied from handbox-llm/src/model/types.rs:30-35.
// Serde repr is DB-bound — must not change.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelPricing {
    pub currency: Option<String>,
    pub input_text: Option<f32>,
    pub output_text: Option<f32>,
}

// Copied from handbox-llm/src/model/types.rs:76-165.
// Serde repr is DB-bound — must not change.
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
