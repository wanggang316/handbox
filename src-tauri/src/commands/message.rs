// 消息相关 IPC 命令

use crate::models::{AppError, Message, MessageRequest, MessageResponse, UUID};
use crate::services::MessageService;
use serde_json::json;
use tauri::{Emitter, State, Window};

/// 发送聊天消息
#[tauri::command]
pub async fn message_send(
    request: MessageRequest,
    message_service: State<'_, MessageService>,
) -> Result<MessageResponse, AppError> {
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
            tracing::info!(
                "[message_list] Command completed successfully, returned {} messages",
                messages.len()
            );
            tracing::info!("[message_list] Messages: {:?}", messages);
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
    tracing::info!(
        "[message_get] IPC command called for message_id: {}",
        message_id
    );
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
    tracing::info!(
        "[message_update] IPC command called for message_id: {}",
        message_id
    );
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
    tracing::info!(
        "[message_delete] IPC command called for message_id: {}",
        message_id
    );
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
) -> Result<MessageResponse, AppError> {
    tracing::info!(
        "[message_regenerate] IPC command called for message_id: {}",
        message_id
    );
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

/// 发送流式消息
#[tauri::command]
pub async fn message_send_stream(
    request: MessageRequest,
    window: Window,
    message_service: State<'_, MessageService>,
) -> Result<String, AppError> {
    tracing::info!("[message_send_stream] IPC command called");

    // 生成流式消息ID
    let stream_id = uuid::Uuid::new_v4().to_string();

    // 克隆必要的数据和窗口引用
    let window_clone = window.clone();
    let stream_id_clone = stream_id.clone();
    let request_clone = request.clone();
    let service_clone = message_service.inner().clone();

    // 在后台任务中处理流式响应
    tokio::spawn(async move {
        // 发送开始事件
        let message_id = uuid::Uuid::new_v4().to_string();
        let _ = window_clone.emit(
            "message_stream_start",
            json!({
                "streamId": stream_id_clone,
                "messageId": message_id
            }),
        );

        // 使用真实的消息服务调用流式API
        let stream_callback = {
            let window = window_clone.clone();
            let stream_id = stream_id_clone.clone();

            move |chunk: crate::services::message::StreamChunk| {
                let _ = window.emit(
                    "message_stream_chunk",
                    json!({
                        "streamId": stream_id,
                        "content": chunk.content,
                        "reasoning": chunk.reasoning,
                        "chunk": "",  // 这里可以改为增量内容
                        "index": 0
                    }),
                );
            }
        };

        // 调用真实的流式API
        match service_clone
            .call_llm_api_stream(&request_clone, stream_callback)
            .await
        {
            Ok(response) => {
                // 发送完成事件
                let _ = window_clone.emit(
                    "message_stream_end",
                    json!({
                        "streamId": stream_id_clone,
                        "messageId": response.message_id,
                        "finalContent": response.content,
                        "finalReasoning": response.reasoning,
                        "chatId": response.chat_id,
                        "modelId": response.model_id,
                        "providerId": response.provider_id,
                        "pendingMcpCall": response.pending_mcp_call
                    }),
                );
            }
            Err(error) => {
                tracing::error!("[message_send_stream] Stream API error: {:?}", error);
                let _ = window_clone.emit(
                    "message_stream_error",
                    json!({
                        "streamId": stream_id_clone,
                        "error": error.message,
                        "code": error.code
                    }),
                );
            }
        }
    });

    Ok(stream_id)
}

/// 执行待确认的 MCP 工具调用
#[tauri::command]
pub async fn message_execute_mcp_call(
    pending_id: String,
    message_service: State<'_, MessageService>,
) -> Result<MessageResponse, AppError> {
    tracing::info!(
        "[message_execute_mcp_call] IPC command called for pending_id: {}",
        pending_id
    );
    message_service.execute_pending_mcp_call(pending_id).await
}

/// 直接执行工具调用（从 toolCallDeltas 创建并执行）
#[tauri::command]
pub async fn message_execute_tool_calls(
    message_id: UUID,
    tool_call_deltas: Vec<crate::llm_client::types::ChatToolCallDelta>,
    message_service: State<'_, MessageService>,
) -> Result<MessageResponse, AppError> {
    tracing::info!(
        "[message_execute_tool_calls] IPC command called for message_id: {}, {} tool calls",
        message_id,
        tool_call_deltas.len()
    );
    message_service.execute_tool_calls_directly(message_id, tool_call_deltas).await
}
