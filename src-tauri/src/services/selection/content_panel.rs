#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{
    ManagerExt, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt, tauri_panel
};

const PANEL_LABEL: &str = "selection_content";

tauri_panel! {
    panel!(SelectionContentPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
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

    panel_event!(SelectionContentEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn init_panel(app_handle: &AppHandle) {
    tracing::info!("Setting up selection panels");

    let window = app_handle.get_webview_window(PANEL_LABEL.into()).unwrap();
    let panel = window.to_panel::<SelectionContentPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().borderless().into());
    panel.set_corner_radius(18.0);

    // 设置事件处理器
    let handler = SelectionContentEventHandler::new();

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
        if let Ok(panel) = handle_clone.get_webview_panel(PANEL_LABEL) {
            if !panel.is_visible() {
                panel.show();
            }
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