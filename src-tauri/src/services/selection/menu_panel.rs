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

pub fn show_panel(handle: &AppHandle, x: f64, y: f64) {
    // 立即更新标志，这样 mouse hook 线程可以快速感知
    MENU_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        tracing::info!("Showing menu panel: {}", PANEL_LABEL);

        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            // 菜单面板尺寸（需与 tauri.conf.json 中的配置一致）
            const PANEL_WIDTH: f64 = 320.0;
            const PANEL_HEIGHT: f64 = 36.0;
            const VERTICAL_GAP: f64 = 20.0; // 鼠标和面板之间的间隙

            // 计算初始位置（居中在鼠标上方，留出间隙）
            let mut target_x = x - PANEL_WIDTH / 2.0;
            let mut target_y = y - PANEL_HEIGHT - VERTICAL_GAP;

            tracing::info!(
                "Initial calculation: mouse({}, {}), panel_size({}, {}), initial_target({}, {})",
                x,
                y,
                PANEL_WIDTH,
                PANEL_HEIGHT,
                target_x,
                target_y
            );

            // 获取所有屏幕，找到包含鼠标位置的屏幕
            match window.available_monitors() {
                Ok(monitors) => {
                    // 找到包含鼠标位置的屏幕
                    let mut found_monitor = None;
                    for monitor in monitors.iter() {
                        let scale_factor = monitor.scale_factor();
                        let screen_x = monitor.position().x as f64 / scale_factor;
                        let screen_y = monitor.position().y as f64 / scale_factor;
                        let screen_width = monitor.size().width as f64 / scale_factor;
                        let screen_height = monitor.size().height as f64 / scale_factor;

                        // 检查鼠标是否在这个屏幕范围内
                        if x >= screen_x
                            && x < screen_x + screen_width
                            && y >= screen_y
                            && y < screen_y + screen_height
                        {
                            found_monitor = Some(monitor);
                            break;
                        }
                    }

                    if let Some(monitor) = found_monitor {
                        let scale_factor = monitor.scale_factor();
                        let monitor_pos = monitor.position();
                        let monitor_size = monitor.size();

                        tracing::info!(
                            "Monitor detected: position({}, {}), size({}, {}), scale_factor({})",
                            monitor_pos.x,
                            monitor_pos.y,
                            monitor_size.width,
                            monitor_size.height,
                            scale_factor
                        );

                        // 屏幕物理尺寸和位置（转换为逻辑坐标）
                        let screen_x = monitor_pos.x as f64 / scale_factor;
                        let screen_y = monitor_pos.y as f64 / scale_factor;
                        let screen_width = monitor_size.width as f64 / scale_factor;
                        let screen_height = monitor_size.height as f64 / scale_factor;

                        // 计算该屏幕的边界
                        let min_x = screen_x;
                        let max_x = screen_x + screen_width - PANEL_WIDTH;
                        let min_y = screen_y;
                        let max_y = screen_y + screen_height - PANEL_HEIGHT;

                        tracing::info!(
                            "Screen logical coords: position({}, {}), size({}, {})",
                            screen_x,
                            screen_y,
                            screen_width,
                            screen_height
                        );
                        tracing::info!(
                            "Calculated bounds: x[{} to {}], y[{} to {}]",
                            min_x,
                            max_x,
                            min_y,
                            max_y
                        );

                        let old_x = target_x;
                        let old_y = target_y;

                        // 限制到屏幕范围内
                        target_x = target_x.max(min_x).min(max_x);
                        target_y = target_y.max(min_y).min(max_y);

                        tracing::info!(
                            "Position clamping: ({}, {}) -> ({}, {})",
                            old_x,
                            old_y,
                            target_x,
                            target_y
                        );
                    } else {
                        tracing::warn!("No monitor contains mouse position ({}, {})", x, y);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to get available monitors: {:?}", e);
                }
            }

            tracing::info!("Setting panel position to ({}, {})", target_x, target_y);
            let _ = window.set_position(LogicalPosition::new(target_x, target_y));

            if !window.is_visible().unwrap_or(true) {
                let _ = window.show();
                tracing::info!("-----> show menu panel at ({}, {})", target_x, target_y);
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
