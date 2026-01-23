#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri::{LogicalSize, Size, WebviewUrl};
use tauri_nspanel::{
    ManagerExt, PanelBuilder, PanelLevel, StyleMask, tauri_panel
};

const PANEL_LABEL: &str = "selection_menu";
const PANEL_URL: &str = "/selection/menu";
const PANEL_TITLE: &str = "Selection Menu";

tauri_panel! {
    panel!(SelectionMenuPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            // ignores_mouse_events: false,
            // is_floating_panel: true
        }
        // with: {
        //     // Enable mouse tracking for the panel
        //     tracking_area: {
        //         options: TrackingAreaOptions::new()
        //             .active_always()           // Track mouse even when app is not active
        //             .mouse_entered_and_exited() // Get notified when mouse enters/exits
        //             .mouse_moved()             // Track mouse movement
        //             .cursor_update(),          // Track cursor updates
        //         auto_resize: true               // Resize tracking area with window
        //     }
        // }
    })

    panel_event!(SelectionMenuEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn init_panel(app_handle: &AppHandle) {
    tracing::info!("Setting up selection panels");

    // 创建菜单面板
    let panel = PanelBuilder::<tauri::Wry, SelectionMenuPanel>::new(app_handle, PANEL_LABEL)
        .url(WebviewUrl::App(PANEL_URL.into()))
        .title(PANEL_TITLE)
        .size(Size::Logical(LogicalSize::new(360.0, 36.0)))
        .style_mask(StyleMask::empty().nonactivating_panel().borderless().into())
        .level(PanelLevel::Floating) 
        // .hides_on_deactivate(true)
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
        })
        .unwrap();

    // 允许接收鼠标移动事件（用于 hover 效果）
    panel.set_accepts_mouse_moved_events(true);

    // 设置事件处理器
    let handler = SelectionMenuEventHandler::new();

    // let handle = app_handle.clone();
    handler.on_mouse_entered(move |_event| {
        println!("🐭 Mouse entered the panel, make it key!");
    });

    //   let handle = app_handle.to_owned();
    handler.on_mouse_exited(move |_event| {
        println!("👋 Mouse exited the panel!");
    });

    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Menu panel became key window");
    });

    // let handle_clone = app_handle.clone();
    handler.window_did_resign_key(move |_| {
        println!("[info]: panel resigned from key window!");
    });

    panel.set_works_when_modal(true);
    panel.set_accepts_mouse_moved_events(true);
    panel.set_event_handler(Some(handler.as_ref()));
}

pub fn show_panel(handle: &AppHandle) {
    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        tracing::info!("Showing menu panel: {}", PANEL_LABEL);
        if let Ok(panel) = handle_clone.get_webview_panel(PANEL_LABEL) {
            tracing::info!("Menu panel is visible: {}", panel.is_visible());
            if !panel.is_visible() {
                let _ = panel.show();
                tracing::info!("Menu panel shown successfully");
            } else {
                tracing::info!("Menu panel is already visible");
            }
        } else {
            tracing::error!("Failed to get menu panel: {}", PANEL_LABEL);
        }
    });
}

pub fn hide_panel(handle: &AppHandle) {
    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Ok(panel) = handle_clone.get_webview_panel(PANEL_LABEL) {
            if panel.is_visible() {
                let _ = panel.hide();
            }
        }
    });
}