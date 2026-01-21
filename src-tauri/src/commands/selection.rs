use crate::models::error::AppError;

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_get_last_payload() -> Result<Option<serde_json::Value>, AppError> {
    Ok(crate::services::selection::get_last_payload_json())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_hide_menu_panel(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection_panel::get_menu_panel;

    if let Some(panel) = get_menu_panel(&app) {
        app.run_on_main_thread(move || {
            panel.hide();
        })
        .map_err(|e| AppError::internal_error(&e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_hide_action_panel(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection_panel::get_action_panel;

    if let Some(panel) = get_action_panel(&app) {
        app.run_on_main_thread(move || {
            panel.hide();
        })
        .map_err(|e| AppError::internal_error(&e.to_string()))?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_show_action_panel(
    app: tauri::AppHandle,
    mode: String,
    text: String,
) -> Result<(), AppError> {
    use crate::services::selection_panel::{get_action_panel, get_menu_panel};
    use tauri::Emitter;

    // 隐藏菜单面板
    if let Some(menu) = get_menu_panel(&app) {
        let app_clone = app.clone();
        app_clone
            .run_on_main_thread(move || {
                menu.hide();
            })
            .map_err(|e| AppError::internal_error(&e.to_string()))?;
    }

    // 显示功能面板
    let Some(panel) = get_action_panel(&app) else {
        return Err(AppError::internal_error("Action panel not found"));
    };

    // 发送模式和文本数据
    if let Some(window) = panel.to_window() {
        window
            .emit("mode_change", &serde_json::json!({ "mode": mode, "text": text }))
            .map_err(|e| AppError::internal_error(&e.to_string()))?;
    }

    // 显示面板并聚焦（必须在主线程执行）
    app.run_on_main_thread(move || {
        panel.show();
        let ns_panel = panel.as_panel();
        unsafe {
            ns_panel.orderFront(None);
        }
    })
    .map_err(|e| AppError::internal_error(&e.to_string()))?;

    Ok(())
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
    anchor: Option<String>,
) -> Result<(), AppError> {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::{NSPoint, NSSize};
    use tauri::Manager;

    let Some(window) = app.get_webview_window("selection_overlay") else {
        return Ok(());
    };

    let window_for_thread = window.clone();
    let keep_bottom = matches!(anchor.as_deref(), Some("bottom"));
    let _ = window.run_on_main_thread(move || {
        let Ok(ns_window_ptr) = window_for_thread.ns_window() else {
            return;
        };
        let ns_window: &NSWindow = unsafe { &*(ns_window_ptr as *mut NSWindow) };
        let frame = ns_window.frame();
        let origin_x = frame.origin.x + (frame.size.width - width) / 2.0;
        let top_left = if keep_bottom {
            NSPoint::new(origin_x, frame.origin.y + height)
        } else {
            NSPoint::new(origin_x, frame.origin.y + frame.size.height)
        };
        ns_window.setContentSize(NSSize::new(width, height));
        ns_window.setFrameTopLeftPoint(top_left);
    });

    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_overlay_lock(locked: bool) -> Result<(), AppError> {
    crate::services::selection::set_overlay_locked(locked);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_overlay_dismiss() -> Result<(), AppError> {
    crate::services::selection::dismiss_current_selection_signature();
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_overlay_set_interactive(
    app: tauri::AppHandle,
    interactive: bool,
) -> Result<(), AppError> {
    use objc2_app_kit::NSWindow;
    use std::sync::mpsc;
    use std::time::Duration;
    use tauri::Manager;

    let Some(window) = app.get_webview_window("selection_overlay") else {
        return Ok(());
    };

    let window_for_thread = window.clone();
    let (tx, rx) = mpsc::channel();
    let _ = window.run_on_main_thread(move || {
        if let Ok(ns_window_ptr) = window_for_thread.ns_window() {
            let ns_window: &NSWindow = unsafe { &*(ns_window_ptr as *mut NSWindow) };
            ns_window.setHidesOnDeactivate(interactive);
        }
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_millis(120));

    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_get_last_payload() -> Result<Option<serde_json::Value>, AppError> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_hide_menu_panel(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_hide_action_panel(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_show_action_panel(
    _app: tauri::AppHandle,
    _mode: String,
    _text: String,
) -> Result<(), AppError> {
    Ok(())
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
    _anchor: Option<String>,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_overlay_lock(_locked: bool) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_overlay_dismiss() -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_overlay_set_interactive(
    _app: tauri::AppHandle,
    _interactive: bool,
) -> Result<(), AppError> {
    Ok(())
}
