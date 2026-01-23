// Selection panel types and management using tauri-nspanel

use tauri::{LogicalSize, Size, WebviewUrl};
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_nspanel::{CollectionBehavior, ManagerExt, PanelBuilder, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt, tauri_panel};

tauri_panel! {
    // 定义面板类：允许成为 Key Window（这样按钮才能点击），但不允许成为 Main Window
    panel!(FloatingPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            // ignores_mouse_events: false,
            is_floating_panel: true
            // borderless: true,
        }
        with: {
            // Enable mouse tracking for the panel
            tracking_area: {
                options: TrackingAreaOptions::new()
                    .active_always()           // Track mouse even when app is not active
                    .mouse_entered_and_exited() // Get notified when mouse enters/exits
                    .mouse_moved()             // Track mouse movement
                    .cursor_update(),          // Track cursor updates
                auto_resize: true               // Resize tracking area with window
            }
        }
    })

    panel_event!(FloatingEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn init_menu_panel1(app_handle: &AppHandle) {
    tracing::info!("Setting up selection panels");

    // 创建菜单面板
    let menu_panel = PanelBuilder::<tauri::Wry, FloatingPanel>::new(app_handle, "floating")
        .url(WebviewUrl::App("/floating".into()))
        .title("floating")
        .size(Size::Logical(LogicalSize::new(360.0, 36.0)))
        // .position(tauri::Position::Logical(tauri::LogicalPosition::new(
        //     100.0, 100.0,
        // )))
        .style_mask(StyleMask::empty().nonactivating_panel().borderless().into())
        .level(PanelLevel::Floating) // PopUpMenu 级别适合菜单
        // .hides_on_deactivate(true) // 不要在失去焦点时隐藏
        // .collection_behavior(CollectionBehavior::new().can_join_all_spaces().stationary())
        // .released_when_closed(true)
        .corner_radius(18.0)
        // .has_shadow(false)
        .with_window(|window| {
            window
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .transparent(true)
                .visible(false)
                .skip_taskbar(true)
        })
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build menu panel: {}", e);
            e
        }).unwrap();

    // 允许接收鼠标移动事件（用于 hover 效果）
    menu_panel.set_accepts_mouse_moved_events(true);

    // 设置事件处理器
    let handler = FloatingEventHandler::new();

    // let handle = app_handle.clone();
    handler.on_mouse_entered(move |_event| {
        println!("🐭 Mouse entered the panel, make it key!");
    
        // let panel = handle.get_webview_panel("flating").unwrap();
        // panel.make_key_window();
      });
    
    //   let handle = app_handle.to_owned();
      handler.on_mouse_exited(move |_event| {
        println!("👋 Mouse exited the panel!");
    
        // let panel = handle.get_webview_panel("flating").unwrap();
        // panel.resign_key_window();
      });


    // 监听鼠标进入事件 - 让面板成为 key window 以接收点击
    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Menu panel became key window");
    });

    // let handle_clone = app_handle.clone();
    handler.window_did_resign_key(move |_| {
      println!("[info]: panel resigned from key window!");
    });

    menu_panel.set_works_when_modal(true);

    menu_panel.set_event_handler(Some(handler.as_ref()));

    menu_panel.set_accepts_mouse_moved_events(true);
}

/// 初始化选择面板
#[cfg(target_os = "macos")]
pub fn setup_selection_panels(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {

    init_menu_panel1(app);
    Ok(())
}
