// Agent Session 相关 IPC 命令
//
// Agent 模式会话的 CRUD 命令层，委托给 `AgentSessionService`。仅会话 CRUD 与
// transcript 读取；runtime / run / streaming / tools 属于后续 feature。

use crate::models::AppError;
use crate::services::{AgentSessionParameter, AgentSessionService};
use crate::storage::types::{AgentSession, AgentSessionMessage, CreateAgentSessionRequest, UUID};
use tauri::State;

/// 创建新的 Agent Session
#[tauri::command]
pub async fn agent_session_create(
    request: CreateAgentSessionRequest,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service.create_session(request).await
}

/// 获取 Agent Session 列表
#[tauri::command]
pub async fn agent_session_list(
    limit: Option<i32>,
    offset: Option<i32>,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<Vec<AgentSession>, AppError> {
    agent_session_service.list_sessions(limit, offset).await
}

/// 获取 Agent Session 详情
#[tauri::command]
pub async fn agent_session_get(
    session_id: UUID,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service.get_session(session_id).await
}

/// 重命名 Agent Session
#[tauri::command]
pub async fn agent_session_rename(
    session_id: UUID,
    name: String,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service.rename_session(session_id, name).await
}

/// 更新 Agent Session 单个字段（镜像 `agent_update_field`）
#[tauri::command]
pub async fn agent_session_update_field(
    session_id: UUID,
    field_name: String,
    value: serde_json::Value,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let parameter = match field_name.as_str() {
        "name" => {
            let name = value
                .as_str()
                .ok_or_else(|| AppError::validation_error("Invalid name value"))?
                .to_string();
            AgentSessionParameter::Name(name)
        }
        "modelId" => AgentSessionParameter::ModelId(parse_optional_string(&value, "model_id")?),
        "providerId" => {
            AgentSessionParameter::ProviderId(parse_optional_string(&value, "provider_id")?)
        }
        "systemPrompt" => {
            AgentSessionParameter::SystemPrompt(parse_optional_string(&value, "system_prompt")?)
        }
        "thinkingLevel" => {
            AgentSessionParameter::ThinkingLevel(parse_optional_string(&value, "thinking_level")?)
        }
        "temperature" => {
            let temp_value = if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_f64()
                        .ok_or_else(|| AppError::validation_error("Invalid temperature value"))?
                        as f32,
                )
            };
            AgentSessionParameter::Temperature(temp_value)
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
            AgentSessionParameter::MaxTokens(max_tokens_value)
        }
        "workingDir" => {
            AgentSessionParameter::WorkingDir(parse_optional_string(&value, "working_dir")?)
        }
        "enabledTools" => {
            let tools = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid enabled_tools value: {}", e))
            })?;
            AgentSessionParameter::EnabledTools(tools)
        }
        "toolExecutionMode" => AgentSessionParameter::ToolExecutionMode(parse_optional_string(
            &value,
            "tool_execution_mode",
        )?),
        _ => {
            return Err(AppError::validation_error(&format!(
                "Unknown field: {}",
                field_name
            )))
        }
    };

    agent_session_service
        .update_session_field(session_id, parameter)
        .await
}

/// 删除 Agent Session
#[tauri::command]
pub async fn agent_session_delete(
    session_id: UUID,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<(), AppError> {
    agent_session_service.delete_session(session_id).await
}

/// 获取 Agent Session 的 transcript
#[tauri::command]
pub async fn agent_session_messages(
    session_id: UUID,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<Vec<AgentSessionMessage>, AppError> {
    agent_session_service.list_messages(session_id).await
}

/// 将 JSON 值解析为 `Option<String>`：null -> None，字符串 -> Some，其它 -> 校验错误。
fn parse_optional_string(
    value: &serde_json::Value,
    field: &str,
) -> Result<Option<String>, AppError> {
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(
            value
                .as_str()
                .ok_or_else(|| AppError::validation_error(&format!("Invalid {} value", field)))?
                .to_string(),
        ))
    }
}
