use crate::models::error::AppError;

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_get_last_payload() -> Result<Option<serde_json::Value>, AppError> {
    Ok(crate::services::selection::get_last_payload_json())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_get_last_payload() -> Result<Option<serde_json::Value>, AppError> {
    Ok(None)
}
