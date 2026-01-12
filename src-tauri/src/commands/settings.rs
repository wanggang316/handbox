// 设置相关 IPC 命令

use crate::models::{AppError, AppSettings, ExportSettingsOptions, ImportSettingsRequest, UpdateSettingsRequest};
use crate::services::SettingsService;
use tauri::State;

#[tauri::command]
pub async fn settings_get(settings_service: State<'_, SettingsService>) -> Result<AppSettings, AppError> {
    settings_service.get_settings()
}

#[tauri::command]
pub async fn settings_update(
    request: UpdateSettingsRequest,
    settings_service: State<'_, SettingsService>,
) -> Result<AppSettings, AppError> {
    settings_service.update_settings(request)
}

#[tauri::command]
pub async fn settings_reset(
    sections: Option<Vec<String>>,
    settings_service: State<'_, SettingsService>,
) -> Result<AppSettings, AppError> {
    settings_service.reset_settings(sections)
}

#[tauri::command]
pub async fn settings_export(
    _options: Option<ExportSettingsOptions>,
    settings_service: State<'_, SettingsService>,
) -> Result<String, AppError> {
    let settings = settings_service.get_settings()?;
    serde_json::to_string_pretty(&settings)
        .map_err(|e| AppError::internal_error(&format!("导出设置失败: {e}")))
}

#[tauri::command]
pub async fn settings_import(
    request: ImportSettingsRequest,
    settings_service: State<'_, SettingsService>,
) -> Result<AppSettings, AppError> {
    let settings: AppSettings = serde_json::from_str(&request.data)
        .map_err(|e| AppError::validation_error(&format!("导入设置失败: {e}")))?;

    settings_service
        .update_settings(UpdateSettingsRequest {
            section: "general".to_string(),
            data: serde_json::to_value(settings.general.clone())
                .map_err(|e| AppError::internal_error(&format!("导入设置失败: {e}")))?,
        })?;
    settings_service
        .update_settings(UpdateSettingsRequest {
            section: "mcp".to_string(),
            data: serde_json::to_value(settings.mcp.clone())
                .map_err(|e| AppError::internal_error(&format!("导入设置失败: {e}")))?,
        })?;
    settings_service
        .update_settings(UpdateSettingsRequest {
            section: "account".to_string(),
            data: serde_json::to_value(settings.account.clone())
                .map_err(|e| AppError::internal_error(&format!("导入设置失败: {e}")))?,
        })?;
    settings_service
        .update_settings(UpdateSettingsRequest {
            section: "translation".to_string(),
            data: serde_json::to_value(settings.translation.clone())
                .map_err(|e| AppError::internal_error(&format!("导入设置失败: {e}")))?,
        })
}

#[tauri::command]
pub async fn settings_validate_mcp(
    _config: String,
) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({ "valid": true }))
}

#[tauri::command]
pub async fn settings_test_mcp_server(
    _server: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({ "success": true }))
}

#[tauri::command]
pub async fn settings_system_info() -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "tauri_version": tauri::VERSION,
    }))
}

#[tauri::command]
pub async fn settings_check_updates() -> Result<serde_json::Value, AppError> {
    Ok(serde_json::json!({ "hasUpdate": false }))
}
