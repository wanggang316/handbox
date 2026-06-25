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

/// 重新注册唤起 Quick Action 浮层的全局快捷键。
///
/// 反注册先前记录的加速键、注册新加速键，使被替换的旧组合彻底失活（设置页 live
/// rebind 调用此命令实现）。注册失败返回结构化 [`AppError`]，前端据此提示用户更
/// 换组合。
#[tauri::command]
pub async fn quick_action_register_shortcut(
    app: tauri::AppHandle,
    accelerator: String,
) -> Result<(), AppError> {
    crate::services::quick_action::register_shortcut(&app, &accelerator)
}

/// 「在聊天中继续」的后端：把主窗口带到前台并通知前端导航到指定会话。
///
/// 取消最小化、显示并聚焦 `main` 窗口，随后向该窗口发送
/// `quick-action-open-session` 事件，载荷为 `session_id`。前端监听器（下个 feature）
/// 据此路由到对应的 agent 会话。`main` 窗口始终存在（关闭仅隐藏），若意外缺失则返回
/// 结构化 [`AppError`]。
#[tauri::command]
pub async fn quick_action_continue_in_chat(
    app: tauri::AppHandle,
    session_id: String,
) -> Result<(), AppError> {
    use tauri::{Emitter, Manager};

    tracing::info!("quick_action_continue_in_chat session_id={session_id}");

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::internal_error("主窗口不存在，无法在聊天中继续"))?;

    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();

    window
        .emit("quick-action-open-session", session_id)
        .map_err(|e| AppError::internal_error(&format!("发送会话导航事件失败: {e}")))?;

    Ok(())
}
