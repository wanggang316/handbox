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

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_overlay_resize(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
) -> Result<(), AppError> {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::{NSPoint, NSSize};
    use tauri::{LogicalSize, Manager};

    let Some(window) = app.get_webview_window("selection_overlay") else {
        return Ok(());
    };

    let _ = window.set_size(LogicalSize::new(width, height));

    let window_for_thread = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Ok(ns_window_ptr) = window_for_thread.ns_window() else {
            return;
        };
        let ns_window: &NSWindow = unsafe { &*(ns_window_ptr as *mut NSWindow) };
        let frame = ns_window.frame();
        let top_left = NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height);
        ns_window.setContentSize(NSSize::new(width, height));
        ns_window.setFrameTopLeftPoint(top_left);
    });

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

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_overlay_resize(
    _app: tauri::AppHandle,
    _width: f64,
    _height: f64,
) -> Result<(), AppError> {
    Ok(())
}
