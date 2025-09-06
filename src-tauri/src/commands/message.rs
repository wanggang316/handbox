// 消息相关 IPC 命令

use crate::models::{AppError, ChatRequest, ChatResponse, Message, UUID};
use crate::services::MessageService;
use tauri::State;

/// 发送聊天消息
#[tauri::command]
pub async fn message_send(
    request: ChatRequest,
    message_service: State<'_, MessageService>,
) -> Result<ChatResponse, AppError> {
    tracing::info!("[message_send] IPC command called");
    match message_service.send_message(request).await {
        Ok(response) => {
            tracing::info!("[message_send] Command completed successfully");
            Ok(response)
        }
        Err(e) => {
            tracing::error!("[message_send] Command failed: {:?}", e);
            Err(e)
        }
    }
}

/// 获取聊天消息
#[tauri::command]
pub async fn message_list(
    chat_id: UUID,
    limit: Option<i32>,
    offset: Option<i32>,
    message_service: State<'_, MessageService>,
) -> Result<Vec<Message>, AppError> {
    tracing::info!("[message_list] IPC command called for chat_id: {}", chat_id);
    match message_service.get_messages(chat_id, limit, offset).await {
        Ok(messages) => {
            tracing::info!("[message_list] Command completed successfully, returned {} messages", messages.len());
            Ok(messages)
        }
        Err(e) => {
            tracing::error!("[message_list] Command failed: {:?}", e);
            Err(e)
        }
    }
}

/// 获取单条消息
#[tauri::command]
pub async fn message_get(
    message_id: UUID,
    message_service: State<'_, MessageService>,
) -> Result<Message, AppError> {
    tracing::info!("[message_get] IPC command called for message_id: {}", message_id);
    match message_service.get_message(message_id).await {
        Ok(message) => {
            tracing::info!("[message_get] Command completed successfully");
            Ok(message)
        }
        Err(e) => {
            tracing::error!("[message_get] Command failed: {:?}", e);
            Err(e)
        }
    }
}

/// 更新消息
#[tauri::command]
pub async fn message_update(
    message_id: UUID,
    content: String,
    message_service: State<'_, MessageService>,
) -> Result<Message, AppError> {
    tracing::info!("[message_update] IPC command called for message_id: {}", message_id);
    match message_service.update_message(message_id, content).await {
        Ok(message) => {
            tracing::info!("[message_update] Command completed successfully");
            Ok(message)
        }
        Err(e) => {
            tracing::error!("[message_update] Command failed: {:?}", e);
            Err(e)
        }
    }
}

/// 删除消息
#[tauri::command]
pub async fn message_delete(
    message_id: UUID,
    message_service: State<'_, MessageService>,
) -> Result<(), AppError> {
    tracing::info!("[message_delete] IPC command called for message_id: {}", message_id);
    match message_service.delete_message(message_id).await {
        Ok(()) => {
            tracing::info!("[message_delete] Command completed successfully");
            Ok(())
        }
        Err(e) => {
            tracing::error!("[message_delete] Command failed: {:?}", e);
            Err(e)
        }
    }
}

/// 重新生成助手消息
#[tauri::command]
pub async fn message_regenerate(
    message_id: UUID,
    message_service: State<'_, MessageService>,
) -> Result<ChatResponse, AppError> {
    tracing::info!("[message_regenerate] IPC command called for message_id: {}", message_id);
    match message_service.regenerate_message(message_id).await {
        Ok(response) => {
            tracing::info!("[message_regenerate] Command completed successfully");
            Ok(response)
        }
        Err(e) => {
            tracing::error!("[message_regenerate] Command failed: {:?}", e);
            Err(e)
        }
    }
}