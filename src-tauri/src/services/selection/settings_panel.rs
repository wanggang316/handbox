use std::sync::atomic::{AtomicBool, Ordering};
use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt
};

/// 跟踪设置面板是否可见
static SETTINGS_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);
/// 跟踪鼠标是否在面板内部
static MOUSE_INSIDE_PANEL: AtomicBool = AtomicBool::new(false);

const PANEL_LABEL: &str = "selection_settings";

tauri_panel! {
    panel!(SelectionSettingsPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
        }
        with: {
            tracking_area: {
                options: TrackingAreaOptions::new()
                    .active_always()
                    .mouse_entered_and_exited()
                    .cursor_update(),
                auto_resize: true
            }
        }
    })

    panel_event!(SelectionSettingsEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn init_panel(app_handle: &AppHandle) {
    let window = app_handle.get_webview_window(PANEL_LABEL.into()).unwrap();
    let panel = window.to_panel::<SelectionSettingsPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_corner_radius(12.0);

    let handler = SelectionSettingsEventHandler::new();
    handler.on_mouse_entered(move |_event| {
        MOUSE_INSIDE_PANEL.store(true, Ordering::Relaxed);
        tracing::debug!("Mouse entered settings panel");
    });

    handler.on_mouse_exited(move |_event| {
        MOUSE_INSIDE_PANEL.store(false, Ordering::Relaxed);
        tracing::debug!("Mouse exited settings panel");
    });
    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Settings panel became key window");
    });
    handler.window_did_resign_key(move |_| {
        tracing::debug!("Settings panel resigned from key window");
    });

    panel.set_works_when_modal(true);
    panel.set_accepts_mouse_moved_events(true);
    panel.set_event_handler(Some(handler.as_ref()));
}

pub fn show_panel(handle: &AppHandle, x: f64, y: f64) {
    SETTINGS_PANEL_VISIBLE.store(true, Ordering::Relaxed);
    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            let _ = window.set_position(LogicalPosition::new(x, y));
            if !window.is_visible().unwrap_or(true) {
                tracing::info!("Settings panel is not visible, showing {} x: {}, y: {}", PANEL_LABEL, x, y);
                let _ = window.show();
            }
        }
    });
}

pub fn hide_panel(handle: &AppHandle) {
    SETTINGS_PANEL_VISIBLE.store(false, Ordering::Relaxed);
    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            let _ = window.set_position(LogicalPosition::new(-9999.0, -9999.0));
            if window.is_visible().unwrap_or(true) {
                let _ = window.hide();
            }
        }
    });
}

pub fn is_panel_visible() -> bool {
    SETTINGS_PANEL_VISIBLE.load(Ordering::Relaxed)
}

pub fn is_mouse_inside() -> bool {
    MOUSE_INSIDE_PANEL.load(Ordering::Relaxed)
}
