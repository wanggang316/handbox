// 窗口管理相关 IPC 命令

use tauri::{AppHandle, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

/// 打开设置窗口
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // 检查设置窗口是否已经存在
    if let Some(window) = app.get_webview_window("settings") {
        // 如果窗口存在，显示并聚焦
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        // 如果窗口不存在，创建新窗口
        let _window =
            WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App("/settings".into()))
                .title("Settings - handbox")
                .inner_size(800.0, 600.0)
                .min_inner_size(600.0, 400.0)
                .resizable(true)
                .decorations(true)
                .title_bar_style(tauri::TitleBarStyle::Overlay)
                .hidden_title(true)
                .maximizable(false)
                .minimizable(false)
                .traffic_light_position(LogicalPosition::new(15.0, 27.0))
                .build()
                .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 关闭设置窗口
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 切换设置窗口显示状态
#[tauri::command]
pub async fn toggle_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        if window.is_visible().map_err(|e| e.to_string())? {
            window.hide().map_err(|e| e.to_string())?;
        } else {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
    } else {
        open_settings_window(app).await?;
    }
    Ok(())
}
