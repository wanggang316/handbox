// 消息相关数据模型

use serde::{Deserialize, Serialize};

use crate::models::chat::{Timestamp, UUID};

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
#[path = "message_test.rs"]
mod message_test;
