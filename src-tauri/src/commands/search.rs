// 搜索相关 IPC 命令

use crate::models::{ApiResponse, AppError};
use crate::services::SearchService;
use tauri::State;

/// 搜索消息和会话
#[tauri::command]
pub async fn search_query(
    query: String,
    search_service: State<'_, SearchService>,
) -> Result<ApiResponse<Vec<String>>, String> {
    // TODO: 实现搜索功能
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}
