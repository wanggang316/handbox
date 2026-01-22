// Selection panel types and management using tauri-nspanel

#[cfg(target_os = "macos")]
use std::sync::Arc;
use tauri::Runtime;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
// use tauri_nspanel::WebviewWindowExt;
// use tauri::{AppHandle, Manager, Runtime, WebviewWindow};
use tauri_nspanel::{ManagerExt, WebviewWindowExt as _};
// use tauri_nspanel::{tauri_panel, Panel, PanelBuilder, PanelLevel, TrackingAreaOptions};

const MENU_PANEL_LABEL: &str = "selection_menu";
const ACTION_PANEL_LABEL: &str = "selection_action";
const MENU_WIDTH: f64 = 360.0;
const MENU_HEIGHT: f64 = 44.0;
const ACTION_WIDTH: f64 = 520.0;
const ACTION_MIN_HEIGHT: f64 = 220.0;

/// 核心函数：将普通的 WebviewWindow 转换为专业的 NSPanel
pub fn init_floating_panel<R: Runtime>(app: &AppHandle<R>) {
    // 1. 获取在 tauri.conf.json 中定义的窗口
    let window = app
        .get_webview_window("floating")
        .expect("找不到 label 为 floating 的窗口");

    // 2. 将 WebviewWindow 转换为 NSPanel
    // 注意：转换后，该窗口将具备 Panel 的特性（如不占 Dock 图标、可浮动在全屏应用上等）
    let panel = window.to_panel().expect("无法将窗口转换为 NSPanel");

    // 3. 配置 Panel 的样式掩码 (Style Mask)
    // NonactivatingPanel: 关键！点击时不激活 App，且是实现“点击别处消失”的前提
    // Titled: 必须包含，否则某些系统属性无法设置
    // FullSizeContentView: 允许内容填充整个面板，适合自定义 UI
    // let mut mask = PanelStyleMask::empty();
    // mask.insert(PanelStyleMask::NonactivatingPanel);
    // mask.insert(PanelStyleMask::Titled);
    // mask.insert(PanelStyleMask::FullSizeContentView);
    // panel.set_style_mask(mask);

    // 4. 配置 Panel 行为
    // 设置为浮动层级，确保它在所有普通窗口之上
    // panel.set_level(NSPanelLevel::Floating);

    // 关键：点击非面板区域时自动隐藏
    panel.set_hides_on_deactivate(true);

    // 允许面板在全屏应用上方显示
    // panel.set_collection_behaviour(
    //     tauri_nspanel::NSWindowCollectionBehavior::FullScreenAuxiliary
    //         | tauri_nspanel::NSWindowCollectionBehavior::CanJoinAllSpaces,
    // );

    // // 设置面板标题（虽然隐藏了，但系统 API 需要）
    // panel.set_title("Handbox Floating Menu");
}

// pub fn init_menu_panel1<R: Runtime>(app: &AppHandle<R>) {
//     tracing::info!("Setting up selection panels");

//     // 创建菜单面板
//     tracing::info!("Creating menu panel with label: {}", MENU_PANEL_LABEL);
//     let menu_panel = PanelBuilder::<tauri::Wry, MenuPanel>::new(app, MENU_PANEL_LABEL)
//         .url(WebviewUrl::App("/floating".into()))
//         .title("Selection Menu")
//         .size(Size::Logical(LogicalSize::new(MENU_WIDTH, MENU_HEIGHT)))
//         .level(PanelLevel::PopUpMenu) // PopUpMenu 级别适合菜单
//         .hides_on_deactivate(true) // 不要在失去焦点时隐藏
//         .released_when_closed(true)
//         .with_window(|window| {
//             window
//                 .resizable(false)
//                 .decorations(false)
//                 .transparent(false)
//                 .visible(false)
//                 .skip_taskbar(true)
//         })
//         .build()
//         .map_err(|e| {
//             tracing::error!("Failed to build menu panel: {}", e);
//             e
//         })?;

//     // 配置菜单面板的行为
//     // menu_panel.set_level(PanelLevel::PopUpMenu.value());

//     // 确保面板不会激活应用
//     // menu_panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());

//     // 允许面板在全屏窗口同一空间显示，并加入所有空间
//     // menu_panel.set_collection_behavior(
//     //     CollectionBehavior::new()
//     //         .full_screen_auxiliary()
//     //         .can_join_all_spaces()
//     //         .into(),
//     // );

//     // 不在失去焦点时隐藏
//     menu_panel.set_hides_on_deactivate(true);
//     menu_panel.set_released_when_closed(false);

//     // 允许在模态对话框运行时接收事件
//     // menu_panel.set_works_when_modal(true);

//     // 设置事件处理器
//     // let handler = MenuPanelEventHandler::new();

//     // // 监听鼠标进入事件 - 让面板成为 key window 以接收点击
//     // handler.window_did_become_key(move |_notification| {
//     //     tracing::debug!("Menu panel became key window");
//     // });

//     // // 监听鼠标离开事件 - 使用独立的 app_handle
//     // let app_for_resign = app.clone();
//     // handler.window_did_resign_key(move |_notification| {
//     //     tracing::debug!("Menu panel resigned key window");
//     //     // 当失去焦点时隐藏面板
//     //     if let Some(panel) = get_menu_panel(&app_for_resign) {
//     //         panel.hide();
//     //     }
//     // });

//     // menu_panel.set_event_handler(Some(handler.as_ref()));

//     tracing::info!("Menu panel created successfully: {:?}", MENU_PANEL_LABEL);
// }

/// 初始化选择面板
#[cfg(target_os = "macos")]
pub fn setup_selection_panels(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    init_floating_panel(app);

    Ok(())
}
