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
use tauri::{AppHandle, Emitter, Manager};
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

    // 容错初始化：若 tauri.conf.json 与运行时漂移导致 quick_action 窗口缺失或
    // NSPanel 转换失败，记录并提前返回，让应用正常启动（仅浮层不可用），而不是
    // panic 整个 app（与 lib.rs / setup_selection 的 log-and-continue 一致）。
    let Some(window) = app_handle.get_webview_window(PANEL_LABEL) else {
        tracing::error!(
            "quick action window '{PANEL_LABEL}' not found; overlay disabled (check tauri.conf.json)"
        );
        return;
    };
    // Raycast 式磨砂背景：原生 NSVisualEffectView（vibrancy），随系统外观自适应。
    // 半透明卡片叠加其上即得到 frosted-glass 观感；radius 与卡片圆角一致以裁剪材质。
    let _ = window.set_effects(
        tauri::window::EffectsBuilder::new()
            .effect(tauri::window::Effect::Popover)
            .state(tauri::window::EffectState::Active)
            .radius(14.0)
            .build(),
    );

    let panel = match window.to_panel::<QuickActionPanel>() {
        Ok(panel) => panel,
        Err(e) => {
            tracing::error!(
                "failed to convert quick action window to NSPanel: {e}; overlay disabled"
            );
            return;
        }
    };
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
    panel.set_corner_radius(14.0);

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
            // 在逻辑坐标系。面板居中于显示器（与鼠标 x 无关），故这里只取显示器矩形。
            let (frame, _cursor_logical_x) =
                resolve_cursor_monitor(&window, cursor_phys_x, cursor_phys_y);
            let (target_x, target_y) = calculate_panel_position(frame, PANEL_WIDTH, PANEL_HEIGHT);

            let _ = window.set_position(LogicalPosition::new(target_x, target_y));
            let _ = window.show();

            // 取得键盘焦点，使 resign-key 自动隐藏成立 (VAL-OVERLAY-004)。
            if let Ok(panel) = window.to_panel::<QuickActionPanel>() {
                panel.make_key_and_order_front();
            }

            // 通知前端：本次召唤把浮层重置为全新空白状态（一次召唤 = 一个一回合文档）。
            // show_panel 是所有召唤的必经路径，比 nonactivating panel 不可靠的 AppKit
            // key 通知更稳妥。
            let _ = window.emit("quick-action-shown", ());
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

/// 一块显示器在**物理坐标系**下的几何（origin/size 为物理像素）加上 DPI scale。
/// 作为 `select_monitor` 的纯输入，便于在不依赖 tauri 运行时的情况下做单元测试。
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Clone, Copy)]
struct MonitorInfo {
    pos_x: f64,
    pos_y: f64,
    width_phys: f64,
    height_phys: f64,
    scale: f64,
}

#[cfg(any(target_os = "macos", test))]
impl MonitorInfo {
    /// 用该屏 scale 把物理矩形换算到逻辑坐标系（点）。
    fn logical_frame(&self) -> MonitorFrame {
        MonitorFrame {
            x: self.pos_x / self.scale,
            y: self.pos_y / self.scale,
            width: self.width_phys / self.scale,
            height: self.height_phys / self.scale,
        }
    }

    /// 在物理空间判断鼠标是否落在此屏内。
    fn contains_phys(&self, cursor_phys_x: f64, cursor_phys_y: f64) -> bool {
        cursor_phys_x >= self.pos_x
            && cursor_phys_x < self.pos_x + self.width_phys
            && cursor_phys_y >= self.pos_y
            && cursor_phys_y < self.pos_y + self.height_phys
    }
}

/// 纯函数：在已知显示器集合中为鼠标挑选目标显示器，返回其**逻辑**矩形与鼠标的
/// **逻辑** x。回退顺序（least-surprise）：
/// 1. 包含鼠标的显示器；
/// 2. 主显示器（`primary_index`）——鼠标落在所有屏之外时，浮层落在主屏而非任意一块；
/// 3. 第一块可用显示器；
/// 4. `None`（无任何显示器，由调用方给出虚拟矩形）。
#[cfg(any(target_os = "macos", test))]
fn select_monitor(
    monitors: &[MonitorInfo],
    primary_index: Option<usize>,
    cursor_phys_x: f64,
    cursor_phys_y: f64,
) -> Option<(MonitorFrame, f64)> {
    let to_result = |m: &MonitorInfo| (m.logical_frame(), cursor_phys_x / m.scale);

    if let Some(m) = monitors
        .iter()
        .find(|m| m.contains_phys(cursor_phys_x, cursor_phys_y))
    {
        return Some(to_result(m));
    }
    if let Some(m) = primary_index.and_then(|i| monitors.get(i)) {
        return Some(to_result(m));
    }
    monitors.first().map(to_result)
}

