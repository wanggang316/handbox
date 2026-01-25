use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{
    PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt, tauri_panel
};
use std::sync::atomic::{AtomicBool, Ordering};

/// 跟踪内容面板是否可见
static CONTENT_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);

/// 跟踪内容面板是否置顶（置顶时点击外部不会隐藏）
static CONTENT_PANEL_PINNED: AtomicBool = AtomicBool::new(false);

/// 跟踪鼠标是否在面板内部
static MOUSE_INSIDE_PANEL: AtomicBool = AtomicBool::new(false);

const PANEL_LABEL: &str = "selection_content";

tauri_panel! {
    panel!(SelectionContentPanel {
        config: {
            can_become_key_window: true,  // 允许接收键盘事件（复制快捷键等）
            can_become_main_window: false,
        }
        with: {
            // Enable mouse tracking for the panel
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

    let window = app_handle.get_webview_window(PANEL_LABEL.into()).unwrap();
    let panel = window.to_panel::<SelectionContentPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_corner_radius(18.0);

    // 设置事件处理器
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
        // 非置顶状态下，失去焦点时隐藏面板（如用户切换程序）
        if !CONTENT_PANEL_PINNED.load(Ordering::Relaxed) && CONTENT_PANEL_VISIBLE.load(Ordering::Relaxed) {
            tracing::info!("-----> hiding content panel (lost focus)");
            // 更新标志
            CONTENT_PANEL_VISIBLE.store(false, Ordering::Relaxed);
            // 隐藏窗口
            let h = handle_for_resign.clone();
            let h2 = h.clone();
            let _ = h.run_on_main_thread(move || {
                if let Some(window) = h2.get_webview_window(PANEL_LABEL.into()) {
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
    // 立即更新标志
    CONTENT_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        tracing::info!("Showing content panel: {}", PANEL_LABEL);

        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            let _ = window.set_position(LogicalPosition::new(x - 180.0, y + 8.0));
            let _ = window.show();
        }
    });
}

pub fn hide_panel(handle: &AppHandle) {
    // 立即更新标志
    CONTENT_PANEL_VISIBLE.store(false, Ordering::Relaxed);
    // 隐藏时重置置顶状态
    CONTENT_PANEL_PINNED.store(false, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL.into()) {
            let _ = window.set_position(LogicalPosition::new(-9999.0, -9999.0));
            let _ = window.hide();
        }
    });
}


/// 检查内容面板是否可见
pub fn is_panel_visible() -> bool {
    CONTENT_PANEL_VISIBLE.load(Ordering::Relaxed)
}

/// 检查内容面板是否置顶
pub fn is_panel_pinned() -> bool {
    CONTENT_PANEL_PINNED.load(Ordering::Relaxed)
}

/// 检查鼠标是否在面板内部
pub fn is_mouse_inside() -> bool {
    MOUSE_INSIDE_PANEL.load(Ordering::Relaxed)
}

/// 设置内容面板置顶状态
pub fn set_panel_pinned(pinned: bool) {
    CONTENT_PANEL_PINNED.store(pinned, Ordering::Relaxed);
}