// MCP management IPC commands

use crate::models::{
    AppError, CreateMcpServerRequest, RefreshMcpServerRequest, ToggleMcpServerRequest,
    UpdateMcpServerRequest, UpdateToolEnabledRequest,
};
use crate::services::McpService;
use crate::storage::types::McpServer;
use tauri::State;

/// 获取 MCP 服务器列表
#[tauri::command]
pub async fn mcp_list_servers(
    mcp_service: State<'_, McpService>,
) -> Result<Vec<McpServer>, AppError> {
    mcp_service.list_servers().await
}

/// 创建新的 MCP 服务器
#[tauri::command]
pub async fn mcp_create_server(
    request: CreateMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.create_server(request).await
}

/// 更新 MCP 服务器
#[tauri::command]
pub async fn mcp_update_server(
    server_id: String,
    request: UpdateMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.update_server(server_id, request).await
}

/// 删除 MCP 服务器
#[tauri::command]
pub async fn mcp_delete_server(
    server_id: String,
    mcp_service: State<'_, McpService>,
) -> Result<(), AppError> {
    mcp_service.delete_server(server_id).await
}

/// 切换 MCP 服务器启用状态
#[tauri::command]
pub async fn mcp_toggle_server(
    request: ToggleMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.toggle_server(request).await
}

/// 刷新 MCP 服务器元数据
#[tauri::command]
pub async fn mcp_refresh_server(
    request: RefreshMcpServerRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.refresh_server(request).await
}

/// 更新工具启用状态
#[tauri::command]
pub async fn mcp_update_tool_enabled(
    request: UpdateToolEnabledRequest,
    mcp_service: State<'_, McpService>,
) -> Result<McpServer, AppError> {
    mcp_service.update_tool_enabled(request).await
}

/// 统计使用特定 MCP 服务器的聊天数量
#[tauri::command]
pub async fn mcp_count_chats_using_server(
    server_id: String,
    mcp_service: State<'_, McpService>,
) -> Result<i32, AppError> {
    mcp_service.count_chats_using_server(&server_id).await
}

/// 从所有聊天中移除 MCP 服务器引用
#[tauri::command]
pub async fn mcp_remove_server_from_chats(
    server_id: String,
    mcp_service: State<'_, McpService>,
) -> Result<i32, AppError> {
    mcp_service.remove_mcp_server_from_chats(&server_id).await
}
