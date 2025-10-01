// 消息相关 IPC 命令

use crate::models::{AppError, Message, MessageRequest, MessageResponse, UUID};
use crate::services::{message::StreamChunk, MessageService};
use serde_json::json;
use tauri::{Emitter, State, Window};

/// 创建流式开始回调
fn create_stream_start_callback(
    window: Window,
    event_name: &'static str,
    extra_data: Option<serde_json::Value>,
) -> impl FnMut(String, String) {
    move |returned_stream_id: String, message_id: String| {
        let mut payload = json!({
            "streamId": returned_stream_id.clone(),
            "messageId": message_id
        });

        // 合并额外的数据
        if let Some(extra) = &extra_data {
            if let Some(obj) = payload.as_object_mut() {
                if let Some(extra_obj) = extra.as_object() {
                    for (key, value) in extra_obj {
                        obj.insert(key.clone(), value.clone());
                    }
                }
            }
        }

        let _ = window.emit(event_name, payload);
        tracing::info!(
            "[{}] Stream started for stream {}",
            event_name,
            returned_stream_id
        );
    }
}

/// 创建流式数据回调
fn create_streaming_callback(window: Window, event_name: &'static str) -> impl FnMut(StreamChunk) {
    move |chunk: StreamChunk| {
        let _ = window.emit(
            event_name,
            json!({
                "streamId": chunk.stream_id,
                "content": chunk.content,
                "reasoning": chunk.reasoning,
                "toolCalls": chunk.tool_calls,
                "chunk": "",
                "index": 0
            }),
        );
    }
}

/// 创建流式结束回调
fn create_stream_end_callback(
    window: Window,
    event_name: &'static str,
) -> impl FnMut(String, MessageResponse) {
    move |stream_id: String, response: MessageResponse| {
        let _ = window.emit(
            event_name,
            json!({
                "streamId": stream_id,
                "messageId": response.message_id,
                "finalContent": response.content,
                "finalReasoning": response.reasoning,
                "chatId": response.chat_id,
                "modelId": response.model_id,
                "providerId": response.provider_id,
                "toolCalls": response.tool_calls
            }),
        );
        tracing::info!("[{}] Stream ended for stream {}", event_name, stream_id);
    }
}

/// 创建流式错误回调
fn create_stream_error_callback(
    window: Window,
    event_name: &'static str,
) -> impl FnMut(String, AppError) {
    move |stream_id: String, error: AppError| {
        let _ = window.emit(
            event_name,
            json!({
                "streamId": stream_id,
                "error": error.message,
                "code": error.code
            }),
        );
        tracing::error!(
            "[{}] Stream error for stream {}: {:?}",
            event_name,
            stream_id,
            error
        );
    }
}

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
) -> Result<(), AppError> {
    tracing::info!("[message_send_stream] IPC command called");

    // 克隆必要的数据和窗口引用
    let window_clone = window.clone();
    let request_clone = request.clone();
    let service_clone = message_service.inner().clone();

    // 在后台任务中处理流式响应
    tokio::spawn(async move {
        let start_callback =
            create_stream_start_callback(window_clone.clone(), "message_stream_start", None);

        let streaming_callback =
            create_streaming_callback(window_clone.clone(), "message_stream_chunk");

        let end_callback = create_stream_end_callback(window_clone.clone(), "message_stream_end");

        let error_callback =
            create_stream_error_callback(window_clone.clone(), "message_stream_error");

        // 调用真实的流式API
        service_clone
            .send_message_stream(
                request_clone,
                start_callback,
                streaming_callback,
                end_callback,
                error_callback,
            )
            .await;
    });

    // 立即返回，真实的 stream_id 通过 start 回调事件传递给前端
    Ok(())
}

/// 执行工具调用
#[tauri::command]
pub async fn message_execute_tool_calls(
    message_id: String,
    tool_call_ids: Vec<String>,
    message_service: State<'_, MessageService>,
) -> Result<(), AppError> {
    tracing::info!(
        "[message_execute_tool_calls] IPC command called for message_id: {} with tool call IDs: {:?}",
        message_id,
        tool_call_ids
    );

    message_service
        .execute_tool_calls(
            message_id,
            tool_call_ids,
            |_stream_id, _message_id| {
                // 开始回调
                tracing::info!("[message_execute_tool_calls] Execution started");
            },
            |_chunk| {
                // 工具调用执行的流式输出，暂时不需要特别处理
                // 可以在这里添加日志或进度追踪
            },
            |_stream_id, _response| {
                // 结束回调
                tracing::info!("[message_execute_tool_calls] Execution completed");
            },
            |_stream_id, error| {
                // 错误回调
                tracing::error!("[message_execute_tool_calls] Execution failed: {:?}", error);
            },
        )
        .await;

    tracing::info!("[message_execute_tool_calls] Command completed successfully");
    Ok(())
}

/// 流式执行工具调用
#[tauri::command]
pub async fn message_execute_tool_calls_stream(
    message_id: String,
    tool_call_ids: Vec<String>,
    window: Window,
    message_service: State<'_, MessageService>,
) -> Result<(), AppError> {
    tracing::info!(
        "[message_execute_tool_calls_stream] IPC command called for message_id: {} with tool call IDs: {:?}",
        message_id,
        tool_call_ids
    );

    // 克隆必要的数据和窗口引用
    let window_clone = window.clone();
    let service_clone = message_service.inner().clone();
    let message_id_clone = message_id.clone();
    let tool_call_ids_clone = tool_call_ids.clone();

    // 在后台任务中处理流式响应
    tokio::spawn(async move {
        let start_callback =
            create_stream_start_callback(window_clone.clone(), "tool_execute_stream_start", None);

        let streaming_callback =
            create_streaming_callback(window_clone.clone(), "tool_execute_stream_chunk");

        let end_callback =
            create_stream_end_callback(window_clone.clone(), "tool_execute_stream_end");

        let error_callback =
            create_stream_error_callback(window_clone.clone(), "tool_execute_stream_error");

        // 调用真实的工具执行流式API
        service_clone
            .execute_tool_calls(
                message_id_clone,
                tool_call_ids_clone,
                start_callback,
                streaming_callback,
                end_callback,
                error_callback,
            )
            .await;
    });

    // 立即返回，真实的 stream_id 通过 start 回调事件传递给前端
    Ok(())
}
