// 聊天相关 IPC 命令

use crate::models::AppError;
use crate::services::{SessionParameter, SessionService};
use crate::storage::types::{Session, SessionReasoningConfig, UUID};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateTitleResponse {
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateRequest {
    pub name: String,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub max_tokens: Option<i32>,
    pub stream: Option<bool>,
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub system_prompt: Option<String>,
    pub mcp_servers: Option<Vec<crate::storage::types::McpServerConfig>>,
}

/// 创建新的聊天
#[tauri::command]
pub async fn session_create(
    request: SessionCreateRequest,
    chat_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    chat_service
        .create_chat(
            request.name,
            request.temperature,
            request.top_p,
            request.top_k,
            request.max_tokens,
            request.stream,
            request.model_id,
            request.provider_id,
            request.system_prompt,
            request.mcp_servers,
        )
        .await
}

/// 获取聊天列表
#[tauri::command]
pub async fn session_list(
    limit: Option<i32>,
    offset: Option<i32>,
    chat_service: State<'_, SessionService>,
) -> Result<Vec<Session>, AppError> {
    chat_service.list_chats(limit, offset).await
}

/// 获取聊天详情
#[tauri::command]
pub async fn session_get(
    chat_id: UUID,
    chat_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    chat_service.get_chat(chat_id).await
}

/// 更新聊天单个字段
#[tauri::command]
pub async fn session_update_field(
    chat_id: UUID,
    field_name: String,
    value: serde_json::Value,
    chat_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    let parameter = match field_name.as_str() {
        "temperature" => {
            let temp_value = if value.is_null() {
                None
            } else {
                let val = value
                    .as_f64()
                    .ok_or_else(|| AppError::validation_error("Invalid temperature value"))?
                    as f32;
                Some(val)
            };
            SessionParameter::Temperature(temp_value)
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
            SessionParameter::TopP(top_p_value)
        }
        "topK" => {
            let top_k_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_i64()
                        .ok_or_else(|| AppError::validation_error("Invalid top_k value"))?
                        as i32,
                )
            };
            SessionParameter::TopK(top_k_value)
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
            SessionParameter::MaxTokens(max_tokens_value)
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
            SessionParameter::Stream(stream_value)
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
            SessionParameter::SystemPrompt(prompt_value)
        }
        "mcpServers" => {
            let servers = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid mcp_servers value: {}", e))
            })?;
            SessionParameter::McpServers(servers)
        }
        "turnCount" => {
            let turn_count_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_i64()
                        .ok_or_else(|| AppError::validation_error("Invalid turn_count value"))?
                        as i32,
                )
            };
            SessionParameter::TurnCount(turn_count_value)
        }
        "reasoning" => {
            let reasoning_value = if value.is_null() {
                None
            } else {
                Some(
                    serde_json::from_value::<SessionReasoningConfig>(value).map_err(|e| {
                        AppError::validation_error(&format!(
                            "Invalid reasoning configuration: {}",
                            e
                        ))
                    })?,
                )
            };
            SessionParameter::Reasoning(reasoning_value)
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
pub async fn session_update_model(
    chat_id: UUID,
    model_id: String,
    provider_id: String,
    chat_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    chat_service
        .update_chat_parameter(
            chat_id,
            SessionParameter::Model {
                model_id,
                provider_id,
            },
        )
        .await
}

/// 清空模型参数
#[tauri::command]
pub async fn session_clear_model_parameters(
    chat_id: UUID,
    chat_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    chat_service.clear_model_parameters(chat_id).await
}

/// 更新聊天名称
#[tauri::command]
pub async fn session_update_name(
    chat_id: UUID,
    name: String,
    chat_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    chat_service
        .update_chat_parameter(chat_id, SessionParameter::Name(name))
        .await
}

/// 删除聊天
#[tauri::command]
pub async fn session_delete(
    chat_id: UUID,
    chat_service: State<'_, SessionService>,
) -> Result<(), AppError> {
    chat_service.delete_chat(chat_id).await
}

/// 生成聊天标题
#[tauri::command]
pub async fn session_generate_title(
    chat_id: UUID,
    chat_service: State<'_, SessionService>,
) -> Result<GenerateTitleResponse, AppError> {
    let title = chat_service.generate_title(chat_id).await?;
    Ok(GenerateTitleResponse { title })
}

/// 通过 Agent 创建 Session（复制 Agent 配置）
#[tauri::command]
pub async fn session_create_from_agent(
    agent_id: UUID,
    session_service: State<'_, SessionService>,
) -> Result<Session, AppError> {
    session_service.create_session_from_agent(agent_id).await
}
