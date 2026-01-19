use crate::models::error::AppError;

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_get_last_payload() -> Result<Option<serde_json::Value>, AppError> {
    Ok(crate::services::selection::get_last_payload_json())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_overlay_hide(app: tauri::AppHandle) -> Result<(), AppError> {
    crate::services::selection::hide_overlay_window_and_restore(&app);
    Ok(())
}


#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_get_last_payload() -> Result<Option<serde_json::Value>, AppError> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_overlay_hide(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}
