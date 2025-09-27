// 消息相关数据模型

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::chat::{Timestamp, UUID};
use crate::llm_client::types::ChatToolCallDelta;

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
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
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<String>>,
}

/// 消息工具数据 - 直接存储 DeltaToolCall 对象
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageTools {
    // 待执行的 MCP 调用信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_mcp_call: Option<PendingMcpCall>,

    // 工具调用增量数据 - 直接存储模型返回的 DeltaToolCall
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_deltas: Option<Vec<ChatToolCallDelta>>,
}


/// 消息实体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: UUID,
    pub chat_id: UUID,
    pub role: MessageRole,
    pub content: String,
    pub reasoning: Option<String>, // 推理过程内容

    // Per-message configuration stored as JSON
    pub config: Option<MessageConfig>,

    // Tool-related data stored as JSON
    pub tools: Option<MessageTools>,

    pub attachments: Option<Vec<MessageAttachment>>,

    // Usage and timing information
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub start_time: Option<Timestamp>,
    pub end_time: Option<Timestamp>,
    pub duration: Option<i64>,

    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 聊天消息（请求中使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub reasoning: Option<String>,
}

/// 消息请求附件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequestAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// 消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRequest {
    pub chat_id: Option<UUID>,
    pub model_id: String,
    pub provider_id: String,
    pub messages: Vec<ChatMessage>,
    pub attachments: Option<Vec<MessageRequestAttachment>>,
}

/// 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub chat_id: UUID,
    pub message_id: UUID,
    pub content: String,
    pub reasoning: Option<String>, // 推理过程内容
    pub model_id: String,
    pub provider_id: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_mcp_call: Option<PendingMcpCall>,
}

/// 待执行的 MCP 调用信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingMcpCall {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub tool_calls: Vec<PendingMcpToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingMcpToolCall {
    pub call_id: String,
    pub server_id: String,
    pub server_name: String,
    pub server_display_name: Option<String>,
    pub tool_name: String,
    pub tool_description: Option<String>,
    pub arguments: Value,
}

/// 流式消息事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum MessageStreamEvent {
    #[serde(rename = "delta")]
    Delta {
        content: String,
        reasoning: Option<String>,
        tokens: Option<i32>,
    },
    #[serde(rename = "done")]
    Done(MessageResponse),
    #[serde(rename = "error")]
    Error { error: String, code: Option<String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_roundtrip_preserves_fields() {
        let message = Message {
            id: "msg_123".to_string(),
            chat_id: "chat_456".to_string(),
            role: MessageRole::User,
            content: "Hello, world!".to_string(),
            reasoning: None,
            config: None,
            tools: None,
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
    fn message_with_attachments_roundtrip() {
        let attachment = MessageAttachment {
            id: "att_123".to_string(),
            name: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 1024,
            path: "/tmp/test.txt".to_string(),
        };

        let message = Message {
            id: "msg_123".to_string(),
            chat_id: "chat_456".to_string(),
            role: MessageRole::User,
            content: "Here's a file".to_string(),
            reasoning: None,
            config: None,
            tools: None,
            attachments: Some(vec![attachment]),
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: 1000,
            updated_at: 1000,
        };

        let json = serde_json::to_string(&message).expect("serialize");
        let deserialized: Message = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.attachments.unwrap().len(), 1);
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
            mcp_servers: Some(vec!["server1".to_string()]),
        };

        let json = serde_json::to_string(&config).expect("serialize config");
        let deserialized: MessageConfig = serde_json::from_str(&json).expect("deserialize config");

        assert_eq!(config.temperature, deserialized.temperature);
        assert_eq!(config.model_id, deserialized.model_id);
    }

    #[test]
    fn message_response_serialization_roundtrip() {
        let response = MessageResponse {
            chat_id: "chat_123".to_string(),
            message_id: "msg_456".to_string(),
            content: "Hello! How can I help you?".to_string(),
            reasoning: Some("I need to be helpful and friendly".to_string()),
            model_id: "gpt-4".to_string(),
            provider_id: "openai".to_string(),
            input_tokens: Some(15),
            output_tokens: Some(20),
            total_tokens: Some(35),
            duration: Some(1500),
            pending_mcp_call: None,
        };

        let json = serde_json::to_string(&response).expect("serialize response");
        let deserialized: MessageResponse =
            serde_json::from_str(&json).expect("deserialize response");

        assert_eq!(response.model_id, deserialized.model_id);
        assert_eq!(response.provider_id, deserialized.provider_id);
        assert_eq!(response.total_tokens, deserialized.total_tokens);
    }
}
