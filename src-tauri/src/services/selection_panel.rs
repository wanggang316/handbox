// Selection panel types and management using tauri-nspanel

use tauri::{LogicalSize, Size, WebviewUrl};
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask, WebviewWindowExt, tauri_panel};

tauri_panel! {
    // 定义面板类：允许成为 Key Window（这样按钮才能点击），但不允许成为 Main Window
    panel!(FloatingPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false
        }
    })

    panel_event!(FloatingEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

/// 核心函数：将普通的 WebviewWindow 转换为专业的 NSPanel
// pub fn init_floating_panel(app_handle: &AppHandle) {

//     let window: WebviewWindow = app_handle.get_webview_window("floating").unwrap();

//     let panel = window.to_panel::<FloatingPanel>().unwrap();
  
//     println!("panel class name: {:?}", panel.as_panel().class().name());
//     println!("panel can become key?: {}", panel.can_become_key_window());
//     println!("panel can become main?: {}", panel.can_become_main_window());
  
//     panel.set_level(PanelLevel::Floating.value());
//     // panel.set_hides_on_deactivate(true);
//     panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());

//     let handler: Retained<FloatingEventHandler> = FloatingEventHandler::new();
  
//     let handle: AppHandle = app_handle.to_owned();
  
//     handler.window_did_become_key(move |notification| {
//       let app_name = handle.package_info().name.to_owned();
  
//       unsafe { println!("[info]: Notification name: {:?}", notification.name()) };
//       println!("[info]: {:?} panel becomes key window!", app_name);
//     });
  
//     let handle_clone = app_handle.clone();
//     handler.window_did_resign_key(move |_| {
//       println!("[info]: panel resigned from key window!");
//       if let Ok(p) = handle_clone.get_webview_panel("floating") {
//         p.hide();
//       }
//     });
  
//     panel.set_event_handler(Some(handler.as_ref()));

// }

pub fn init_menu_panel1(app_handle: &AppHandle) {
    tracing::info!("Setting up selection panels");

    // 创建菜单面板
    let menu_panel = PanelBuilder::<tauri::Wry, FloatingPanel>::new(app_handle, "floating")
        .url(WebviewUrl::App("/floating".into()))
        .title("floating")
        .size(Size::Logical(LogicalSize::new(360.0, 48.0)))
        // .position(tauri::Position::Logical(tauri::LogicalPosition::new(
        //     100.0, 100.0,
        // )))
        .style_mask(StyleMask::empty().nonactivating_panel().into())
        .level(PanelLevel::Floating) // PopUpMenu 级别适合菜单
        // .hides_on_deactivate(true) // 不要在失去焦点时隐藏
        // .collection_behavior(CollectionBehavior::new().can_join_all_spaces().stationary())
        // .released_when_closed(true)
        .with_window(|window| {
            window
                .resizable(false)
                .decorations(false)
                .transparent(false)
                .visible(false)
                .skip_taskbar(true)
        })
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build menu panel: {}", e);
            e
        }).unwrap();

    // 允许在模态对话框运行时接收事件
    // menu_panel.set_works_when_modal(true);

    // 设置事件处理器
    let handler = FloatingEventHandler::new();

    // 监听鼠标进入事件 - 让面板成为 key window 以接收点击
    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Menu panel became key window");
    });


      
    let handle_clone = app_handle.clone();
    handler.window_did_resign_key(move |_| {
      println!("[info]: panel resigned from key window!");
      if let Ok(p) = handle_clone.get_webview_panel("floating") {
        p.hide();
      }
    });

    menu_panel.set_event_handler(Some(handler.as_ref()));

}

/// 初始化选择面板
#[cfg(target_os = "macos")]
pub fn setup_selection_panels(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    init_menu_panel1(app);
    Ok(())
}
