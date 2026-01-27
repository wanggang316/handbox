use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::models::error::AppError;
use crate::models::UpdateSettingsRequest;
use crate::SettingsService;

/// 内容面板模式
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentPanelMode {
    Show,
    Translate,
    Ai,
}

/// 选中文本的应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionAppInfo {
    pub name: String,
    pub bundle_id: String,
    pub pid: i32,
}

/// 显示内容面板的 payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionPayload {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub app_info: SelectionAppInfo,
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

/// 显示 content panel
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_show_content_panel(
    app: tauri::AppHandle,
    mode: ContentPanelMode,
    payload: SelectionPayload,
) -> Result<(), AppError> {
    use crate::services::selection::show_content_panel;
    tracing::info!(">>>>>>>>>>>> selection_show_content_panel start, mode: {:?}", mode);
    let _ = app.emit(
        "init-content",
        serde_json::json!({
            "mode": mode,
            "text": payload.text,
            "x": payload.x,
            "y": payload.y,
            "app_info": payload.app_info
        }),
    );
    show_content_panel(&app, payload.x, payload.y);
    Ok(())
}

/// 隐藏 content panel
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_hide_content_panel(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::hide_content_panel;

    tracing::info!(">>>>>>>>>>>> selection_hide_content_panel start");
    hide_content_panel(&app);
    Ok(())
}

/// 隐藏 menu panel
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_hide_menu_panel(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::hide_menu_panel;

    tracing::info!(">>>>>>>>>>>> selection_hide_menu_panel start");
    hide_menu_panel(&app);
    Ok(())
}

/// 设置 content panel 置顶状态
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_set_content_pinned(pinned: bool) -> Result<(), AppError> {
    use crate::services::selection::set_content_panel_pinned;

    tracing::info!(">>>>>>>>>>>> selection_set_content_pinned: {}", pinned);
    set_content_panel_pinned(pinned);
    Ok(())
}

/// 获取 content panel 置顶状态
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_get_content_pinned() -> Result<bool, AppError> {
    use crate::services::selection::is_content_panel_pinned;
    Ok(is_content_panel_pinned())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_show_settings_panel(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<(), AppError> {
    use crate::services::selection::show_settings_panel;
    show_settings_panel(&app, x, y);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_hide_settings_panel(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::hide_settings_panel;
    hide_settings_panel(&app);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_show_settings_panel(
    _app: tauri::AppHandle,
    _x: f64,
    _y: f64,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_show_settings_disable_panel(
    app: tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<(), AppError> {
    use crate::services::selection::show_settings_disable_panel;
    tracing::info!(">>>>>>>>>>>> selection_show_settings_disable_panel start, x: {}, y: {}", x, y);
    show_settings_disable_panel(&app, x, y);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_disable_current_app_by_pid(
    settings_service: tauri::State<'_, SettingsService>,
) -> Result<(), AppError> {
    use crate::utils::get_frontmost_app_info;
    let Some(info) = get_frontmost_app_info() else {
        return Ok(());
    };
    let mut settings = settings_service.get_settings()?;
    let blacklist = &mut settings.quick_tools.selection_blacklist;
    if !blacklist.pids.contains(&info.pid) {
        blacklist.pids.push(info.pid);
    }
    settings_service.update_settings(UpdateSettingsRequest {
        section: "quickTools".to_string(),
        data: serde_json::to_value(settings.quick_tools)
            .map_err(|e| AppError::internal_error(&format!("序列化设置失败: {e}")))?,
    })?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_disable_current_app_by_bundle_id(
    settings_service: tauri::State<'_, SettingsService>,
) -> Result<(), AppError> {
    use crate::utils::get_frontmost_app_info;
    let Some(info) = get_frontmost_app_info() else {
        return Ok(());
    };
    let mut settings = settings_service.get_settings()?;
    let blacklist = &mut settings.quick_tools.selection_blacklist;
    if !blacklist.bundle_ids.contains(&info.bundle_id) {
        blacklist.bundle_ids.push(info.bundle_id);
    }
    settings_service.update_settings(UpdateSettingsRequest {
        section: "quickTools".to_string(),
        data: serde_json::to_value(settings.quick_tools)
            .map_err(|e| AppError::internal_error(&format!("序列化设置失败: {e}")))?,
    })?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_disable_global(
    settings_service: tauri::State<'_, SettingsService>,
) -> Result<(), AppError> {
    let mut settings = settings_service.get_settings()?;
    settings.quick_tools.show_toolbar_on_selection = false;
    settings_service.update_settings(UpdateSettingsRequest {
        section: "quickTools".to_string(),
        data: serde_json::to_value(settings.quick_tools)
            .map_err(|e| AppError::internal_error(&format!("序列化设置失败: {e}")))?,
    })?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_disable_current_app_by_pid(
    _settings_service: tauri::State<'_, SettingsService>,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_disable_current_app_by_bundle_id(
    _settings_service: tauri::State<'_, SettingsService>,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_disable_global(
    _settings_service: tauri::State<'_, SettingsService>,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn selection_hide_settings_disable_panel(app: tauri::AppHandle) -> Result<(), AppError> {
    use crate::services::selection::hide_settings_disable_panel;
    hide_settings_disable_panel(&app);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_show_settings_disable_panel(
    _app: tauri::AppHandle,
    _x: f64,
    _y: f64,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_hide_settings_disable_panel(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_hide_settings_panel(_app: tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_show_content_panel(
    _app: tauri::AppHandle,
    _mode: ContentPanelMode,
    _payload: SelectionPayload,
) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_hide_content_panel(_app: tauri::AppHandle) -> Result<(), AppError> {
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
pub async fn selection_set_content_pinned(_pinned: bool) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn selection_get_content_pinned() -> Result<bool, AppError> {
    Ok(false)
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
