// 聊天相关数据模型

use serde::{Deserialize, Serialize};

pub type UUID = String;
pub type Timestamp = i64;

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


#[cfg(test)]
#[path = "chat_test.rs"]
mod chat_test;