use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

use crate::models::AppError;

/// 通用-消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatMessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// 工具调用执行模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ToolExecutionMode {
    Auto,
    Manual,
}

impl Default for ToolExecutionMode {
    fn default() -> Self {
        ToolExecutionMode::Auto
    }
}

/// 工具调用执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Pending,    // 待执行
    Executing,  // 执行中
    Completed,  // 已执行
    Failed,     // 执行错误
}

impl Default for ToolExecutionStatus {
    fn default() -> Self {
        ToolExecutionStatus::Pending
    }
}

/// 通用-工具调用信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatToolFunction,
    #[serde(default)]
    pub execution_mode: ToolExecutionMode,
    #[serde(default)]
    pub execution_status: ToolExecutionStatus,
}

/// 通用-工具函数信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatToolFunction {
    pub name: String,
    pub arguments: String,
}

/// 聊天消息角色枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ChatMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatMessageRole::System => "system",
            ChatMessageRole::User => "user",
            ChatMessageRole::Assistant => "assistant",
            ChatMessageRole::Tool => "tool",
        }
    }
}

impl std::fmt::Display for ChatMessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ChatMessageRole {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(ChatMessageRole::System),
            "user" => Ok(ChatMessageRole::User),
            "assistant" => Ok(ChatMessageRole::Assistant),
            "tool" => Ok(ChatMessageRole::Tool),
            _ => Err(AppError::validation_error(&format!("Invalid role: {}", s))),
        }
    }
}

// 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<RequestTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "required")]
    Required,
}

// 请求-工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: RequestToolFunction,
}

// 请求-工具函数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

// 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: i32,
    pub delta: Option<ChatMessage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

// 响应-增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunkResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<ChatChunkChoice>,
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunkChoice {
    pub index: i32,
    pub delta: Option<ChatDeltaMessage>,
    pub finish_reason: Option<String>,
}

// 响应-消息-增量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDeltaMessage {
    pub role: Option<ChatMessageRole>,
    pub content: Option<String>,
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatDeltaToolCall>>,
}

/// 工具调用-增量
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatDeltaToolCall {
    pub index: u32,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub tool_type: Option<String>,
    pub function: Option<ChatDeltaToolFunction>,
}

// Type alias for backward compatibility
pub type ChatToolCallDelta = ChatDeltaToolCall;

/// 工具函数-增量
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatDeltaToolFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelFeature {
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "completion")]
    Completion,
    #[serde(rename = "embedding")]
    Embedding,
    #[serde(rename = "function_calling")]
    FunctionCalling,
    #[serde(rename = "vision")]
    Vision,
    #[serde(rename = "streaming")]
    Streaming,
}

#[derive(Debug, Clone)]
pub struct StandardModel {
    pub id: String,
    pub name: String,
    pub context_length: Option<i32>,
    pub input_cost: Option<f32>,
    pub output_cost: Option<f32>,
    pub supported_features: Option<Vec<ModelFeature>>,
}

/// 聊天 API 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatApiType {
    OpenAICompletions,
    OpenAIResponses,
    Google,
    Anthropic,
}

impl ChatApiType {
    pub fn as_str(self) -> &'static str {
        match self {
            ChatApiType::OpenAICompletions => "openai-completions",
            ChatApiType::OpenAIResponses => "openai-responses",
            ChatApiType::Google => "google",
            ChatApiType::Anthropic => "anthropic",
        }
    }
}

impl fmt::Display for ChatApiType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ChatApiType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai" | "openai-completions" => Ok(ChatApiType::OpenAICompletions),
            "openai-responses" => Ok(ChatApiType::OpenAIResponses),
            "google" => Ok(ChatApiType::Google),
            "anthropic" => Ok(ChatApiType::Anthropic),
            other => Err(AppError::validation_error(&format!(
                "Unsupported chat API type: {}",
                other
            ))),
        }
    }
}

impl TryFrom<&str> for ChatApiType {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        ChatApiType::from_str(value)
    }
}

/// 模型 API 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelApiType {
    OpenAI,
    OpenAIWithLocal,
    Google,
    Anthropic,
    OpenRouter,
}

impl ModelApiType {
    pub fn as_str(self) -> &'static str {
        match self {
            ModelApiType::OpenAI => "openai",
            ModelApiType::OpenAIWithLocal => "openai+local",
            ModelApiType::Google => "google",
            ModelApiType::Anthropic => "anthropic",
            ModelApiType::OpenRouter => "openrouter",
        }
    }
}

impl fmt::Display for ModelApiType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ModelApiType {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai" => Ok(ModelApiType::OpenAI),
            "openai+local" => Ok(ModelApiType::OpenAIWithLocal),
            "google" => Ok(ModelApiType::Google),
            "anthropic" => Ok(ModelApiType::Anthropic),
            "openrouter" => Ok(ModelApiType::OpenRouter),
            other => Err(AppError::validation_error(&format!(
                "Unsupported model API type: {}",
                other
            ))),
        }
    }
}

impl TryFrom<&str> for ModelApiType {
    type Error = AppError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        ModelApiType::from_str(value)
    }
}
