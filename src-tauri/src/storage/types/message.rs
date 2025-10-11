use super::common::{Timestamp, UUID};
use crate::storage::types::McpServerConfig;
use handbox_llm::{types::LlmMessageRole, LlmToolCall};

use serde::{Deserialize, Serialize};

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
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<McpServerConfig>>,
    pub turn_count: Option<i32>,
}

/// 消息实体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: UUID,
    pub chat_id: UUID,
    pub role: LlmMessageRole,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
    pub turn_id: Option<i32>,
    pub tool_call_id: Option<String>,
    pub config: Option<MessageConfig>,
    pub attachments: Option<Vec<MessageAttachment>>,
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
    use handbox_llm::types::LlmMessageRole;

    #[test]
    fn message_roundtrip_preserves_fields() {
        let message = Message {
            id: "msg_123".to_string(),
            chat_id: "chat_456".to_string(),
            role: LlmMessageRole::User,
            content: "Hello".to_string(),
            reasoning: None,
            tool_calls: None,
            turn_id: Some(1),
            tool_call_id: None,
            config: None,
            attachments: None,
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
        assert_eq!(message.chat_id, deserialized.chat_id);
        assert_eq!(message.content, deserialized.content);
    }

    #[test]
    fn message_config_serialization_roundtrip() {
        let config = MessageConfig {
            temperature: Some(0.8),
            top_p: Some(0.9),
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
        };

        let json = serde_json::to_string(&config).expect("serialize config");
        let deserialized: MessageConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(config.temperature, deserialized.temperature);
        assert_eq!(config.model_id, deserialized.model_id);
        assert_eq!(config.turn_count, deserialized.turn_count);
    }
}
