// 聊天相关数据模型

use serde::{Deserialize, Serialize};

pub type UUID = String;
pub type Timestamp = i64;

/// 消息角色
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// 消息附件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAttachment {
    pub id: UUID,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub path: String,
}

/// 消息元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub tokens: Option<TokenUsage>,
    pub timing: Option<TimingInfo>,
    pub streaming: Option<bool>,
}

/// Token 使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: i32,
    pub output: i32,
    pub total: i32,
}

/// 时序信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingInfo {
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub duration: i64,
}

/// 消息实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: UUID,
    pub session_id: UUID,
    pub role: MessageRole,
    pub content: String,
    pub attachments: Option<Vec<MessageAttachment>>,
    pub metadata: Option<MessageMetadata>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 模型参数
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 聊天配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatConfig {
    pub system_prompt: Option<String>,
    pub model: String,
    pub provider: String,
    pub parameters: ModelParameters,
    pub mcp_servers: Vec<String>,
}

/// 聊天会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: UUID,
    pub name: String,
    pub last_message_at: Option<Timestamp>,
    pub message_count: i32,
    pub config: ChatConfig,
    pub artifact_id: Option<UUID>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// 聊天请求
#[derive(Debug, Clone, Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<UUID>,
    pub artifact_id: Option<UUID>,
    pub inline_config: Option<ChatConfig>,
    pub messages: Vec<ChatMessage>,
    pub attachments: Option<Vec<ChatAttachment>>,
}

/// 聊天消息（请求中使用）
#[derive(Debug, Clone, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

/// 聊天附件（请求中使用）
#[derive(Debug, Clone, Deserialize)]
pub struct ChatAttachment {
    pub name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// 聊天响应
#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub session_id: UUID,
    pub message_id: UUID,
    pub content: String,
    pub metadata: MessageMetadata,
}

/// 流式聊天事件
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ChatStreamEvent {
    #[serde(rename = "delta")]
    Delta {
        content: String,
        metadata: Option<MessageMetadata>,
    },
    #[serde(rename = "done")]
    Done(ChatResponse),
    #[serde(rename = "error")]
    Error { error: String, code: Option<String> },
}
