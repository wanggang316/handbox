// The panel_event! DSL requires an explicit `-> ()` (Obj-C void delegate), which
// trips clippy::unused_unit inside the macro expansion; only a module-level allow
// reaches macro-generated code.
#![allow(clippy::unused_unit)]

use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{
    CollectionBehavior, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt, tauri_panel
};
use std::sync::atomic::{AtomicBool, Ordering};

static CONTENT_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);

/// While pinned, outside clicks and focus loss do not hide the panel.
static CONTENT_PANEL_PINNED: AtomicBool = AtomicBool::new(false);

static MOUSE_INSIDE_PANEL: AtomicBool = AtomicBool::new(false);

const PANEL_LABEL: &str = "selection_content";

tauri_panel! {
    panel!(SelectionContentPanel {
        config: {
            can_become_key_window: true,  // accepts keyboard events, e.g. copy shortcuts
            can_become_main_window: false,
        }
        with: {
            tracking_area: {
                options: TrackingAreaOptions::new()
                    .active_always()           // Track mouse even when app is not active
                    .mouse_entered_and_exited() // Get notified when mouse enters/exits
                    .cursor_update(),          // Track cursor updates
                auto_resize: true               // Resize tracking area with window
            }
        }
    })

    panel_event!(SelectionContentEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn init_panel(app_handle: &AppHandle) {
    tracing::info!("Setting up selection panels");

    let window = app_handle.get_webview_window(PANEL_LABEL).unwrap();
    let panel = window.to_panel::<SelectionContentPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_corner_radius(18.0);

    let handler = SelectionContentEventHandler::new();

    handler.on_mouse_entered(move |_event| {
        MOUSE_INSIDE_PANEL.store(true, Ordering::Relaxed);
        tracing::debug!("Mouse entered content panel");
    });

    handler.on_mouse_exited(move |_event| {
        MOUSE_INSIDE_PANEL.store(false, Ordering::Relaxed);
        tracing::debug!("Mouse exited content panel");
    });

    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Content panel became key window");
    });

    let handle_for_resign = app_handle.clone();
    handler.window_did_resign_key(move |_| {
        tracing::debug!("Content panel resigned from key window");
        // An unpinned panel hides on focus loss, e.g. when the user switches apps.
        if !CONTENT_PANEL_PINNED.load(Ordering::Relaxed) && CONTENT_PANEL_VISIBLE.load(Ordering::Relaxed) {
            tracing::info!("-----> hiding content panel (lost focus)");
            CONTENT_PANEL_VISIBLE.store(false, Ordering::Relaxed);
            let h = handle_for_resign.clone();
            let h2 = h.clone();
            let _ = h.run_on_main_thread(move || {
                if let Some(window) = h2.get_webview_window(PANEL_LABEL) {
                    let _ = window.set_position(LogicalPosition::new(-9999.0, -9999.0));
                    let _ = window.hide();
                }
            });
        }
    });

    panel.set_works_when_modal(true);
    panel.set_accepts_mouse_moved_events(true);
    panel.set_event_handler(Some(handler.as_ref()));
}

pub fn show_panel(handle: &AppHandle, x: f64, y: f64) {
    // Set before dispatching, so the mouse-hook thread sees it right away.
    CONTENT_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        tracing::info!("Showing content panel: {}", PANEL_LABEL);

        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL) {
            let _ = window.set_position(LogicalPosition::new(x - 20.0, y - 250.0));
            let _ = window.show();
        }
    });
}

pub fn hide_panel(handle: &AppHandle) {
    CONTENT_PANEL_VISIBLE.store(false, Ordering::Relaxed);
    CONTENT_PANEL_PINNED.store(false, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL) {
            let _ = window.set_position(LogicalPosition::new(-9999.0, -9999.0));
            let _ = window.hide();
        }
    });
}


pub fn is_panel_visible() -> bool {
    CONTENT_PANEL_VISIBLE.load(Ordering::Relaxed)
}

pub fn is_panel_pinned() -> bool {
    CONTENT_PANEL_PINNED.load(Ordering::Relaxed)
}

pub fn is_mouse_inside() -> bool {
    MOUSE_INSIDE_PANEL.load(Ordering::Relaxed)
}

pub fn set_panel_pinned(pinned: bool) {
    CONTENT_PANEL_PINNED.store(pinned, Ordering::Relaxed);
}