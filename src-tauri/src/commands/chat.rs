// 聊天相关 IPC 命令

use crate::models::{ApiResponse, AppError, ChatRequest, ChatResponse, ChatSession, Message, UUID};
use crate::services::ChatService;
use tauri::State;

/// 发送聊天消息
#[tauri::command]
pub async fn chat_send(
    request: ChatRequest,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<ChatResponse>, String> {
    match chat_service.send_message(request).await {
        Ok(response) => Ok(ApiResponse::success(response)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 创建新的聊天会话
#[tauri::command]
pub async fn chat_create_session(
    name: Option<String>,
    config: Option<serde_json::Value>,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<ChatSession>, String> {
    match chat_service.create_session(name, config).await {
        Ok(session) => Ok(ApiResponse::success(session)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 获取会话列表
#[tauri::command]
pub async fn chat_list_sessions(
    limit: Option<i32>,
    offset: Option<i32>,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<Vec<ChatSession>>, String> {
    match chat_service.list_sessions(limit, offset).await {
        Ok(sessions) => Ok(ApiResponse::success(sessions)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 获取会话详情
#[tauri::command]
pub async fn chat_get_session(
    session_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<ChatSession>, String> {
    match chat_service.get_session(session_id).await {
        Ok(session) => Ok(ApiResponse::success(session)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 更新会话
#[tauri::command]
pub async fn chat_update_session(
    session_id: UUID,
    updates: serde_json::Value,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<ChatSession>, String> {
    match chat_service.update_session(session_id, updates).await {
        Ok(session) => Ok(ApiResponse::success(session)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 删除会话
#[tauri::command]
pub async fn chat_delete_session(
    session_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<()>, String> {
    match chat_service.delete_session(session_id).await {
        Ok(_) => Ok(ApiResponse::success(())),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 获取会话消息
#[tauri::command]
pub async fn chat_get_messages(
    session_id: UUID,
    limit: Option<i32>,
    offset: Option<i32>,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<Vec<Message>>, String> {
    match chat_service.get_messages(session_id, limit, offset).await {
        Ok(messages) => Ok(ApiResponse::success(messages)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 更新消息
#[tauri::command]
pub async fn chat_update_message(
    message_id: UUID,
    content: String,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<Message>, String> {
    match chat_service.update_message(message_id, content).await {
        Ok(message) => Ok(ApiResponse::success(message)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 删除消息
#[tauri::command]
pub async fn chat_delete_message(
    message_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<()>, String> {
    match chat_service.delete_message(message_id).await {
        Ok(_) => Ok(ApiResponse::success(())),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}

/// 重新生成助手消息
#[tauri::command]
pub async fn chat_regenerate_message(
    message_id: UUID,
    chat_service: State<'_, ChatService>,
) -> Result<ApiResponse<ChatResponse>, String> {
    match chat_service.regenerate_message(message_id).await {
        Ok(response) => Ok(ApiResponse::success(response)),
        Err(error) => Ok(ApiResponse::error(error)),
    }
}
