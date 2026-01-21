// Selection panel types and management using tauri-nspanel

#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager, WebviewUrl};
#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, ManagerExt, Panel, PanelBuilder, PanelLevel};

// 定义两个面板类型
// MenuPanel: 菜单面板 - 小型横向按钮条 (显示、复制、翻译、收藏、设置)
// ActionPanel: 功能面板 - 大型交互面板 (翻译、问AI、选区、查询结果)
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(MenuPanel {
        config: {
            can_become_key_window: true,       // 允许成为 key window
            can_become_main_window: false,     // 不需要成为主窗口
            is_floating_panel: false,          // 不是浮动面板(浮动面板会拦截事件)
        }
    })

    panel!(ActionPanel {
        config: {
            can_become_key_window: true,       // 需要接收键盘事件(Escape关闭)
            can_become_main_window: false,     // 不需要成为主窗口
            is_floating_panel: false,          // 不是浮动面板
        }
    })
}

const MENU_PANEL_LABEL: &str = "selection_menu";
const ACTION_PANEL_LABEL: &str = "selection_action";
const MENU_WIDTH: f64 = 360.0;
const MENU_HEIGHT: f64 = 44.0;
const ACTION_WIDTH: f64 = 520.0;
const ACTION_MIN_HEIGHT: f64 = 220.0;

/// 初始化选择面板
#[cfg(target_os = "macos")]
pub fn setup_selection_panels(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Setting up selection panels");

    // 创建菜单面板
    let menu_panel = PanelBuilder::<tauri::Wry, MenuPanel>::new(
        app,
        MENU_PANEL_LABEL,
    )
    .url(WebviewUrl::App("/selection/menu".into()))
    .title("Selection Menu")
    .with_window(|window| {
        window
            .inner_size(MENU_WIDTH, MENU_HEIGHT)
            .position(0.0, 0.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .visible(false)
            .content_protected(true)
    })
    .level(PanelLevel::Normal)  // 使用 Normal 而不是 Floating
    .transparent(true)
    .hides_on_deactivate(false)
    .build()?;

    // 配置面板交互属性
    menu_panel.set_ignores_mouse_events(false); // 确保可以接收鼠标事件
    menu_panel.set_accepts_mouse_moved_events(true);
    menu_panel.set_floating_panel(false); // 确保不是浮动面板

    tracing::info!("Menu panel created: {:?}", MENU_PANEL_LABEL);

    // 创建功能面板
    let action_panel = PanelBuilder::<tauri::Wry, ActionPanel>::new(
        app,
        ACTION_PANEL_LABEL,
    )
    .url(WebviewUrl::App("/selection/action".into()))
    .title("Selection Action")
    .with_window(|window| {
        window
            .inner_size(ACTION_WIDTH, ACTION_MIN_HEIGHT)
            .position(0.0, 0.0)
            .resizable(true)
            .decorations(false)
            .transparent(true)
            .visible(false)
            .content_protected(true)
    })
    .level(PanelLevel::Normal)  // 使用 Normal 而不是 Floating
    .transparent(true)
    .hides_on_deactivate(false)
    .build()?;

    // 配置面板交互属性
    action_panel.set_ignores_mouse_events(false); // 确保可以接收鼠标事件
    action_panel.set_accepts_mouse_moved_events(true);
    action_panel.set_floating_panel(false); // 确保不是浮动面板

    tracing::info!("Action panel created: {:?}", ACTION_PANEL_LABEL);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn setup_selection_panels(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

/// 获取菜单面板
#[cfg(target_os = "macos")]
pub fn get_menu_panel(app: &AppHandle) -> Option<Arc<dyn Panel<tauri::Wry>>> {
    app.get_webview_panel(MENU_PANEL_LABEL).ok()
}

/// 获取功能面板
#[cfg(target_os = "macos")]
pub fn get_action_panel(app: &AppHandle) -> Option<Arc<dyn Panel<tauri::Wry>>> {
    app.get_webview_panel(ACTION_PANEL_LABEL).ok()
}

/// 隐藏所有选择面板
#[cfg(target_os = "macos")]
pub fn hide_all_panels(app: &AppHandle) {
    if let Some(panel) = get_menu_panel(app) {
        let _ = panel.hide();
    }
    if let Some(panel) = get_action_panel(app) {
        let _ = panel.hide();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_all_panels(_app: &AppHandle) {}
