// MCP management IPC commands

use crate::models::{
    AppError, CreateMcpServerRequest, RefreshMcpServerRequest, ToggleMcpServerRequest,
    UpdateMcpServerRequest, UpdateToolEnabledRequest,
};
use crate::services::McpService;
use crate::storage::types::McpServer;
use tauri::State;

#[tauri::command]
pub async fn mcp_list_servers(
    mcp_service: State<'_, McpService>,
) -> Result<Vec<McpServer>, AppError> {
    mcp_service.list_servers().await
}

#[tauri::command]
pub async fn mcp_create_server(
    request: CreateMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.create_server(request).await
}

#[tauri::command]
pub async fn mcp_update_server(
    server_id: String,
    request: UpdateMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.update_server(server_id, request).await
}

#[tauri::command]
pub async fn mcp_delete_server(
    server_id: String,
    mcp_service: State<'_, McpService>,
) -> Result<(), AppError> {
    mcp_service.delete_server(server_id).await
}

#[tauri::command]
pub async fn mcp_toggle_server(
    request: ToggleMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.toggle_server(request).await
}

#[tauri::command]
pub async fn mcp_refresh_server(
    request: RefreshMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.refresh_server(request).await
}

#[tauri::command]
pub async fn mcp_update_tool_enabled(
    request: UpdateToolEnabledRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.update_tool_enabled(request).await
}
