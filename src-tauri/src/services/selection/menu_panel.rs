use crate::services::selection::settings_panel::hide_panel as hide_settings_panel;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelLevel, StyleMask, WebviewWindowExt};

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

/// 计算面板位置，确保不超出屏幕边界
fn calculate_panel_position(
    window: &tauri::WebviewWindow,
    mouse_x: f64,
    mouse_y: f64,
    panel_width: f64,
    panel_height: f64,
    vertical_gap: f64,
) -> (f64, f64) {
    // 计算初始位置（居中在鼠标上方，留出间隙）
    let mut target_x = mouse_x - panel_width / 2.0;
    let mut target_y = mouse_y - panel_height - vertical_gap;

    // 获取所有屏幕，找到包含鼠标位置的屏幕
    if let Ok(monitors) = window.available_monitors() {
        for monitor in monitors.iter() {
            let scale_factor = monitor.scale_factor();
            let screen_x = monitor.position().x as f64 / scale_factor;
            let screen_y = monitor.position().y as f64 / scale_factor;
            let screen_width = monitor.size().width as f64 / scale_factor;
            let screen_height = monitor.size().height as f64 / scale_factor;

            // 检查鼠标是否在这个屏幕范围内
            if mouse_x >= screen_x
                && mouse_x < screen_x + screen_width
                && mouse_y >= screen_y
                && mouse_y < screen_y + screen_height
            {
                // 计算该屏幕的边界
                let min_x = screen_x;
                let max_x = screen_x + screen_width - panel_width;
                let min_y = screen_y;
                let max_y = screen_y + screen_height - panel_height;

                // 限制到屏幕范围内
                target_x = target_x.max(min_x).min(max_x);
                target_y = target_y.max(min_y).min(max_y);
                break;
            }
        }
    }

    (target_x, target_y)
}

pub fn show_panel(handle: &AppHandle, x: f64, y: f64) {
    MENU_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            const PANEL_WIDTH: f64 = 320.0;
            const PANEL_HEIGHT: f64 = 36.0;
            const VERTICAL_GAP: f64 = 20.0;

            let (target_x, target_y) =
                calculate_panel_position(&window, x, y, PANEL_WIDTH, PANEL_HEIGHT, VERTICAL_GAP);

            let _ = window.set_position(LogicalPosition::new(target_x, target_y));
            if !window.is_visible().unwrap_or(true) {
                let _ = window.show();
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
