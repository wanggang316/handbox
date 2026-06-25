//! Quick Action 浮层窗口（macOS NSPanel）。
//!
//! 与 `content_panel.rs` 同源：frameless / transparent / always-on-top /
//! non-activating，可悬浮于 macOS 全屏应用之上而不离开当前 Space，居中于鼠标
//! 所在显示器的上三分之一，失焦（resign-key）自动隐藏。`show_panel` 先定位再
//! `window.show()`；`hide_panel` 先把窗口移到屏外再 `window.hide()`，避免下次
//! 显示时在旧位置闪烁。
//!
//! 可见性标志在派发到主线程之前先行写入（进程级 `AtomicBool`），使其他线程能
//! 快速、无锁地感知状态。

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Manager};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt,
};

/// 跟踪 Quick Action 面板是否可见。
static QUICK_ACTION_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);

const PANEL_LABEL: &str = "quick_action";

/// 面板逻辑尺寸（须与 tauri.conf.json 中的窗口声明保持一致）。
const PANEL_WIDTH: f64 = 720.0;
const PANEL_HEIGHT: f64 = 480.0;

tauri_panel! {
    panel!(QuickActionPanel {
        config: {
            can_become_key_window: true, // 接收键盘输入（后续 composer 需要）
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

    panel_event!(QuickActionEventHandler {
        window_did_become_key(notification: &NSNotification) -> (),
        window_did_resign_key(notification: &NSNotification) -> (),
    })
}

#[cfg(target_os = "macos")]
pub fn init_panel(app_handle: &AppHandle) {
    tracing::info!("Setting up quick action panel");

    let window = app_handle.get_webview_window(PANEL_LABEL).unwrap();
    let panel = window.to_panel::<QuickActionPanel>().unwrap();
    panel.set_level(PanelLevel::Floating.value());
    // can_join_all_spaces + full_screen_auxiliary：悬浮于全屏 app 之上且不切走 Space
    // (VAL-OVERLAY-005)。
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    // nonactivating：显示面板不抢占前台 app 的激活态 (VAL-OVERLAY-021 的 frameless
    // 体验由 decorations:false + transparent:true + 此 style mask 共同保证)。
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_corner_radius(18.0);

    let handler = QuickActionEventHandler::new();

    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Quick action panel became key window");
    });

    // 失焦自动隐藏（VAL-OVERLAY-004）。先写可见性标志，再派发主线程隐藏。
    let handle_for_resign = app_handle.clone();
    handler.window_did_resign_key(move |_| {
        if QUICK_ACTION_PANEL_VISIBLE.load(Ordering::Relaxed) {
            tracing::info!("hiding quick action panel (lost focus)");
            QUICK_ACTION_PANEL_VISIBLE.store(false, Ordering::Relaxed);
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

/// 显示面板：根据鼠标所在显示器把面板水平居中、置于上三分之一并边缘夹紧，然后
/// `make_key_and_order_front` 取得键盘焦点。`cursor_phys_x` / `cursor_phys_y` 为
/// **物理像素**坐标系下的全局鼠标位置（来自 `AppHandle::cursor_position()`）。
#[cfg(target_os = "macos")]
pub fn show_panel(handle: &AppHandle, cursor_phys_x: f64, cursor_phys_y: f64) {
    // 立即更新标志（resign-key handler 据此判断是否需要隐藏）。
    QUICK_ACTION_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL) {
            // 在物理坐标系下解析鼠标所在显示器，再换算到逻辑坐标系（点）。
            // `set_position(LogicalPosition)` 与 `calculate_panel_position` 都工作
            // 在逻辑坐标系，故定位计算前统一换算。
            let (frame, cursor_logical_x) =
                resolve_cursor_monitor(&window, cursor_phys_x, cursor_phys_y);
            let (target_x, target_y) =
                calculate_panel_position(frame, cursor_logical_x, PANEL_WIDTH, PANEL_HEIGHT);

            let _ = window.set_position(LogicalPosition::new(target_x, target_y));
            let _ = window.show();

            // 取得键盘焦点，使 resign-key 自动隐藏成立 (VAL-OVERLAY-004)。
            if let Ok(panel) = window.to_panel::<QuickActionPanel>() {
                panel.make_key_and_order_front();
            }
        }
    });
}

/// 隐藏面板：先移到屏外避免下次显示闪烁，再 `hide()`。
#[cfg(target_os = "macos")]
pub fn hide_panel(handle: &AppHandle) {
    QUICK_ACTION_PANEL_VISIBLE.store(false, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL) {
            let _ = window.set_position(LogicalPosition::new(-9999.0, -9999.0));
            let _ = window.hide();
        }
    });
}

