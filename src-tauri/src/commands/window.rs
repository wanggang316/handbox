// 窗口管理相关 IPC 命令

use crate::models::AppError;
use tauri::{AppHandle, Emitter, Manager};

fn map_window_error(action: &'static str) -> impl FnOnce(tauri::Error) -> AppError {
    move |error| AppError {
        code: "WINDOW_ERROR".to_string(),
        message: format!("{}: {}", action, error),
        hint: Some("请重启应用或重新打开窗口".to_string()),
    }
}

/// 打开设置：设置页在主窗口内渲染（不再是独立窗口）。
/// 聚焦主窗口并通知其导航到 /settings[/path]；供原生菜单（⌘,）与
/// 划词等其他 webview 窗口调用，主窗口内部直接 goto 即可、无需经此命令。
#[tauri::command]
pub async fn open_settings_window(app: AppHandle, path: Option<String>) -> Result<(), AppError> {
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

    let window = app.get_webview_window("main").ok_or_else(|| AppError {
        code: "WINDOW_ERROR".to_string(),
        message: "主窗口不存在".to_string(),
        hint: Some("请重启应用".to_string()),
    })?;
    window
        .show()
        .map_err(map_window_error("显示主窗口失败"))?;
    window
        .set_focus()
        .map_err(map_window_error("聚焦主窗口失败"))?;
    // 定向发给主窗口；根布局监听该事件并 goto。
    app.emit_to("main", "settings:navigate", url_path)
        .map_err(map_window_error("通知主窗口导航失败"))?;
    Ok(())
}
