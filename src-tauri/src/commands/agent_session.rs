// Agent Session 相关 IPC 命令
//
// Agent 模式会话的 CRUD 命令层，委托给 `AgentSessionService`。仅会话 CRUD 与
// transcript 读取；runtime / run / streaming / tools 属于后续 feature。

use crate::models::AppError;
use crate::services::{AgentRuntime, AgentSessionParameter, AgentSessionService};
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
        "enabledSkills" => {
            let skills: Vec<String> = serde_json::from_value(value).map_err(|e| {
                AppError::validation_error(&format!("Invalid enabled_skills value: {}", e))
            })?;
            AgentSessionParameter::EnabledSkills(skills)
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
///
/// 删除前先中止该会话可能存在的活跃 run（`runtime.abort` 对无活跃 run 是 no-op），
/// 这样删除后不会再有 `agent_stream_event { sessionId: <deleted> }` 抵达前端。
#[tauri::command]
pub async fn agent_session_delete(
    session_id: UUID,
    agent_session_service: State<'_, AgentSessionService>,
    runtime: State<'_, AgentRuntime>,
) -> Result<(), AppError> {
    runtime.abort(&session_id).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors the exact expression the `"enabledSkills"` branch of
    /// `agent_session_update_field` runs to coerce the IPC value into a
    /// `Vec<String>`. Centralizing the contract here keeps VAL-PERSIST-008
    /// (reject non-array / non-string element / null) and VAL-PERSIST-012
    /// (a well-formed array maps to `EnabledSkills`) directly testable without a
    /// Tauri runtime.
    fn parse_enabled_skills(value: serde_json::Value) -> Result<AgentSessionParameter, AppError> {
        let skills: Vec<String> = serde_json::from_value(value).map_err(|e| {
            AppError::validation_error(&format!("Invalid enabled_skills value: {}", e))
        })?;
        Ok(AgentSessionParameter::EnabledSkills(skills))
    }

    /// VAL-PERSIST-012: a well-formed string array is accepted and produces the
    /// EnabledSkills parameter verbatim (no dedup at the wire layer).
    #[test]
    fn enabled_skills_accepts_string_array_verbatim() {
        let param =
            parse_enabled_skills(serde_json::json!(["a", "a", ""])).expect("array must parse");
        match param {
            AgentSessionParameter::EnabledSkills(skills) => {
                assert_eq!(skills, vec!["a".to_string(), "a".to_string(), "".to_string()]);
            }
            _ => panic!("expected EnabledSkills variant"),
        }

        // Empty array is valid and yields an empty Vec.
        let empty = parse_enabled_skills(serde_json::json!([])).expect("empty array must parse");
        match empty {
            AgentSessionParameter::EnabledSkills(skills) => assert!(skills.is_empty()),
            _ => panic!("expected EnabledSkills variant"),
        }
    }

    /// VAL-PERSIST-008: non-array, null, and arrays containing non-string
    /// elements are all rejected with a VALIDATION_ERROR — the bad value never
    /// becomes an EnabledSkills parameter, so no row is ever written.
    #[test]
    fn enabled_skills_rejects_non_array_null_and_non_string_elements() {
        for bad in [
            serde_json::json!(null),
            serde_json::json!("not-an-array"),
            serde_json::json!(42),
            serde_json::json!({ "k": "v" }),
            serde_json::json!(["ok", 1]),
            serde_json::json!([true]),
            serde_json::json!([null]),
        ] {
            // AgentSessionParameter is not Debug, so match instead of expect_err.
            match parse_enabled_skills(bad.clone()) {
                Ok(_) => panic!("value must be rejected: {}", bad),
                Err(err) => assert_eq!(
                    err.code, "VALIDATION_ERROR",
                    "rejection must be a VALIDATION_ERROR for {}",
                    bad
                ),
            }
        }
    }
}
