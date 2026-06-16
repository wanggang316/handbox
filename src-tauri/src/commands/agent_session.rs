// Agent Session 相关 IPC 命令
//
// Agent 模式会话的 CRUD 命令层，委托给 `AgentSessionService`。仅会话 CRUD 与
// transcript 读取；runtime / run / streaming / tools 属于后续 feature。
//
// M3 数据来源（双源并存，前端透明）：
//  - 会话**配置**（model/provider/tools/project 挂靠 等）权威源仍是 SQLite
//    （`agent_sessions` 行）—— grouping 依赖的 projectId 等只在此。
//  - 会话**活动**（messageCount / lastMessageAt / 标题）与 **transcript** 的
//    权威源是 JSONL（coding-agent SessionManager 落盘）。`agent_session_list`
//    用 JSONL 活动元数据覆盖 SQLite 行对应字段；`agent_session_messages` 直接
//    读 JSONL，无 JSONL 文件（pre-M3 老会话）时回退 SQLite transcript。

use crate::models::AppError;
use crate::services::{
    agent_jsonl_store, AgentRuntime, AgentSessionParameter, AgentSessionService,
};
use crate::storage::types::{AgentSession, AgentSessionMessage, CreateAgentSessionRequest, UUID};
use tauri::{AppHandle, Manager, State};

/// 创建新的 Agent Session
#[tauri::command]
pub async fn agent_session_create(
    request: CreateAgentSessionRequest,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    agent_session_service.create_session(request).await
}

/// 获取 Agent Session 列表
///
/// SQLite 提供配置行（含顺序：updated_at DESC）；JSONL 提供活动元数据
/// （messageCount / lastMessageAt / 标题），逐行 overlay 到对应会话上，使侧栏的
/// 计数与最近活动时间反映真实 transcript（JSONL 为权威）。无 JSONL 文件的老会话
/// 保留其 SQLite 字段不变。
#[tauri::command]
pub async fn agent_session_list(
    limit: Option<i32>,
    offset: Option<i32>,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<Vec<AgentSession>, AppError> {
    let mut sessions = agent_session_service.list_sessions(limit, offset).await?;
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    for session in sessions.iter_mut() {
        overlay_jsonl_activity(session, &app_data_dir);
    }
    Ok(sessions)
}

/// 获取 Agent Session 详情
///
/// 同 `agent_session_list`：在 SQLite 行上 overlay JSONL 活动元数据，使
/// `refreshAfterRun`（前端 `getAgentSession`）在 run 结束后拿到真实的 messageCount
/// / lastMessageAt（不再依赖已不更新这两列的 SQLite append 路径）。
#[tauri::command]
pub async fn agent_session_get(
    session_id: UUID,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let mut session = agent_session_service.get_session(session_id).await?;
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    overlay_jsonl_activity(&mut session, &app_data_dir);
    Ok(session)
}

/// 解析 Tauri 应用数据目录（JSONL 持久化根 `base_dir`）。
fn resolve_app_data_dir(app_handle: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal_error(&format!("failed to resolve app data dir: {e}")))
}

/// 用 JSONL 活动元数据 overlay 一个 SQLite 会话行（就地）。
///
/// JSONL 有该会话文件时：messageCount / lastMessageAt 取 JSONL（权威活动源），
/// 若 JSONL 携带 session 标签（agent 重命名）则覆盖 name。无 JSONL 文件（pre-M3
/// 老会话）或读取失败时：保持 SQLite 字段不变（优雅回退，绝不让一个坏文件
/// 拖垮整个列表）。
fn overlay_jsonl_activity(session: &mut AgentSession, app_data_dir: &std::path::Path) {
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), app_data_dir);
    match agent_jsonl_store::session_activity(app_data_dir, &cwd, &session.id) {
        Ok(Some(activity)) => {
            session.message_count = activity.message_count;
            session.last_message_at = activity.last_message_at;
            if let Some(name) = activity.name {
                session.name = name;
            }
        }
        // 无 JSONL（老会话）或读取错误：保留 SQLite 值，不阻断列表。
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(
                session_id = %session.id,
                "failed to read JSONL activity, keeping SQLite values: {e}"
            );
        }
    }
}

