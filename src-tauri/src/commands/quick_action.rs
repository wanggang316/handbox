//! Show / hide / toggle commands for the Quick Action overlay window.
//!
//! Entry points for the global hotkey (and devtools manual testing). show /
//! toggle read the global mouse position in **physical pixels** from
//! `AppHandle::cursor_position()`; the panel module converts it and positions
//! the panel on the display under the cursor. Non-macOS builds get no-op stubs.

use crate::models::error::AppError;

/// Global mouse position in physical pixels; falls back to (0, 0) on failure.
#[cfg(target_os = "macos")]
fn cursor_phys_position(app: &tauri::AppHandle) -> (f64, f64) {
    match app.cursor_position() {
        Ok(pos) => (pos.x, pos.y),
        Err(e) => {
            tracing::warn!("failed to read cursor position, defaulting to (0,0): {e}");
            (0.0, 0.0)
        }
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn quick_action_show(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::show_quick_action_panel;
    let (x, y) = cursor_phys_position(&app);
    tracing::info!("quick_action_show at cursor ({x}, {y})");
    show_quick_action_panel(&app, x, y);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn quick_action_hide(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::hide_quick_action_panel;
    tracing::info!("quick_action_hide");
    hide_quick_action_panel(&app);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn quick_action_toggle(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::toggle_quick_action_panel;
    let (x, y) = cursor_phys_position(&app);
    tracing::info!("quick_action_toggle at cursor ({x}, {y})");
    toggle_quick_action_panel(&app, x, y);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn quick_action_show(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn quick_action_hide(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn quick_action_toggle(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

/// Re-registers the global shortcut that summons the Quick Action overlay.
///
/// Unregisters the previously recorded accelerator before registering the new
/// one, so a replaced combination is fully deactivated (the settings page's
/// live rebind calls this). A registration failure returns a structured
/// [`AppError`] so the frontend can prompt for a different combination.
#[tauri::command]
pub async fn quick_action_register_shortcut(
    app: tauri::AppHandle,
    accelerator: String,
) -> Result<(), AppError> {
    crate::services::quick_action::register_shortcut(&app, &accelerator)
}

/// Unregisters the Quick Action global shortcut (called when the settings page
/// disables Quick Action). Idempotent: a no-op when not registered.
#[tauri::command]
pub async fn quick_action_unregister_shortcut(app: tauri::AppHandle) -> Result<(), AppError> {
    crate::services::quick_action::unregister_shortcut(&app)
}

/// Backend for "continue in chat": brings the main window to the front and
/// tells the frontend to navigate to the given chat session.
///
/// Unminimizes, shows, and focuses the `main` window, then emits
/// `quick-action-open-chat` with `chat_id`; the frontend listener routes to
/// `/chat?id=<chatId>` (the overlay creates a real persisted chat session).
/// The `main` window always exists (close only hides it); if it is unexpectedly
/// missing, a structured [`AppError`] is returned.
#[tauri::command]
pub async fn quick_action_continue_in_chat(
    app: tauri::AppHandle,
    chat_id: String,
) -> Result<(), AppError> {
    use tauri::{Emitter, Manager};

    tracing::info!("quick_action_continue_in_chat chat_id={chat_id}");

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::internal_error("主窗口不存在，无法在对话中继续"))?;

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();

    window
        .emit("quick-action-open-chat", chat_id)
        .map_err(|e| AppError::internal_error(&format!("发送会话导航事件失败: {e}")))?;

    Ok(())
}
