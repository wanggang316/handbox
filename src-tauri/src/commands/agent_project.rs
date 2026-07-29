// Agent-mode project commands (sessions grouped by working directory), delegating to
// `AgentProjectService`. Naming: `agent_*` commands belong to `/agents` presets;
// this module's `agent_project_*` commands are entirely independent.

use crate::models::AppError;
use crate::services::AgentProjectService;
use crate::storage::types::{AgentProject, UUID};
use tauri::{AppHandle, Manager, State};

/// Get-or-create by canonical path.
#[tauri::command]
pub async fn agent_project_create(
    path: String,
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<AgentProject, AppError> {
    agent_project_service.create_project(path).await
}

#[tauri::command]
pub async fn agent_project_list(
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<Vec<AgentProject>, AppError> {
    agent_project_service.list_projects().await
}

#[tauri::command]
pub async fn agent_project_rename(
    project_id: UUID,
    name: String,
    agent_project_service: State<'_, AgentProjectService>,
) -> Result<AgentProject, AppError> {
    agent_project_service.rename_project(project_id, name).await
}

/// Aborts any active run of the project's sessions first (abort is a no-op when
/// idle), best-effort deletes each session's JSONL transcript, then cascade-deletes
/// the project, its sessions, and SQLite transcripts. `app_handle` resolves the
/// JSONL base dir (app data dir).
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
