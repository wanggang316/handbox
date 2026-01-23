#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri::{LogicalSize, Size, WebviewUrl};
use tauri_nspanel::{
    ManagerExt, WebviewWindowExt, PanelLevel, StyleMask, TrackingAreaOptions, tauri_panel
};

const PANEL_LABEL: &str = "selection_menu";

tauri_panel! {
    panel!(SelectionMenuPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
        }
        // with: {
        //     tracking_area: {
        //         options: TrackingAreaOptions::new()
        //             .active_always()
        //             .mouse_entered_and_exited()
        //             .mouse_moved()
        //             .cursor_update(),
        //         auto_resize: true
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
    // let panel = PanelBuilder::<tauri::Wry, SelectionMenuPanel>::new(app_handle, PANEL_LABEL)
    //     .url(WebviewUrl::App(PANEL_URL.into()))
    //     .title(PANEL_TITLE)
    //     .size(Size::Logical(LogicalSize::new(360.0, 36.0)))
    //     .style_mask(StyleMask::empty().nonactivating_panel().borderless().into())
    //     .level(PanelLevel::Floating) 
    //     // .hides_on_deactivate(false)
    //     // .collection_behavior(CollectionBehavior::new().can_join_all_spaces().stationary())
    //     // .released_when_closed(true)
    //     .corner_radius(18.0)
    //     // .has_shadow(false)
    //     .with_window(|window| {
    //         window
    //             .resizable(false)
    //             .decorations(false)
    //             .always_on_top(true)
    //             .transparent(true)
    //             .visible(false)
    //             .skip_taskbar(true)
    //     })
    //     .build()
    //     .map_err(|e| {
    //         tracing::error!("Failed to build menu panel: {}", e);
    //         e
    //     })
    //     .unwrap();

    let window = app_handle.get_webview_window(PANEL_LABEL.into()).unwrap();
    let panel = window.to_panel::<SelectionMenuPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().borderless().into());
    panel.set_corner_radius(18.0);

    let handler = SelectionMenuEventHandler::new();
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