/// 重命名 Agent Session（M3：SQLite name + JSONL label 双写）。
///
/// SQLite `name` 是 fallback 名字源；但 `agent_session_list` / `agent_session_get`
/// 的 overlay 在 JSONL 携带 session 标签时用它覆盖 `name`（JSONL 为活动权威源）。
/// 因此仅写 SQLite 的旧 rename 会被该会话已有的 JSONL 旧标签（agent 自动取的标题）
/// 盖掉，重命名视觉上不生效。这里在 SQLite 写成功之后，对该会话的 JSONL **追加**
/// 一条 label（最新 label 胜），使 overlay 反映用户输入的新名。
///
/// side-effect 诚实：JSONL 写入失败降级为 warn 日志、不让整个 rename 失败——
/// SQLite 仍是 fallback 名字源，与 overlay「JSONL 读失败保留 SQLite 值」的容错
/// 姿态一致。返回**经 overlay** 的 session（与 list/get 一致），使前端直接拿到新名。
///
/// `app_handle` 是 Tauri 注入参数（前端透明），用于解析 app_data_dir 与该会话的
/// JSONL 路径。
#[tauri::command]
pub async fn agent_session_rename(
    session_id: UUID,
    name: String,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    // SQLite 权威写入（fallback 名字源）先行。
    let mut session = agent_session_service
        .rename_session(session_id, name.clone())
        .await?;

    // 把新名同时写进 JSONL label，使 overlay 反映新名而非旧 label。best-effort：
    // 失败不阻断 rename（SQLite 已写成功），只记 warn。
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);
    if let Err(e) = agent_jsonl_store::append_label(&app_data_dir, &cwd, &session.id, &name) {
        tracing::warn!(
            session_id = %session.id,
            "failed to write JSONL label on rename, keeping SQLite name: {e}"
        );
    }

    // 返回经 overlay 的 session（与 list/get 一致）：刚写的 label 让 name 即新名。
    overlay_jsonl_activity(&mut session, &app_data_dir);
    Ok(session)
}

