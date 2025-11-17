// 窗口管理相关 IPC 命令

use crate::models::AppError;
use tauri::{AppHandle, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

fn map_window_error(action: &'static str) -> impl FnOnce(tauri::Error) -> AppError {
    move |error| AppError {
        code: "WINDOW_ERROR".to_string(),
        message: format!("{}: {}", action, error),
        hint: Some("请重启应用或重新打开窗口".to_string()),
    }
}

/// 打开设置窗口
#[tauri::command]
pub async fn open_settings_window(app: AppHandle, path: Option<String>) -> Result<(), AppError> {
    // 构建 URL 路径
    let url_path = if let Some(p) = path {
        format!(
            "/settings{}",
            if p.starts_with('/') {
                p
            } else {
                format!("/{}", p)
            }
        )
    } else {
        "/settings".to_string()
    };

    // 检查设置窗口是否已经存在
    if let Some(window) = app.get_webview_window("settings") {
        // 如果窗口存在，显示并聚焦
        window
            .show()
            .map_err(map_window_error("显示设置窗口失败"))?;
        window
            .set_focus()
            .map_err(map_window_error("聚焦设置窗口失败"))?;

        // 导航到指定路径
        window
            .eval(&format!("window.location.href = '{}'", url_path))
            .map_err(map_window_error("跳转到设置页面失败"))?;
    } else {
        // 如果窗口不存在，创建新窗口
        let _window = WebviewWindowBuilder::new(&app, "settings", WebviewUrl::App(url_path.into()))
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
            .map_err(map_window_error("创建设置窗口失败"))?;
    }
    Ok(())
}

/// 关闭设置窗口
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("settings") {
        window
            .close()
            .map_err(map_window_error("关闭设置窗口失败"))?;
    }
    Ok(())
}

/// 切换设置窗口显示状态
#[tauri::command]
pub async fn toggle_settings_window(app: AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window("settings") {
        if window
            .is_visible()
            .map_err(map_window_error("检测设置窗口状态失败"))?
        {
            window
                .hide()
                .map_err(map_window_error("隐藏设置窗口失败"))?;
        } else {
            window
                .show()
                .map_err(map_window_error("显示设置窗口失败"))?;
            window
                .set_focus()
                .map_err(map_window_error("聚焦设置窗口失败"))?;
        }
    } else {
        open_settings_window(app, None).await?;
    }
    Ok(())
}