/// 在物理坐标系下找到包含鼠标的显示器，返回其**逻辑**矩形与鼠标的**逻辑** x。
/// 多显示器在物理空间匹配（各屏 DPI 可能不同），匹配后用该屏 scale 统一换算到
/// 逻辑坐标系。鼠标落在所有屏之外时优先回退到主显示器，再退到第一块可用显示器，
/// 最后退回到以鼠标为中心、面板大小的一块虚拟矩形（保证 `calculate_panel_position`
/// 仍能给出可见结果）。
#[cfg(target_os = "macos")]
fn resolve_cursor_monitor(
    window: &tauri::WebviewWindow,
    cursor_phys_x: f64,
    cursor_phys_y: f64,
) -> (MonitorFrame, f64) {
    if let Ok(monitors) = window.available_monitors() {
        let infos: Vec<MonitorInfo> = monitors
            .iter()
            .map(|monitor| MonitorInfo {
                pos_x: monitor.position().x as f64,
                pos_y: monitor.position().y as f64,
                width_phys: monitor.size().width as f64,
                height_phys: monitor.size().height as f64,
                scale: monitor.scale_factor(),
            })
            .collect();

        // 主显示器在 available_monitors 中的下标（用于鼠标落空时的回退）。
        let primary_index = window.primary_monitor().ok().flatten().and_then(|primary| {
            let p = primary.position();
            infos
                .iter()
                .position(|m| m.pos_x == p.x as f64 && m.pos_y == p.y as f64)
        });

        if let Some(result) = select_monitor(&infos, primary_index, cursor_phys_x, cursor_phys_y) {
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

/// 纯函数：在给定显示器矩形内，把面板水平居中于**显示器**、纵向置于上三分之一处，
/// 并夹紧到屏幕范围内使其完整可见。
///
/// - 水平：居中于鼠标所在**显示器**（VAL-OVERLAY-008，与鼠标 x 无关），随后夹紧到
///   `[x, x+width-panel]` 使其不越界（VAL-OVERLAY-009）；当面板比屏幕宽时退化为左
///   对齐屏幕原点。
/// - 纵向：目标顶边设在显示器高度约 20% 处（“上三分之一”，VAL-OVERLAY-001），同样
///   夹紧到屏内。
#[cfg(any(target_os = "macos", test))]
fn calculate_panel_position(
    frame: MonitorFrame,
    panel_width: f64,
    panel_height: f64,
) -> (f64, f64) {
    // 水平居中于显示器（而非鼠标），随后整体夹紧进屏幕。
    let mut target_x = frame.x + (frame.width - panel_width) / 2.0;
    let min_x = frame.x;
    let max_x = (frame.x + frame.width - panel_width).max(min_x);
    target_x = target_x.clamp(min_x, max_x);

    // 纵向：上三分之一。顶边设在屏幕高度约 20% 处。
    let mut target_y = frame.y + frame.height * 0.2;
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
    fn centers_horizontally_on_display_in_upper_third() {
        // 1920x1080 主屏：面板水平居中于显示器，顶边在屏高约 20% 处（上三分之一）。
        let (x, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, (1920.0 - 720.0) / 2.0); // 600.0 — display center
        assert_eq!(y, 1080.0 * 0.2); // 216.0 — upper third, not mid-screen
    }

    #[test]
    fn horizontal_center_independent_of_cursor() {
        // 修复回归点：水平位置与鼠标 x 无关，永远是显示器中心。
        // （bug 复现：鼠标 x=629 时旧逻辑把面板中心放到 629；x=1600 时夹紧到 1200。
        //  现在两者都应给出 (1920 - 720) / 2 = 600。）
        let f = frame(0.0, 0.0, 1920.0, 1080.0);
        let (x_left, _) = calculate_panel_position(f, 720.0, 480.0);
        let (x_right, _) = calculate_panel_position(f, 720.0, 480.0);
        assert_eq!(x_left, 600.0);
        assert_eq!(x_right, 600.0);
        // 面板水平中心 == 显示器水平中心。
        assert_eq!(x_left + 720.0 / 2.0, 0.0 + 1920.0 / 2.0);
    }

    #[test]
    fn clamps_when_display_center_would_overflow() {
        // 面板比屏幕窄但接近：居中后两侧仍应留在屏内（夹紧不改变已合法的居中值）。
        let (x, _) = calculate_panel_position(frame(0.0, 0.0, 800.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, (800.0 - 720.0) / 2.0); // 40.0，完整可见
        assert!(x >= 0.0);
        assert!(x + 720.0 <= 800.0);
    }

    #[test]
    fn stays_fully_on_screen_vertically() {
        // 矮屏：20% 处加面板高度会越过底界，纵向应夹紧到 (height - panel)。
        let (_, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 600.0), 720.0, 480.0);
        // 0.2 * 600 = 120, 120 + 480 = 600 == 600 → 仍恰好可见，不夹紧。
        assert_eq!(y, 120.0);
    }

    #[test]
    fn clamps_vertically_on_short_display() {
        // 更矮的屏：20% 顶边加面板高度越界，纵向夹紧到 (height - panel)。
        let (_, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 500.0), 720.0, 480.0);
        // 0.2 * 500 = 100, 100 + 480 = 580 > 500 → 夹紧到 500 - 480 = 20
        assert_eq!(y, 20.0);
    }

    #[test]
    fn honors_monitor_origin_offset() {
        // 第二块显示器原点在 (1920, 0)：定位应相对该显示器原点居中。
        let (x, y) = calculate_panel_position(frame(1920.0, 0.0, 1920.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, 1920.0 + (1920.0 - 720.0) / 2.0); // 1920 + 600
        assert_eq!(y, 1080.0 * 0.2); // 216.0
    }

    #[test]
    fn negative_origin_monitor_centers_within_bounds() {
        // 左侧扩展屏原点为负：居中应相对该屏原点（负坐标），并完整落在屏内。
        let (x, _) = calculate_panel_position(frame(-1920.0, 0.0, 1920.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, -1920.0 + (1920.0 - 720.0) / 2.0); // -1920 + 600 = -1320
        assert!(x >= -1920.0);
        assert!(x + 720.0 <= -1920.0 + 1920.0);
    }

    #[test]
    fn panel_wider_than_screen_left_aligns() {
        // 面板比屏幕宽：max_x 退化为 min_x，应左对齐屏幕原点而非产生越界负值。
        let (x, _) = calculate_panel_position(frame(0.0, 0.0, 600.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, 0.0);
    }

    fn monitor(pos_x: f64, pos_y: f64, w: f64, h: f64, scale: f64) -> MonitorInfo {
        MonitorInfo {
            pos_x,
            pos_y,
            width_phys: w,
            height_phys: h,
            scale,
        }
    }

    #[test]
    fn select_monitor_prefers_monitor_under_cursor() {
        // 鼠标落在第二块屏内：忽略 primary，选包含鼠标的那块。
        let monitors = [
            monitor(0.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(1920.0, 0.0, 1920.0, 1080.0, 1.0),
        ];
        let (frame, cursor_logical_x) =
            select_monitor(&monitors, Some(0), 2880.0, 540.0).expect("a monitor");
        assert_eq!(frame.x, 1920.0);
        assert_eq!(cursor_logical_x, 2880.0);
    }

    #[test]
    fn select_monitor_falls_back_to_primary_when_cursor_outside_all() {
        // 鼠标在所有屏之外：回退到 primary（下标 1），而非第一块。
        let monitors = [
            monitor(0.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(1920.0, 0.0, 1920.0, 1080.0, 1.0),
        ];
        let (frame, _) = select_monitor(&monitors, Some(1), -5000.0, -5000.0).expect("a monitor");
        assert_eq!(frame.x, 1920.0);
    }

    #[test]
    fn select_monitor_falls_back_to_first_when_no_primary() {
        // 鼠标在所有屏之外且无 primary 信息：回退到第一块可用显示器。
        let monitors = [
            monitor(0.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(1920.0, 0.0, 1920.0, 1080.0, 1.0),
        ];
        let (frame, _) = select_monitor(&monitors, None, -5000.0, -5000.0).expect("a monitor");
        assert_eq!(frame.x, 0.0);
    }

    #[test]
    fn select_monitor_returns_none_when_empty() {
        // 无任何显示器：返回 None，由调用方给出虚拟矩形。
        assert!(select_monitor(&[], None, 0.0, 0.0).is_none());
    }

    #[test]
    fn select_monitor_converts_to_logical_with_scale() {
        // Retina 屏（scale=2）：逻辑矩形与逻辑鼠标 x 应除以 scale。
        let monitors = [monitor(0.0, 0.0, 3840.0, 2160.0, 2.0)];
        let (frame, cursor_logical_x) =
            select_monitor(&monitors, Some(0), 1920.0, 1080.0).expect("a monitor");
        assert_eq!(frame.width, 1920.0);
        assert_eq!(frame.height, 1080.0);
        assert_eq!(cursor_logical_x, 960.0);
    }
}
