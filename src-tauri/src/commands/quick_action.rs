//! Quick Action 浮层窗口的 show / hide / toggle 命令。
//!
//! 这些命令是后续 global hotkey（以及 devtools 手测）的调用入口。show / toggle
//! 在显示时从 `AppHandle::cursor_position()` 取得**物理像素**的全局鼠标位置，交由
//! 面板模块换算并定位到鼠标所在显示器。非 macOS 下提供 no-op stub。

use crate::models::error::AppError;

/// 取得全局鼠标的物理像素位置；失败时回退到 (0, 0)。
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

/// 显示 Quick Action 浮层。
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn quick_action_show(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::show_quick_action_panel;
    let (x, y) = cursor_phys_position(&app);
    tracing::info!("quick_action_show at cursor ({x}, {y})");
    show_quick_action_panel(&app, x, y);
    Ok(())
}

/// 隐藏 Quick Action 浮层。
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn quick_action_hide(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::hide_quick_action_panel;
    tracing::info!("quick_action_hide");
    hide_quick_action_panel(&app);
    Ok(())
}

/// 切换 Quick Action 浮层可见性。
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
