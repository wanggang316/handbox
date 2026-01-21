// Selection panel types and management using tauri-nspanel

#[cfg(target_os = "macos")]
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, LogicalSize, Manager, Size, WebviewUrl};
#[cfg(target_os = "macos")]
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, ManagerExt, Panel, PanelBuilder, PanelLevel, StyleMask,
    TrackingAreaOptions,
};

// 定义两个面板类型
// MenuPanel: 菜单面板 - 小型横向按钮条 (显示、复制、翻译、收藏、设置)
// ActionPanel: 功能面板 - 大型交互面板 (翻译、问AI、选区、查询结果)
#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(MenuPanel {
        config: {
            can_become_key_window: true,       // 允许成为 key window
            can_become_main_window: false,     // 不需要成为主窗口
            becomes_key_only_if_needed: true,  // 只在需要时成为 key window
            is_floating_panel: true,          // 浮动在其他窗口之上（重要！）
        }
        with: {
            // 启用鼠标追踪以支持按钮点击和自动隐藏
            tracking_area: {
                options: TrackingAreaOptions::new()
                    .active_always()            // 始终追踪，即使应用未激活
                    .mouse_entered_and_exited() // 获取鼠标进入/离开通知
                    .mouse_moved()              // 追踪鼠标移动
                    .cursor_update(),           // 追踪光标更新
                auto_resize: true                // 自动调整追踪区域大小
            }
        }
    })

    panel!(ActionPanel {
        config: {
            can_become_key_window: true,       // 需要接收键盘事件(Escape关闭)
            can_become_main_window: false,     // 不需要成为主窗口
            becomes_key_only_if_needed: true,  // 只在需要时成为 key window
            is_floating_panel: true,          // 浮动在其他窗口之上（重要！）
        }
    })

    // 面板事件处理器
    panel_event!(MenuPanelEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
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
    tracing::info!("Creating menu panel with label: {}", MENU_PANEL_LABEL);
    // let menu_panel =
    PanelBuilder::<tauri::Wry, MenuPanel>::new(app, MENU_PANEL_LABEL)
        .url(WebviewUrl::App("/selection/menu".into()))
        .title("Selection Menu")
        .size(Size::Logical(LogicalSize::new(MENU_WIDTH, MENU_HEIGHT)))
        .level(PanelLevel::Floating) // PopUpMenu 级别适合菜单
        // .hides_on_deactivate(false) // 不要在失去焦点时隐藏
        .with_window(|window| {
            window
                .resizable(false)
                .decorations(false)
                .transparent(true)
                .visible(false)
                .skip_taskbar(true)
        })
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build menu panel: {}", e);
            e
        })?;

    // 配置菜单面板的行为
    // menu_panel.set_level(PanelLevel::PopUpMenu.value());

    // 确保面板不会激活应用
    // menu_panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());

    // 允许面板在全屏窗口同一空间显示，并加入所有空间
    // menu_panel.set_collection_behavior(
    //     CollectionBehavior::new()
    //         .full_screen_auxiliary()
    //         .can_join_all_spaces()
    //         .into(),
    // );

    // 不在失去焦点时隐藏
    // menu_panel.set_hides_on_deactivate(false);

    // 允许在模态对话框运行时接收事件
    // menu_panel.set_works_when_modal(true);

    // 设置事件处理器
    // let handler = MenuPanelEventHandler::new();

    // // 监听鼠标进入事件 - 让面板成为 key window 以接收点击
    // handler.window_did_become_key(move |_notification| {
    //     tracing::debug!("Menu panel became key window");
    // });

    // // 监听鼠标离开事件 - 使用独立的 app_handle
    // let app_for_resign = app.clone();
    // handler.window_did_resign_key(move |_notification| {
    //     tracing::debug!("Menu panel resigned key window");
    //     // 当失去焦点时隐藏面板
    //     if let Some(panel) = get_menu_panel(&app_for_resign) {
    //         panel.hide();
    //     }
    // });

    // menu_panel.set_event_handler(Some(handler.as_ref()));

    tracing::info!("Menu panel created successfully: {:?}", MENU_PANEL_LABEL);

    // 创建功能面板
    // tracing::info!("Creating action panel with label: {}", ACTION_PANEL_LABEL);
    // let _action_panel = PanelBuilder::<tauri::Wry, ActionPanel>::new(app, ACTION_PANEL_LABEL)
    //     .url(WebviewUrl::App("/selection/action".into()))
    //     .title("Selection Action")
    //     .size(Size::Logical(LogicalSize::new(
    //         ACTION_WIDTH,
    //         ACTION_MIN_HEIGHT,
    //     )))
    //     .level(PanelLevel::PopUpMenu) // PopUpMenu 级别
    //     .hides_on_deactivate(false) // 不要在失去焦点时隐藏
    //     .with_window(|window| {
    //         window
    //             .resizable(true)
    //             .decorations(false)
    //             .transparent(true)
    //             .visible(false)
    //             .skip_taskbar(true)
    //     })
    //     .build()
    //     .map_err(|e| {
    //         tracing::error!("Failed to build action panel: {}", e);
    //         e
    //     })?;

    // tracing::info!(
    //     "Action panel created successfully: {:?}",
    //     ACTION_PANEL_LABEL
    // );

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
        panel.hide();
    }
    if let Some(panel) = get_action_panel(app) {
        panel.hide();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_all_panels(_app: &AppHandle) {}
