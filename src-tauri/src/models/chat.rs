// 聊天相关数据模型

use serde::{Deserialize, Serialize};

pub type UUID = String;
pub type Timestamp = i64;

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

/// 消息元数据（简化版，主要信息已移到 Message 结构体中）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub streaming: Option<bool>,
    pub extra: Option<serde_json::Value>, // 用于存储其他元数据
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

/// 模型参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelParameters {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub context_length: Option<i32>,
    pub stream: Option<bool>,
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            context_length: Some(4096),
            stream: Some(true),
        }
    }
}

/// 聊天配置（简化版，模型信息移到消息级别）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<String>,
    pub default_parameters: Option<ModelParameters>, // 默认参数，可在消息级别覆盖
}

/// 聊天实体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: UUID,
    pub name: String,
    pub last_message_at: Option<Timestamp>,
    pub message_count: i32,
    
    // Chat-level configuration (default values)
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Vec<String>,
    
    pub artifact_id: Option<UUID>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}


/// 聊天请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub chat_id: Option<UUID>,
    pub artifact_id: Option<UUID>,
    pub model_id: String,
    pub provider_id: String,
    pub parameters: Option<ModelParameters>,
    pub messages: Vec<ChatMessage>,
    pub attachments: Option<Vec<ChatAttachment>>,
}

/// 聊天消息（请求中使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

/// 聊天附件（请求中使用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// 聊天响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub chat_id: UUID,
    pub message_id: UUID,
    pub content: String,
    pub model_id: String,
    pub provider_id: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub duration: Option<i64>,
}

/// 流式聊天事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ChatStreamEvent {
    #[serde(rename = "delta")]
    Delta {
        content: String,
        tokens: Option<i32>,
    },
    #[serde(rename = "done")]
    Done(ChatResponse),
    #[serde(rename = "error")]
    Error { error: String, code: Option<String> },
}

#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;
