use std::sync::atomic::{AtomicBool, Ordering};
use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt};
use crate::services::selection::settings_panel::hide_panel as hide_settings_panel;

/// 跟踪菜单面板是否可见（用于在 mouse hook 线程中快速检查）
static MENU_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);

const PANEL_LABEL: &str = "selection_menu";

tauri_panel! {
    panel!(SelectionMenuPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
        }
    })

    panel_event!(SelectionMenuEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

pub fn init_panel(app_handle: &AppHandle) {
    tracing::info!("Setting up selection panels");

    let window = app_handle.get_webview_window(PANEL_LABEL.into()).unwrap();
    let panel = window.to_panel::<SelectionMenuPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_corner_radius(18.0);

    // panel.set_becomes_key_only_if_needed(true);

    let handler = SelectionMenuEventHandler::new();
    handler.window_did_become_key(move |_notification| {
        tracing::info!("Menu panel became key window");
    });

    // let handle_clone = app_handle.clone();
    handler.window_did_resign_key(move |_| {
        tracing::info!("Menu panel resigned from key window!");
    });

    panel.set_works_when_modal(true);
    panel.set_accepts_mouse_moved_events(true);
    panel.set_event_handler(Some(handler.as_ref()));
}

pub fn show_panel(handle: &AppHandle, x: f64, y: f64) {
    // 立即更新标志，这样 mouse hook 线程可以快速感知
    MENU_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        tracing::info!("Showing menu panel: {}", PANEL_LABEL);

        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            let _ = window.set_position(LogicalPosition::new(x - 180.0, y - 56.0));
            if !window.is_visible().unwrap_or(true) {
                let _ = window.show();
                tracing::info!("-----> show menu panel successfully");
            }
        }
    });
}

pub fn hide_panel(handle: &AppHandle) {
    // 立即更新标志，这样 mouse hook 线程可以快速感知
    MENU_PANEL_VISIBLE.store(false, Ordering::Relaxed);
    hide_settings_panel(handle);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            // 先移到屏幕外，避免下次显示时在旧位置闪烁
            let _ = window.set_position(LogicalPosition::new(-9999.0, -9999.0));
            if window.is_visible().unwrap_or(true) {
                let _ = window.hide();
                tracing::info!("-----> hide menu panel successfully");
            }
        }
    });
}

/// 检查菜单面板是否可见
pub fn is_panel_visible() -> bool {
    MENU_PANEL_VISIBLE.load(Ordering::Relaxed)
}