/// 切换面板可见性。`cursor_phys_x` / `cursor_phys_y`（物理像素）仅在转为显示时
/// 用于定位。
#[cfg(target_os = "macos")]
pub fn toggle(handle: &AppHandle, cursor_phys_x: f64, cursor_phys_y: f64) {
    if is_panel_visible() {
        hide_panel(handle);
    } else {
        show_panel(handle, cursor_phys_x, cursor_phys_y);
    }
}

/// 检查 Quick Action 面板是否可见。
pub fn is_panel_visible() -> bool {
    QUICK_ACTION_PANEL_VISIBLE.load(Ordering::Relaxed)
}

/// 一个显示器在逻辑坐标系下的矩形（点为单位，原点在左上）。
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 在物理坐标系下找到包含鼠标的显示器，返回其**逻辑**矩形与鼠标的**逻辑** x。
/// 多显示器在物理空间匹配（各屏 DPI 可能不同），匹配后用该屏 scale 统一换算到
/// 逻辑坐标系。找不到时回退到第一块显示器，再退回到以鼠标为中心、面板大小的一
/// 块虚拟矩形（保证 `calculate_panel_position` 仍能给出可见结果）。
#[cfg(target_os = "macos")]
fn resolve_cursor_monitor(
    window: &tauri::WebviewWindow,
    cursor_phys_x: f64,
    cursor_phys_y: f64,
) -> (MonitorFrame, f64) {
    if let Ok(monitors) = window.available_monitors() {
        let mut first: Option<(MonitorFrame, f64)> = None;
        for monitor in monitors.iter() {
            let scale = monitor.scale_factor();
            let pos_x = monitor.position().x as f64;
            let pos_y = monitor.position().y as f64;
            let width_phys = monitor.size().width as f64;
            let height_phys = monitor.size().height as f64;

            let frame = MonitorFrame {
                x: pos_x / scale,
                y: pos_y / scale,
                width: width_phys / scale,
                height: height_phys / scale,
            };
            let cursor_logical_x = cursor_phys_x / scale;
            if first.is_none() {
                first = Some((frame, cursor_logical_x));
            }
            // 在物理空间判断鼠标归属（各屏 origin/size 均为物理像素）。
            if cursor_phys_x >= pos_x
                && cursor_phys_x < pos_x + width_phys
                && cursor_phys_y >= pos_y
                && cursor_phys_y < pos_y + height_phys
            {
                return (frame, cursor_logical_x);
            }
        }
        if let Some(result) = first {
            return result;
        }
    }

    // 极端回退：假设 scale = 1，以鼠标为中心给出一块刚好容纳面板的矩形。
    (
        MonitorFrame {
            x: cursor_phys_x - PANEL_WIDTH / 2.0,
            y: cursor_phys_y - PANEL_HEIGHT / 2.0,
            width: PANEL_WIDTH,
            height: PANEL_HEIGHT,
        },
        cursor_phys_x,
    )
}

