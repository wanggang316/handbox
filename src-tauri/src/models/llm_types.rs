//! Local definitions of the leaf types HandBox's storage and chat dispatch
//! consume. Originally re-exported from the `handbox-llm` crate (M1-T1 →
//! M2-T5.1); inlined here at M3-T0 so the upstream crate can be deleted at
//! M3-T1 without affecting consumers. Every serde representation is
//! byte-identical to the upstream definition at the time of copy —
//! deliberate forward changes to the wire format must be made carefully
//! to preserve DB compatibility.

use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

// Verbatim copy from handbox-llm/src/chat/types.rs:14-20; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 消息附件（图片、文件等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessageAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:22-34; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 通用-消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: LlmMessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<LlmMessageAttachment>>,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:37-44; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 通用-工具调用信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmToolFunction,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:47-52; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 通用-工具函数信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolFunction {
    pub name: String,
    pub arguments: String,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:54-93; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
// Note: per M1-T1's decision, `FromStr::Err` is `String` (not the upstream
// `LlmClientError`) — the error type is local to HandBox and does not affect
// the serde wire format.
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

// Verbatim copy from handbox-llm/src/chat/types.rs:146-150; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 生成的图片数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGeneratedImage {
    pub mime_type: String, // e.g., "image/png", "image/jpeg"
    pub data: String,      // Base64-encoded image data
}

// Verbatim copy from handbox-llm/src/chat/types.rs:229-237; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// Responses API 推理配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmResponsesReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<LlmReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<LlmReasoningSummary>,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:240-247; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 通用的推理强度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:250-256; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// 推理总结级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmReasoningSummary {
    Auto,
    Concise,
    Detailed,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:259-266; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// Completions API 推理配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmReasoningEffortConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<LlmReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_reasoning: Option<bool>,
}

// Verbatim copy from handbox-llm/src/chat/types.rs:269-276; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
/// Google 思维配置
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
}

// Verbatim copy from handbox-llm/src/model/types.rs:30-35; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelPricing {
    pub currency: Option<String>,
    pub input_text: Option<f32>,
    pub output_text: Option<f32>,
}

// Verbatim copy from handbox-llm/src/model/types.rs:76-165; serde repr is
// DB-bound. The originating crate is deleted in M3-T1.
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
