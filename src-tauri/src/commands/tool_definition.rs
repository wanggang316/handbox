// User-defined tool ("dynamic tool") management IPC commands.

use crate::models::AppError;
use crate::services::ToolDefinitionService;
use crate::storage::types::{
    CreateToolDefinitionRequest, ToolDefinition, UpdateToolDefinitionRequest,
};
use tauri::State;

#[tauri::command]
pub async fn tool_definition_list(
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<Vec<ToolDefinition>, AppError> {
    tool_definition_service.list().await
}

#[tauri::command]
pub async fn tool_definition_get(
    tool_id: String,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<ToolDefinition, AppError> {
    tool_definition_service.get(&tool_id).await
}

#[tauri::command]
pub async fn tool_definition_create(
    request: CreateToolDefinitionRequest,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<ToolDefinition, AppError> {
    tool_definition_service.create(request).await
}

#[tauri::command]
pub async fn tool_definition_update(
    tool_id: String,
    request: UpdateToolDefinitionRequest,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<ToolDefinition, AppError> {
    tool_definition_service.update(&tool_id, request).await
}

#[tauri::command]
pub async fn tool_definition_delete(
    tool_id: String,
    tool_definition_service: State<'_, ToolDefinitionService>,
) -> Result<(), AppError> {
    tool_definition_service.delete(&tool_id).await
}