/// 更新 Agent Session 单个字段（镜像 `agent_update_field`）
#[tauri::command]
pub async fn agent_session_update_field(
    session_id: UUID,
    field_name: String,
    value: serde_json::Value,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<AgentSession, AppError> {
    let parameter = parse_session_parameter(&field_name, value)?;

    agent_session_service
        .update_session_field(session_id, parameter)
        .await
}

/// 将 IPC 字段名 + JSON 值解析为 `AgentSessionParameter`。
///
/// 未知字段（包括已废弃的 `"enabledSkills"`）一律返回 VALIDATION_ERROR，
/// 此时参数从未构造、service 从未被调用，因此不会写入任何行。
fn parse_session_parameter(
    field_name: &str,
    value: serde_json::Value,
) -> Result<AgentSessionParameter, AppError> {
    let parameter = match field_name {
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
    Ok(parameter)
}

/// 删除 Agent Session（M3：同时清理其 JSONL transcript 文件）。
///
/// 顺序：先中止该会话可能存在的活跃 run（`runtime.abort` 对无活跃 run 是 no-op），
/// 这样删除后不会再有 `agent_stream_event { sessionId: <deleted> }` 抵达前端；
/// 再 best-effort 删除其 JSONL 文件（M3 后 transcript 落在 JSONL，仅删 SQLite 行
/// 会在磁盘留下孤儿 `<id>.jsonl`）；最后删 SQLite 行（**权威**，决定列表是否还
/// 显示该行）。即便 JSONL 删除失败（warn），SQLite 删除成功即保证「行消失」。
///
/// `app_handle` 是 Tauri 注入参数（前端透明），用于解析 app_data_dir 与该会话的
/// JSONL 路径；为此需先取一次 session 拿 `working_dir` → cwd。会话不存在时
/// `get_session` 返回 NOT_FOUND（与旧行为一致：删一个不存在的会话报错）。
#[tauri::command]
pub async fn agent_session_delete(
    session_id: UUID,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
    runtime: State<'_, AgentRuntime>,
) -> Result<(), AppError> {
    // 先取 session 拿 working_dir 以解析 JSONL cwd（也借此对不存在的会话报 NOT_FOUND）。
    let session = agent_session_service
        .get_session(session_id.clone())
        .await?;

    runtime.abort(&session_id).await;

    // best-effort 清理 JSONL 文件，不阻断权威的 SQLite 删除。
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);
    if let Err(e) = agent_jsonl_store::delete_session_file(&app_data_dir, &cwd, &session_id) {
        tracing::warn!(
            session_id = %session_id,
            "failed to delete JSONL transcript file on delete, removing SQLite row anyway: {e}"
        );
    }

    agent_session_service.delete_session(session_id).await
}

/// 获取 Agent Session 的 transcript（M3: JSONL 为权威源）。
///
/// 优先读该会话的 JSONL transcript（`<app_data_dir>/sessions/<flattened-cwd>/
/// <id>.jsonl`，经 SessionManager `build_context` 还原，含工具调用与思考块——它们
/// 内嵌在 assistant 消息的 content blocks 内）。该会话尚无 JSONL 文件（pre-M3 老
/// 会话，只有 SQLite transcript）时，回退到 SQLite。JSONL 读取硬失败（罕见，如文件
/// 损坏到 open 报错）时同样回退 SQLite，避免单个坏文件白屏整条 timeline。
#[tauri::command]
pub async fn agent_session_messages(
    session_id: UUID,
    app_handle: AppHandle,
    agent_session_service: State<'_, AgentSessionService>,
) -> Result<Vec<AgentSessionMessage>, AppError> {
    let session = agent_session_service
        .get_session(session_id.clone())
        .await?;
    let app_data_dir = resolve_app_data_dir(&app_handle)?;
    let cwd = agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &app_data_dir);

    match agent_jsonl_store::load_transcript(&app_data_dir, &cwd, &session_id) {
        Ok(Some(rows)) => Ok(rows),
        // 无 JSONL 文件：pre-M3 老会话 → 回退 SQLite transcript。
        Ok(None) => agent_session_service.list_messages(session_id).await,
        // JSONL 存在但读取硬失败：记录并回退 SQLite，避免白屏。
        Err(e) => {
            tracing::warn!(
                session_id = %session_id,
                "failed to read JSONL transcript, falling back to SQLite: {e}"
            );
            agent_session_service.list_messages(session_id).await
        }
    }
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

    /// VAL-DEPRECATE-006 (inverted from the old presence test): `"enabledSkills"`
    /// is no longer a known field — every value shape falls into the
    /// Unknown-field VALIDATION_ERROR. The parameter is never constructed, the
    /// service is never invoked, so no row can be written.
    #[test]
    fn update_field_enabled_skills_is_unknown_field() {
        for value in [
            serde_json::json!(["a", "b"]),
            serde_json::json!([]),
            serde_json::json!(null),
            serde_json::json!("not-an-array"),
        ] {
            // AgentSessionParameter is not Debug, so match instead of expect_err.
            match parse_session_parameter("enabledSkills", value.clone()) {
                Ok(_) => panic!("enabledSkills must be rejected as unknown: {}", value),
                Err(err) => {
                    assert_eq!(
                        err.code, "VALIDATION_ERROR",
                        "rejection must be a VALIDATION_ERROR for {}",
                        value
                    );
                    assert!(
                        err.message.contains("Unknown field: enabledSkills"),
                        "must fall into the Unknown-field branch, got: {}",
                        err.message
                    );
                }
            }
        }
    }

    /// VAL-DEPRECATE-003: removing the enabledSkills branch leaves every other
    /// field mapping intact — thinkingLevel / enabledTools / workingDir /
    /// modelId still parse into their parameter variants.
    #[test]
    fn other_field_mappings_survive_enabled_skills_removal() {
        match parse_session_parameter("thinkingLevel", serde_json::json!("high")) {
            Ok(AgentSessionParameter::ThinkingLevel(Some(level))) => assert_eq!(level, "high"),
            _ => panic!("thinkingLevel must map to ThinkingLevel(Some)"),
        }

        match parse_session_parameter("enabledTools", serde_json::json!(["read", "write"])) {
            Ok(AgentSessionParameter::EnabledTools(tools)) => {
                assert_eq!(tools, vec!["read".to_string(), "write".to_string()]);
            }
            _ => panic!("enabledTools must map to EnabledTools"),
        }

        match parse_session_parameter("workingDir", serde_json::json!("/tmp")) {
            Ok(AgentSessionParameter::WorkingDir(Some(dir))) => assert_eq!(dir, "/tmp"),
            _ => panic!("workingDir must map to WorkingDir(Some)"),
        }

        match parse_session_parameter("modelId", serde_json::json!(null)) {
            Ok(AgentSessionParameter::ModelId(None)) => {}
            _ => panic!("modelId null must map to ModelId(None)"),
        }
    }
}
