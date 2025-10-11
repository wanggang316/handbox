// 消息相关数据模型

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::storage::types::UUID;
use handbox_llm::types::{LlmMessage, LlmToolCall};

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
    pub messages: Vec<LlmMessage>,
    pub temp_user_message_id: Option<String>,
    pub attachments: Option<Vec<MessageRequestAttachment>>,
}

/// 用户消息流式发送请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessageSendRequest {
    pub chat_id: UUID,
    pub content: String,
    pub temp_user_message_id: String,
    pub attachments: Option<Vec<MessageRequestAttachment>>,
}

/// 流式消息块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub stream_id: String,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<LlmToolCall>>,
}

/// 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub chat_id: UUID,
    pub message_id: UUID,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    pub model_id: String,
    pub provider_id: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub duration: Option<i64>,
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
}
