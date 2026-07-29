use core_graphics::event::{CGEventType, EventField};
use mouce::{Mouse, MouseActions};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::services::selection::content_panel::hide_panel as hide_content_panel;
use crate::services::selection::content_panel::init_panel as init_content_panel;
use crate::services::selection::content_panel::is_mouse_inside as is_mouse_inside_content_panel;
use crate::services::selection::content_panel::is_panel_pinned as is_content_panel_pinned;
use crate::services::selection::content_panel::is_panel_visible as is_content_panel_visible;
use crate::services::selection::menu_panel::hide_panel as hide_menu_panel;
use crate::services::selection::menu_panel::init_panel as init_menu_panel;
use crate::services::selection::menu_panel::is_panel_visible as is_menu_panel_visible;
use crate::services::selection::menu_panel::show_panel as show_menu_panel;
use crate::services::selection::settings_panel::init_panel as init_settings_panel;
use crate::services::selection::settings_panel::is_panel_visible as is_settings_panel_visible;
use crate::services::selection::settings_panel::hide_panel as hide_settings_panel;
use crate::services::selection::settings_panel::is_mouse_inside as is_mouse_inside_settings_panel;
use crate::services::SettingsService;
use crate::utils::accessibility::get_ax_selected_text;
use crate::utils::{get_frontmost_app_info, FrontmostAppInfo};

#[cfg(target_os = "macos")]
pub fn setup_selection(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    init_menu_panel(app);
    init_content_panel(app);
    init_settings_panel(app);
    setup_mouce_observer(app.clone());
    setup_keyboard_monitor(app.clone());

    Ok(())
}

#[cfg(target_os = "macos")]
fn setup_mouce_observer(app_handle: AppHandle) {
    let mut mouse = Mouse::new();
    let handle_clone = app_handle.clone();

    // Run on a dedicated thread; the hook blocks.
    std::thread::spawn(move || {
        let _ = mouse.hook(Box::new(move |event| {
            match event {
                mouce::common::MouseEvent::Scroll(_, _) => {
                    hide_menu_panel(&handle_clone);
                }
                // On left press hide the content panel, unless a panel is showing
                // (let the user interact with it).
                mouce::common::MouseEvent::Press(mouce::common::MouseButton::Left) => {
                    record_mouse_press();

                    if !is_menu_panel_visible() && !is_content_panel_visible() {
                        hide_content_panel(&handle_clone);
                    }
                }
                // Left release drives the selection flow.
                mouce::common::MouseEvent::Release(mouce::common::MouseButton::Left) => {
                    // Settings panel first: keep it open while the mouse is inside it.
                    if is_settings_panel_visible() {
                        if is_mouse_inside_settings_panel() {
                            return;
                        }
                        hide_settings_panel(&handle_clone);
                    }
                    if is_content_panel_visible() {
                        // Pinned: only the close button dismisses the panel.
                        if is_content_panel_pinned() {
                            return;
                        }
                        // Mouse inside the panel: let the user select text there.
                        if is_mouse_inside_content_panel() {
                            return;
                        }
                        // Clicked outside: hide after a short grace period.
                        let h = handle_clone.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if is_content_panel_visible() && !is_content_panel_pinned() && !is_mouse_inside_content_panel() {
                                tracing::info!("-----> hiding content panel (clicked outside)");
                                hide_content_panel(&h);
                            }
                        });
                        return;
                    }

                    if is_menu_panel_visible() {
                        // Delay the check so a button's onclick can run first; it
                        // calls hide_menu_panel itself.
                        let h: AppHandle = handle_clone.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if is_menu_panel_visible() && !is_settings_panel_visible() {
                                tracing::info!("hiding menu panel (clicked outside)");
                                hide_menu_panel(&h);
                                // Only a fresh drag or double-click starts a new selection.
                                if should_trigger_selection() {
                                    tracing::info!("---------------------------------------------------------");
                                    trigger_selection_logic(&h);
                                }
                            }
                        });
                        return;
                    }

                    if should_trigger_selection() {
                        tracing::info!("---------------------------------------------------------");
                        trigger_selection_logic(&handle_clone);
                    }
                }
                mouce::common::MouseEvent::RelativeMove(_x, _y) => {
                }
                mouce::common::MouseEvent::AbsoluteMove(_x, _y) => {
                }
                _ => {}
            }
        })).expect("无法启动 mouce hook");
    });
}

/// Position of the current mouse press.
static MOUSE_PRESS_X: AtomicU64 = AtomicU64::new(0);
static MOUSE_PRESS_Y: AtomicU64 = AtomicU64::new(0);
/// Timestamp of the current mouse press, in milliseconds.
static MOUSE_PRESS_TIME: AtomicU64 = AtomicU64::new(0);

/// Position of the previous mouse press, used for double-click detection.
static PREV_MOUSE_PRESS_X: AtomicU64 = AtomicU64::new(0);
static PREV_MOUSE_PRESS_Y: AtomicU64 = AtomicU64::new(0);
/// Timestamp of the previous mouse press, used for double-click detection.
static PREV_MOUSE_PRESS_TIME: AtomicU64 = AtomicU64::new(0);

/// Minimum travel, in pixels, that counts as a drag selection.
const MIN_DRAG_DISTANCE: i32 = 5;
/// Maximum gap, in milliseconds, between two presses of a double click.
const DOUBLE_CLICK_MS: u64 = 500;

