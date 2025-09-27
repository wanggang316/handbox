use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;

use crate::models::AppError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_deltas: Option<Vec<ChatToolCallDelta>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
}

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
    pub message: Option<ChatMessage>,
    pub delta: Option<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls_delta: Option<Vec<ChatToolCallDelta>>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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
