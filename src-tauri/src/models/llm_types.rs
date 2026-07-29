//! Leaf LLM types consumed by HandBox's storage and chat dispatch. The serde
//! representations are persisted to the database; any change to the wire
//! format must preserve compatibility with existing rows.

use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr};

/// Message attachment (images, files, ...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessageAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmToolFunction {
    pub name: String,
    pub arguments: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGeneratedImage {
    pub mime_type: String, // e.g., "image/png", "image/jpeg"
    pub data: String,      // Base64-encoded image data
}

/// Reasoning config for the Responses API.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmResponsesReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<LlmReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<LlmReasoningSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmReasoningSummary {
    Auto,
    Concise,
    Detailed,
}

/// Reasoning config for the Completions API.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmReasoningEffortConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<LlmReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_reasoning: Option<bool>,
}

/// Thinking config for Google models.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmThinkingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_thoughts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelPricing {
    pub currency: Option<String>,
    pub input_text: Option<f32>,
    pub output_text: Option<f32>,
}

/// Parameters a model supports; shared across provider adapters.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmModelParameter {
    Tools,
    ToolChoice,

    MaxTokens,

    Temperature,
    TopP,
    TopK,

    Reasoning,
    IncludeReasoning,

    StructuredOutputs,
    ResponseFormat,

    Stop,

    FrequencyPenalty,
    PresencePenalty,

    Seed,

    #[serde(other)]
    Unknown,
}

impl LlmModelParameter {
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

// Defined here rather than in `storage::types` so the agent stack can use the
// reasoning config without depending on the chat-session module.
/// Reasoning config for OpenRouter.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LlmOpenrouterReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<bool>,
}

/// Per-session reasoning config; one optional slot per provider API style.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionReasoningConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<LlmResponsesReasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<LlmReasoningEffortConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<LlmThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openrouter: Option<LlmOpenrouterReasoning>,
}