/// Records the current press, rolling the previous one forward.
fn record_mouse_press() {
    PREV_MOUSE_PRESS_TIME.store(MOUSE_PRESS_TIME.load(Ordering::Relaxed), Ordering::Relaxed);
    PREV_MOUSE_PRESS_X.store(MOUSE_PRESS_X.load(Ordering::Relaxed), Ordering::Relaxed);
    PREV_MOUSE_PRESS_Y.store(MOUSE_PRESS_Y.load(Ordering::Relaxed), Ordering::Relaxed);

    let mouse = Mouse::new();
    if let Ok((x, y)) = mouse.get_position() {
        MOUSE_PRESS_X.store(x as u64, Ordering::Relaxed);
        MOUSE_PRESS_Y.store(y as u64, Ordering::Relaxed);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    MOUSE_PRESS_TIME.store(now, Ordering::Relaxed);
}

/// True when the cursor travelled far enough from the press point to count as a
/// drag rather than a plain click.
fn is_drag_selection() -> bool {
    let mouse = Mouse::new();
    if let Ok((x, y)) = mouse.get_position() {
        let press_x = MOUSE_PRESS_X.load(Ordering::Relaxed) as i32;
        let press_y = MOUSE_PRESS_Y.load(Ordering::Relaxed) as i32;
        let dx = (x - press_x).abs();
        let dy = (y - press_y).abs();
        return dx > MIN_DRAG_DISTANCE || dy > MIN_DRAG_DISTANCE;
    }
    false
}

/// True when the last two presses were close enough in time and space to be a
/// double click.
fn is_double_click_selection() -> bool {
    let current_time = MOUSE_PRESS_TIME.load(Ordering::Relaxed);
    let prev_time = PREV_MOUSE_PRESS_TIME.load(Ordering::Relaxed);

    if current_time > prev_time && current_time - prev_time < DOUBLE_CLICK_MS {
        let press_x = MOUSE_PRESS_X.load(Ordering::Relaxed) as i32;
        let press_y = MOUSE_PRESS_Y.load(Ordering::Relaxed) as i32;
        let prev_x = PREV_MOUSE_PRESS_X.load(Ordering::Relaxed) as i32;
        let prev_y = PREV_MOUSE_PRESS_Y.load(Ordering::Relaxed) as i32;
        let dx = (press_x - prev_x).abs();
        let dy = (press_y - prev_y).abs();
        return dx <= MIN_DRAG_DISTANCE && dy <= MIN_DRAG_DISTANCE;
    }
    false
}

fn should_trigger_selection() -> bool {
    is_drag_selection() || is_double_click_selection()
}

fn trigger_selection_logic(handle: &AppHandle) {
    if !is_selection_toolbar_enabled(handle) {
        return;
    }

    let mouse = Mouse::new();
    if let Ok((x, y)) = mouse.get_position() {
        let handle_clone: AppHandle = handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            if let Some(text) = get_ax_selected_text() {
                tracing::info!("-----> text: {}, x: {}, y: {}", text, x, y);
                let app_info = get_frontmost_app_info();
                tracing::info!("-----> frontmost app info: {:?}", app_info);
                if let Some(ref info) = app_info {
                    if is_selection_blacklisted(&handle_clone, info) {
                        tracing::info!("-----> selection blocked by blacklist");
                        return;
                    }
                }
                let app_info_payload = app_info.map(|info| {
                    serde_json::json!({
                        "name": info.name,
                        "bundle_id": info.bundle_id,
                        "pid": info.pid,
                    })
                });

                let _ = handle_clone.emit(
                    "global-selection",
                    serde_json::json!({
                        "text": text,
                        "x": x,
                        "y": y,
                        "app_info": app_info_payload.unwrap_or_else(|| {
                            serde_json::json!({ "name": "Unknown", "bundle_id": "unknown.app", "pid": 0 })
                        })
                    }),
                );

                show_menu_panel(&handle_clone.clone(), x as f64, y as f64);
            }
        });
    }
}

fn setup_keyboard_monitor(handle: AppHandle<Wry>) {
    std::thread::spawn(move || {
        if let Ok(tap) = core_graphics::event::CGEventTap::new(
            core_graphics::event::CGEventTapLocation::HID,
            core_graphics::event::CGEventTapPlacement::HeadInsertEventTap,
            core_graphics::event::CGEventTapOptions::Default,
            vec![CGEventType::KeyDown],
            move |_, _, event| {
                // Leave keys alone while the content panel is up, so copy works there.
                if is_content_panel_visible() {
                    return None;
                }
                let key_code = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
                tracing::info!("-----> key_code: {}", key_code);
                hide_menu_panel(&handle);
                None
            },
        ) {
            unsafe {
                let loop_source = tap.mach_port.create_runloop_source(0).expect("RunLoop Err");
                let current_loop = core_foundation::runloop::CFRunLoopGetCurrent();

                let source_ptr: *mut std::ffi::c_void = std::mem::transmute(loop_source);

                core_foundation::runloop::CFRunLoopAddSource(
                    current_loop,
                    source_ptr as *mut _,
                    core_foundation::runloop::kCFRunLoopCommonModes,
                );

                tap.enable();
                core_foundation::runloop::CFRunLoopRun();
            }
        }
    });
}

fn is_selection_toolbar_enabled(handle: &AppHandle) -> bool {
    let settings_service: tauri::State<'_, SettingsService> = handle.state();
    match settings_service.get_settings() {
        Ok(settings) => settings.quick_tools.show_toolbar_on_selection,
        Err(_) => false,
    }
}

fn is_selection_blacklisted(handle: &AppHandle, app_info: &FrontmostAppInfo) -> bool {
    let settings_service: tauri::State<'_, SettingsService> = handle.state();
    match settings_service.get_settings() {
        Ok(settings) => {
            let blacklist = settings.quick_tools.selection_blacklist;
            if blacklist.pids.contains(&app_info.pid) {
                return true;
            }
            blacklist
                .apps
                .iter()
                .any(|app| app.bundle_id == app_info.bundle_id)
        }
        Err(_) => false,
    }
}
