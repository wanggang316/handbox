// 设置相关 IPC 命令

use crate::models::{ApiResponse, AppError, AppSettings};
use crate::services::SettingsService;
use tauri::State;

/// 获取应用设置
#[tauri::command]
pub async fn settings_get(
    settings_service: State<'_, SettingsService>,
) -> Result<ApiResponse<AppSettings>, String> {
    // TODO: 实现设置获取
    Ok(ApiResponse::error(AppError::internal_error(
        "Not implemented yet",
    )))
}
