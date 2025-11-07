// 聊天相关 IPC 命令

use crate::models::AppError;
use crate::services::{ChatParameter, ChatService};
use crate::storage::types::{Chat, UUID};
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
    mcp_servers: Option<Vec<crate::storage::types::McpServerConfig>>,
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

/// 更新聊天单个字段
#[tauri::command]
pub async fn chat_update_field(
    chat_id: UUID,
    field_name: String,
    value: serde_json::Value,
    chat_service: State<'_, ChatService>,
) -> Result<Chat, AppError> {
    println!(
        "[chat_update_field] chat_id: {}, field_name: {}, value: {:?}",
        chat_id, field_name, value
    );

    let parameter = match field_name.as_str() {
        "temperature" => {
            let temp_value = if value.is_null() {
                println!("[chat_update_field] temperature: value is null, setting to None");
                None
            } else {
                let val = value
                    .as_f64()
                    .ok_or_else(|| AppError::validation_error("Invalid temperature value"))?
                    as f32;
                println!("[chat_update_field] temperature: value is {}", val);
                Some(val)
            };
            ChatParameter::Temperature(temp_value)
        }
        "topP" => {
            let top_p_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_f64()
                        .ok_or_else(|| AppError::validation_error("Invalid top_p value"))?
                        as f32,
                )
            };
            ChatParameter::TopP(top_p_value)
        }
        "maxTokens" => {
            let max_tokens_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_i64()
                        .ok_or_else(|| AppError::validation_error("Invalid max_tokens value"))?
                        as i32,
                )
            };
            ChatParameter::MaxTokens(max_tokens_value)
        }
        "stream" => {
            let stream_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_bool()
                        .ok_or_else(|| AppError::validation_error("Invalid stream value"))?,
                )
            };
            ChatParameter::Stream(stream_value)
        }
        "systemPrompt" => {
            let prompt_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_str()
                        .ok_or_else(|| AppError::validation_error("Invalid system_prompt value"))?
                        .to_string(),
                )
            };
            ChatParameter::SystemPrompt(prompt_value)
        }
        "mcpServers" => {
            let servers = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid mcp_servers value: {}", e))
            })?;
            ChatParameter::McpServers(servers)
        }
        _ => {
            return Err(AppError::validation_error(&format!(
                "Unknown field: {}",
                field_name
            )))
        }
    };

    chat_service.update_chat_parameter(chat_id, parameter).await
}

/// 更新聊天模型
#[tauri::command]
pub async fn chat_update_model(
    chat_id: UUID,
    model_id: String,
    provider_id: String,
    chat_service: State<'_, ChatService>,
) -> Result<Chat, AppError> {
    chat_service
        .update_chat_parameter(
            chat_id,
            ChatParameter::Model {
                model_id,
                provider_id,
            },
        )
        .await
}

/// 更新聊天名称
#[tauri::command]
pub async fn chat_update_name(
    chat_id: UUID,
    name: String,
    chat_service: State<'_, ChatService>,
) -> Result<Chat, AppError> {
    chat_service
        .update_chat_parameter(chat_id, ChatParameter::Name(name))
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
