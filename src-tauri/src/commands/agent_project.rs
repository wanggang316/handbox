// Agent Project 相关 IPC 命令
//
// Agent 模式项目（按工作目录分组会话）的命令层，委托给 `AgentProjectService`。
// 注意命名区分：`/agents` 预设的命令是 `agent_*`，本模块的命令是
// `agent_project_*`，两者完全独立。

use crate::models::AppError;
use crate::services::AgentProjectService;
use crate::storage::types::{AgentProject, UUID};
use tauri::{AppHandle, Manager, State};

/// 创建 Agent Project（get-or-create by canonical path）
#[tauri::command]
pub async fn agent_project_create(
    path: String,
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<AgentProject, AppError> {
    agent_project_service.create_project(path).await
}

/// 获取 Agent Project 列表
#[tauri::command]
pub async fn agent_project_list(
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<Vec<AgentProject>, AppError> {
    agent_project_service.list_projects().await
}

/// 重命名 Agent Project
#[tauri::command]
pub async fn agent_project_rename(
    project_id: UUID,
    name: String,
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<AgentProject, AppError> {
    agent_project_service.rename_project(project_id, name).await
}

/// 删除 Agent Project
///
/// 删除前先中止该项目全部会话可能存在的活跃 run（`coding_agent_runtime::abort_run`
/// 对无活跃 run 是 no-op，对齐 `agent_session_delete` 的写法），再 best-effort 删除
/// 每个会话的 JSONL transcript 文件，最后级联删除项目、其会话及 SQLite transcript
/// （VAL-CASESS-021）。注入 `app_handle` 以解析 JSONL base（app data dir）。
#[tauri::command]
pub async fn agent_project_delete(
    project_id: UUID,
    app_handle: AppHandle,
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<(), AppError> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal_error(&format!("failed to resolve app data dir: {e}")))?;
    agent_project_service
        .delete_project(project_id, &app_data_dir)
        .await
}