/// 纯函数：在给定显示器矩形内，把面板水平居中、纵向置于上三分之一处，并夹紧到
/// 屏幕范围内使其完整可见。
///
/// - 水平：默认居中于鼠标所在显示器（VAL-OVERLAY-008），随后夹紧到 `[x, x+width-panel]`
///   使其不越界（VAL-OVERLAY-009）；当面板比屏幕宽时退化为左对齐屏幕原点。
/// - 纵向：目标顶边设在显示器高度的 1/3 处（“上三分之一”），同样夹紧到屏内。
#[cfg(any(target_os = "macos", test))]
fn calculate_panel_position(
    frame: MonitorFrame,
    cursor_x: f64,
    panel_width: f64,
    panel_height: f64,
) -> (f64, f64) {
    // 水平居中于鼠标，随后整体夹紧进屏幕。
    let mut target_x = cursor_x - panel_width / 2.0;
    let min_x = frame.x;
    let max_x = (frame.x + frame.width - panel_width).max(min_x);
    target_x = target_x.clamp(min_x, max_x);

    // 纵向：上三分之一。顶边设在屏幕高度的 1/3 处。
    let mut target_y = frame.y + frame.height / 3.0;
    let min_y = frame.y;
    let max_y = (frame.y + frame.height - panel_height).max(min_y);
    target_y = target_y.clamp(min_y, max_y);

    (target_x, target_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f64, y: f64, w: f64, h: f64) -> MonitorFrame {
        MonitorFrame {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn centers_horizontally_on_cursor_within_screen() {
        // 1920x1080 主屏，鼠标居中：面板应水平居中、顶边在 1/3 高度处。
        let (x, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 1080.0), 960.0, 720.0, 480.0);
        assert_eq!(x, 960.0 - 360.0); // 600.0
        assert_eq!(y, 360.0); // 1080 / 3
    }

    #[test]
    fn clamps_to_left_edge_when_cursor_near_left() {
        // 鼠标贴左边：水平居中会越过左界，应夹紧到屏幕左缘。
        let (x, _) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 1080.0), 10.0, 720.0, 480.0);
        assert_eq!(x, 0.0);
    }

    #[test]
    fn clamps_to_right_edge_when_cursor_near_right() {
        // 鼠标贴右边：水平居中会越过右界，应夹紧到 (width - panel)。
        let (x, _) =
            calculate_panel_position(frame(0.0, 0.0, 1920.0, 1080.0), 1910.0, 720.0, 480.0);
        assert_eq!(x, 1920.0 - 720.0); // 1200.0
    }

    #[test]
    fn stays_fully_on_screen_vertically() {
        // 矮屏：1/3 处加面板高度会越过底界，纵向应夹紧到 (height - panel)。
        let (_, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 600.0), 960.0, 720.0, 480.0);
        // 1/3 == 200, 200 + 480 = 680 > 600 → 夹紧到 600 - 480 = 120
        assert_eq!(y, 120.0);
    }

    #[test]
    fn honors_monitor_origin_offset() {
        // 第二块显示器原点在 (1920, 0)：定位应相对该显示器原点计算。
        let (x, y) = calculate_panel_position(
            frame(1920.0, 0.0, 1920.0, 1080.0),
            1920.0 + 960.0,
            720.0,
            480.0,
        );
        assert_eq!(x, 1920.0 + 600.0);
        assert_eq!(y, 360.0);
    }

    #[test]
    fn negative_origin_monitor_clamps_within_bounds() {
        // 左侧扩展屏原点为负：贴左边时夹紧到该屏左缘（负坐标）。
        let (x, _) =
            calculate_panel_position(frame(-1920.0, 0.0, 1920.0, 1080.0), -1910.0, 720.0, 480.0);
        assert_eq!(x, -1920.0);
    }

    #[test]
    fn panel_wider_than_screen_left_aligns() {
        // 面板比屏幕宽：max_x 退化为 min_x，应左对齐屏幕原点而非产生越界负值。
        let (x, _) = calculate_panel_position(frame(0.0, 0.0, 600.0, 1080.0), 300.0, 720.0, 480.0);
        assert_eq!(x, 0.0);
    }
}
