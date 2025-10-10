// 聊天相关 IPC 命令

use crate::models::{AppError, Chat, UUID};
use crate::services::ChatService;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateTitleResponse {
    pub title: String,
}

/// 创建新的聊天
#[tauri::command]
pub async fn chat_create(
    name: String,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<i32>,
    stream: Option<bool>,
    model_id: Option<String>,
    provider_id: Option<String>,
    system_prompt: Option<String>,
    mcp_servers: Option<Vec<crate::models::McpServerConfig>>,
    chat_service: State<'_, ChatService>,
) -> Result<Chat, AppError> {
    chat_service
        .create_chat(
            name,
            temperature,
            top_p,
            max_tokens,
            stream,
            model_id,
            provider_id,
            system_prompt,
            mcp_servers,
        )
        .await
}

/// 获取聊天列表
#[tauri::command]
pub async fn chat_list(
    limit: Option<i32>,
    offset: Option<i32>,
    chat_service: State<'_, ChatService>,
) -> Result<Vec<Chat>, AppError> {
    chat_service.list_chats(limit, offset).await
}

/// 获取聊天详情
#[tauri::command]
pub async fn chat_get(
    chat_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<Chat, AppError> {
    chat_service.get_chat(chat_id).await
}

/// 更新聊天
#[tauri::command]
pub async fn chat_update(
    chat_id: UUID,
    name: Option<String>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<i32>,
    stream: Option<bool>,
    model_id: Option<String>,
    provider_id: Option<String>,
    system_prompt: Option<String>,
    mcp_servers: Option<Vec<crate::models::McpServerConfig>>,
    turn_count: Option<i32>,
    chat_service: State<'_, ChatService>,
) -> Result<Chat, AppError> {
    chat_service
        .update_chat(
            chat_id,
            name,
            temperature,
            top_p,
            max_tokens,
            stream,
            model_id,
            provider_id,
            system_prompt,
            mcp_servers,
            turn_count,
        )
        .await
}

/// 删除聊天
#[tauri::command]
pub async fn chat_delete(
    chat_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<(), AppError> {
    chat_service.delete_chat(chat_id).await
}

/// 生成聊天标题
#[tauri::command]
pub async fn chat_generate_title(
    chat_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<GenerateTitleResponse, AppError> {
    let title = chat_service.generate_title(chat_id).await?;
    Ok(GenerateTitleResponse { title })
}
