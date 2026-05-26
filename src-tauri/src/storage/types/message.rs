use super::session::SessionReasoningConfig;
use super::common::{Timestamp, UUID};
use crate::storage::types::McpServerConfig;
use crate::models::llm_types::{LlmMessageRole, LlmToolFunction};

use serde::{Deserialize, Serialize};

/// 工具调用执行模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageToolExecutionMode {
    Auto,
    Manual,
}

impl Default for MessageToolExecutionMode {
    fn default() -> Self {
        MessageToolExecutionMode::Auto
    }
}

/// 工具调用执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageToolExecutionStatus {
    Pending,   // 待执行
    Executing, // 执行中
    Completed, // 已执行
    Failed,    // 执行错误
}

impl Default for MessageToolExecutionStatus {
    fn default() -> Self {
        MessageToolExecutionStatus::Pending
    }
}

/// 业务层工具调用信息（包含执行相关的业务字段）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmToolFunction,
    #[serde(default)]
    pub execution_mode: MessageToolExecutionMode,
    #[serde(default)]
    pub execution_status: MessageToolExecutionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

impl From<crate::models::llm_types::LlmToolCall> for MessageToolCall {
    fn from(llm_call: crate::models::llm_types::LlmToolCall) -> Self {
        MessageToolCall {
            id: llm_call.id,
            tool_type: llm_call.tool_type,
            function: llm_call.function,
            execution_mode: MessageToolExecutionMode::default(),
            execution_status: MessageToolExecutionStatus::default(),
            result: None,
        }
    }
}

impl MessageToolCall {
    /// 转换为 LLM 层的 ToolCall（移除业务字段）
    pub fn to_llm_tool_call(&self) -> crate::models::llm_types::LlmToolCall {
        crate::models::llm_types::LlmToolCall {
            id: self.id.clone(),
            tool_type: self.tool_type.clone(),
            function: self.function.clone(),
        }
    }
}

/// 消息附件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageAttachment {
    pub id: UUID,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub path: String,
}

/// 消息配置 - 每条消息可以有独立的配置参数
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<SessionReasoningConfig>,
}

/// 消息实体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: UUID,
    pub session_id: UUID,
    pub role: LlmMessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<MessageToolCall>>,
    pub turn_id: Option<i32>,
    pub tool_call_id: Option<String>,
    pub config: Option<MessageConfig>,
    pub attachments: Option<Vec<MessageAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_assets: Option<Vec<MessageAttachment>>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub start_time: Option<Timestamp>,
    pub end_time: Option<Timestamp>,
    pub duration: Option<i64>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::llm_types::LlmMessageRole;

    #[test]
    fn message_roundtrip_preserves_fields() {
        let message = Message {
            id: "msg_123".to_string(),
            session_id: "chat_456".to_string(),
            role: LlmMessageRole::User,
            content: "Hello".to_string(),
            reasoning: None,
            tool_calls: None,
            turn_id: Some(1),
            tool_call_id: None,
            config: None,
            attachments: None,
            generated_assets: None,
            input_tokens: Some(10),
            output_tokens: Some(20),
            total_tokens: Some(30),
            start_time: Some(1000),
            end_time: Some(2000),
            duration: Some(1000),
            created_at: 1000,
            updated_at: 2000,
        };

        let json = serde_json::to_string(&message).expect("serialize message");
        let deserialized: Message = serde_json::from_str(&json).expect("deserialize message");

        assert_eq!(message.id, deserialized.id);
        assert_eq!(message.session_id, deserialized.session_id);
        assert_eq!(message.content, deserialized.content);
    }

    #[test]
    fn message_config_serialization_roundtrip() {
        let config = MessageConfig {
            temperature: Some(0.8),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(1000),
            stream: Some(true),
            model_id: Some("gpt-4".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a helpful assistant".to_string()),
            mcp_servers: Some(vec![crate::storage::types::McpServerConfig {
                server_id: "server1".to_string(),
                execution_mode: "auto".to_string(),
                enabled_tools: vec!["tool1".to_string()],
            }]),
            turn_count: Some(5),
            reasoning: None,
        };

        let json = serde_json::to_string(&config).expect("serialize config");
        let deserialized: MessageConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(config.temperature, deserialized.temperature);
        assert_eq!(config.model_id, deserialized.model_id);
        assert_eq!(config.turn_count, deserialized.turn_count);
        assert_eq!(config.top_k, deserialized.top_k);
    }
}
