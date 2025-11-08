use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt::Display, str::FromStr};

use crate::error::LlmClientError;

/// Minimal provider context required by the LLM client layer.
#[derive(Debug, Clone)]
pub struct LlmProvider {
    pub base_url: String,
    pub api_key: String,
}

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
}

/// 通用-工具调用信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmToolFunction,
}

/// 通用-工具函数信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolFunction {
    pub name: String,
    pub arguments: String,
}

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
    type Err = LlmClientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(LlmMessageRole::System),
            "user" => Ok(LlmMessageRole::User),
            "assistant" => Ok(LlmMessageRole::Assistant),
            "tool" => Ok(LlmMessageRole::Tool),
            _ => Err(LlmClientError::validation(format!("Invalid role: {}", s))),
        }
    }
}

// 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<LlmRequestTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<LlmToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "required")]
    Required,
}

// 请求-工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmRequestToolFunction,
}

// 请求-工具函数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequestToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

// 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<LlmChoice>,
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChoice {
    pub index: i32,
    pub delta: Option<LlmMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

// 响应-增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChunkResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<LlmChunkChoice>,
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChunkChoice {
    pub index: i32,
    pub delta: Option<LlmDeltaMessage>,
    pub finish_reason: Option<String>,
}

// 响应-消息-增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmDeltaMessage {
    pub role: Option<LlmMessageRole>,
    pub content: Option<String>,
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmDeltaToolCall>>,
}

/// 工具调用-增量
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmDeltaToolCall {
    pub index: u32,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub function: Option<LlmDeltaToolFunction>,
}

// Type alias for backward compatibility
pub type LlmToolCallDelta = LlmDeltaToolCall;

/// 工具函数-增量
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmDeltaToolFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// 聊天 API 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmApiType {
    OpenAICompletions,
    OpenAIResponses,
    Google,
    Anthropic,
}

impl LlmApiType {
    pub fn as_str(self) -> &'static str {
        match self {
            LlmApiType::OpenAICompletions => "openai-completions",
            LlmApiType::OpenAIResponses => "openai-responses",
            LlmApiType::Google => "google",
            LlmApiType::Anthropic => "anthropic",
        }
    }
}

impl Display for LlmApiType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LlmApiType {
    type Err = LlmClientError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai" | "openai-completions" => Ok(LlmApiType::OpenAICompletions),
            "openai-responses" => Ok(LlmApiType::OpenAIResponses),
            "google" => Ok(LlmApiType::Google),
            "anthropic" => Ok(LlmApiType::Anthropic),
            other => Err(LlmClientError::validation(format!(
                "Unsupported chat API type: {}",
                other
            ))),
        }
    }
}

impl TryFrom<&str> for LlmApiType {
    type Error = LlmClientError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        LlmApiType::from_str(value)
    }
}
