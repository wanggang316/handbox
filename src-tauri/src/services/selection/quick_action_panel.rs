//! Quick Action overlay window (macOS NSPanel): frameless, transparent,
//! always-on-top and non-activating, so it floats above full-screen apps without
//! switching Space. It centers on the upper third of the monitor under the
//! cursor and hides itself when it resigns key.
//!
//! The visibility flag is written before dispatching to the main thread so other
//! threads can observe the state lock-free.

// The panel_event! DSL requires an explicit `-> ()` (Obj-C void delegate), which
// trips clippy::unused_unit inside the macro expansion; only a module-level allow
// reaches macro-generated code.
#![allow(clippy::unused_unit)]

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::LogicalPosition;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::{
    tauri_panel, CollectionBehavior, PanelLevel, StyleMask, TrackingAreaOptions, WebviewWindowExt,
};

static QUICK_ACTION_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);

const PANEL_LABEL: &str = "quick_action";

/// Logical panel size; must stay in sync with the window in tauri.conf.json.
const PANEL_WIDTH: f64 = 720.0;
const PANEL_HEIGHT: f64 = 480.0;

tauri_panel! {
    panel!(QuickActionPanel {
        config: {
            can_become_key_window: true, // accepts keyboard input
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

    // A missing window or a failed NSPanel conversion only disables the overlay;
    // log and return so the rest of the app still starts.
    let Some(window) = app_handle.get_webview_window(PANEL_LABEL) else {
        tracing::error!(
            "quick action window '{PANEL_LABEL}' not found; overlay disabled (check tauri.conf.json)"
        );
        return;
    };
    // Native NSVisualEffectView vibrancy backdrop that follows the system
    // appearance; the radius matches the card corner so the material is clipped.
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
    // Float above full-screen apps without switching the active Space.
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .can_join_all_spaces()
            .full_screen_auxiliary()
            .value(),
    );
    // Nonactivating: showing the panel must not steal activation from the
    // frontmost app; combined with decorations:false + transparent:true it also
    // yields the frameless look.
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_corner_radius(14.0);

    let handler = QuickActionEventHandler::new();

    handler.window_did_become_key(move |_notification| {
        tracing::debug!("Quick action panel became key window");
    });

    // Auto-hide on focus loss: clear the flag first, then dispatch the hide.
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

/// Positions the panel on the monitor under the cursor and takes keyboard focus.
/// `cursor_phys_x` / `cursor_phys_y` are global **physical** pixel coordinates,
/// as returned by `AppHandle::cursor_position()`.
#[cfg(target_os = "macos")]
pub fn show_panel(handle: &AppHandle, cursor_phys_x: f64, cursor_phys_y: f64) {
    // Set before dispatching: the resign-key handler reads this to decide.
    QUICK_ACTION_PANEL_VISIBLE.store(true, Ordering::Relaxed);

    let handle_clone = handle.clone();
    let _ = handle.run_on_main_thread(move || {
        if let Some(window) = handle_clone.get_webview_window(PANEL_LABEL) {
            // Resolve the monitor in physical space, then stay in logical points:
            // `set_position` and `calculate_panel_position` both expect them. Only
            // the monitor rect matters — the panel centers on the display, not the cursor.
            let (frame, _cursor_logical_x) =
                resolve_cursor_monitor(&window, cursor_phys_x, cursor_phys_y);
            let (target_x, target_y) = calculate_panel_position(frame, PANEL_WIDTH, PANEL_HEIGHT);

            let _ = window.set_position(LogicalPosition::new(target_x, target_y));
            let _ = window.show();

            // Key focus is what makes the resign-key auto-hide fire at all.
            if let Ok(panel) = window.to_panel::<QuickActionPanel>() {
                panel.make_key_and_order_front();
            }

            // Tell the frontend to reset to a blank state — one summon is one
            // single-turn document. Emitted here because show_panel is the only
            // entry point; AppKit key notifications are unreliable for a
            // nonactivating panel.
            let _ = window.emit("quick-action-shown", ());
        }
    });
}

/// Moves the panel off-screen before hiding, so the next show cannot flash at
/// the stale position.
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

/// Toggles visibility; the physical cursor coordinates are only used when showing.
#[cfg(target_os = "macos")]
pub fn toggle(handle: &AppHandle, cursor_phys_x: f64, cursor_phys_y: f64) {
    if is_panel_visible() {
        hide_panel(handle);
    } else {
        show_panel(handle, cursor_phys_x, cursor_phys_y);
    }
}

pub fn is_panel_visible() -> bool {
    QUICK_ACTION_PANEL_VISIBLE.load(Ordering::Relaxed)
}

/// A monitor rect in logical coordinates (points, origin top-left).
#[cfg(any(target_os = "macos", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Monitor geometry in **physical** pixels plus its DPI scale; a runtime-free
/// input for `select_monitor` so the selection logic stays unit-testable.
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
    fn logical_frame(&self) -> MonitorFrame {
        MonitorFrame {
            x: self.pos_x / self.scale,
            y: self.pos_y / self.scale,
            width: self.width_phys / self.scale,
            height: self.height_phys / self.scale,
        }
    }

    fn contains_phys(&self, cursor_phys_x: f64, cursor_phys_y: f64) -> bool {
        cursor_phys_x >= self.pos_x
            && cursor_phys_x < self.pos_x + self.width_phys
            && cursor_phys_y >= self.pos_y
            && cursor_phys_y < self.pos_y + self.height_phys
    }
}

/// Picks the target monitor for the cursor and returns its **logical** rect plus
/// the cursor's **logical** x. Fallback order: the monitor containing the cursor,
/// then `primary_index` (a cursor outside every screen lands on the main display
/// rather than an arbitrary one), then the first monitor, then `None`.
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

/// Resolves the cursor's monitor into its **logical** rect and the cursor's
/// **logical** x. Matching happens in physical space because displays can have
/// different DPI; the matched monitor's scale then converts to points.
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

        // Index of the primary monitor, used when the cursor is off every screen.
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

    // No monitors at all: assume scale 1 and fake a panel-sized rect around the
    // cursor so positioning still yields a visible result.
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

/// Centers the panel horizontally on the **display** (independent of the cursor
/// x) with its top edge at ~20% of the display height, clamping both axes so the
/// panel stays fully on screen. A panel wider than the screen left-aligns to the
/// display origin instead of going out of bounds.
#[cfg(any(target_os = "macos", test))]
fn calculate_panel_position(
    frame: MonitorFrame,
    panel_width: f64,
    panel_height: f64,
) -> (f64, f64) {
    let mut target_x = frame.x + (frame.width - panel_width) / 2.0;
    let min_x = frame.x;
    let max_x = (frame.x + frame.width - panel_width).max(min_x);
    target_x = target_x.clamp(min_x, max_x);

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
        let (x, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, (1920.0 - 720.0) / 2.0); // 600.0 — display center
        assert_eq!(y, 1080.0 * 0.2); // 216.0 — upper third, not mid-screen
    }

    #[test]
    fn horizontal_center_independent_of_cursor() {
        // The horizontal position is always the display center, never the cursor x.
        let f = frame(0.0, 0.0, 1920.0, 1080.0);
        let (x_left, _) = calculate_panel_position(f, 720.0, 480.0);
        let (x_right, _) = calculate_panel_position(f, 720.0, 480.0);
        assert_eq!(x_left, 600.0);
        assert_eq!(x_right, 600.0);
        assert_eq!(x_left + 720.0 / 2.0, 0.0 + 1920.0 / 2.0);
    }

    #[test]
    fn clamps_when_display_center_would_overflow() {
        // Panel nearly as wide as the screen: clamping must not move a valid center.
        let (x, _) = calculate_panel_position(frame(0.0, 0.0, 800.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, (800.0 - 720.0) / 2.0); // 40.0, fully visible
        assert!(x >= 0.0);
        assert!(x + 720.0 <= 800.0);
    }

    #[test]
    fn stays_fully_on_screen_vertically() {
        let (_, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 600.0), 720.0, 480.0);
        // 0.2 * 600 = 120, 120 + 480 = 600 → exactly fits, no clamping.
        assert_eq!(y, 120.0);
    }

    #[test]
    fn clamps_vertically_on_short_display() {
        let (_, y) = calculate_panel_position(frame(0.0, 0.0, 1920.0, 500.0), 720.0, 480.0);
        // 0.2 * 500 = 100, 100 + 480 = 580 > 500 → clamped to 500 - 480 = 20
        assert_eq!(y, 20.0);
    }

    #[test]
    fn honors_monitor_origin_offset() {
        let (x, y) = calculate_panel_position(frame(1920.0, 0.0, 1920.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, 1920.0 + (1920.0 - 720.0) / 2.0); // 1920 + 600
        assert_eq!(y, 1080.0 * 0.2); // 216.0
    }

    #[test]
    fn negative_origin_monitor_centers_within_bounds() {
        let (x, _) = calculate_panel_position(frame(-1920.0, 0.0, 1920.0, 1080.0), 720.0, 480.0);
        assert_eq!(x, -1920.0 + (1920.0 - 720.0) / 2.0); // -1920 + 600 = -1320
        assert!(x >= -1920.0);
        assert!(x + 720.0 <= -1920.0 + 1920.0);
    }

    #[test]
    fn panel_wider_than_screen_left_aligns() {
        // max_x degrades to min_x, so the panel left-aligns instead of going negative.
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
        let monitors = [
            monitor(0.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(1920.0, 0.0, 1920.0, 1080.0, 1.0),
        ];
        let (frame, _) = select_monitor(&monitors, Some(1), -5000.0, -5000.0).expect("a monitor");
        assert_eq!(frame.x, 1920.0);
    }

    #[test]
    fn select_monitor_falls_back_to_first_when_no_primary() {
        let monitors = [
            monitor(0.0, 0.0, 1920.0, 1080.0, 1.0),
            monitor(1920.0, 0.0, 1920.0, 1080.0, 1.0),
        ];
        let (frame, _) = select_monitor(&monitors, None, -5000.0, -5000.0).expect("a monitor");
        assert_eq!(frame.x, 0.0);
    }

    #[test]
    fn select_monitor_returns_none_when_empty() {
        assert!(select_monitor(&[], None, 0.0, 0.0).is_none());
    }

    #[test]
    fn select_monitor_converts_to_logical_with_scale() {
        let monitors = [monitor(0.0, 0.0, 3840.0, 2160.0, 2.0)];
        let (frame, cursor_logical_x) =
            select_monitor(&monitors, Some(0), 1920.0, 1080.0).expect("a monitor");
        assert_eq!(frame.width, 1920.0);
        assert_eq!(frame.height, 1080.0);
        assert_eq!(cursor_logical_x, 960.0);
    }
}
