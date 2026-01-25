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
use crate::services::SettingsService;
use crate::utils::accessibility::get_ax_selected_text;

// ============================================================================
// 入口和事件监听
// ============================================================================

#[cfg(target_os = "macos")]
pub fn setup_selection(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    init_menu_panel(app);
    init_content_panel(app);
    setup_mouce_observer(app.clone());
    setup_keyboard_monitor(app.clone());

    Ok(())
}

/// Mouse 监听和处理模块
#[cfg(target_os = "macos")]
fn setup_mouce_observer(app_handle: AppHandle) {
    let mut mouse = Mouse::new();
    let handle_clone = app_handle.clone();

    // 在独立线程中运行，因为 hook 是阻塞的
    std::thread::spawn(move || {
        // 使用 mouce 监听全局事件
        let _ = mouse.hook(Box::new(move |event| {
            match event {
                // 1. 滚动事件：直接触发隐藏
                mouce::common::MouseEvent::Scroll(_, _) => {
                    hide_menu_panel(&handle_clone);
                }
                // 2. 左键点击：如果是按下（Press），通常也需要隐藏
                //    但如果菜单面板或内容面板正在显示，不隐藏（让用户可以在面板上操作）
                mouce::common::MouseEvent::Press(mouce::common::MouseButton::Left) => {
                    record_mouse_press();

                    if !is_menu_panel_visible() && !is_content_panel_visible() {
                        hide_content_panel(&handle_clone);
                    }
                }
                // 3. 左键松开：这是你划词逻辑的触发点
                mouce::common::MouseEvent::Release(mouce::common::MouseButton::Left) => {
                    // 如果内容面板正在显示
                    if is_content_panel_visible() {
                        // 如果置顶，完全不处理（用户只能通过关闭按钮关闭）
                        if is_content_panel_pinned() {
                            return;
                        }
                        // 如果鼠标在面板内，不隐藏（允许用户在面板上选择文字）
                        if is_mouse_inside_content_panel() {
                            return;
                        }
                        // 非置顶且鼠标在面板外：延迟检查后隐藏
                        let h = handle_clone.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            // 如果面板仍可见、非置顶且鼠标不在面板内，则隐藏
                            if is_content_panel_visible() && !is_content_panel_pinned() && !is_mouse_inside_content_panel() {
                                tracing::info!("-----> hiding content panel (clicked outside)");
                                hide_content_panel(&h);
                            }
                        });
                        return;
                    }

                    // 如果菜单面板正在显示，延迟检查是否需要隐藏
                    if is_menu_panel_visible() {
                        let h: AppHandle = handle_clone.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            // 如果面板仍可见（说明用户点击的是面板外部），则隐藏并触发新的选词逻辑
                            if is_menu_panel_visible() {
                                tracing::info!("hiding menu panel (clicked outside)");
                                hide_menu_panel(&h);
                                // 只有拖动选择才触发新的选词逻辑
                                if should_trigger_selection() {
                                    tracing::info!("---------------------------------------------------------");
                                    trigger_selection_logic(&h);
                                }
                            }
                        });
                        return;
                    }

                    // 都不可见时，正常触发选择逻辑
                    if should_trigger_selection() {
                        tracing::info!("---------------------------------------------------------");
                        trigger_selection_logic(&handle_clone);
                    }
                }
                mouce::common::MouseEvent::RelativeMove(_x, _y) => {
                    // tracing::info!("======> x: {}, y: {}", x, y);
                }
                mouce::common::MouseEvent::AbsoluteMove(_x, _y) => {
                    // tracing::info!("-----> x: {}, y: {}", x, y);
                }
                _ => {}
            }
        })).expect("无法启动 mouce hook");
    });
}

// ============================================================================
// 鼠标按下状态（用于判断拖动选择和双击选择）
// ============================================================================

/// 本次鼠标按下的位置
static MOUSE_PRESS_X: AtomicU64 = AtomicU64::new(0);
static MOUSE_PRESS_Y: AtomicU64 = AtomicU64::new(0);
/// 本次鼠标按下的时间戳（毫秒）
static MOUSE_PRESS_TIME: AtomicU64 = AtomicU64::new(0);

/// 上一次鼠标按下的位置（用于双击检测）
static PREV_MOUSE_PRESS_X: AtomicU64 = AtomicU64::new(0);
static PREV_MOUSE_PRESS_Y: AtomicU64 = AtomicU64::new(0);
/// 上一次鼠标按下的时间戳（用于双击检测）
static PREV_MOUSE_PRESS_TIME: AtomicU64 = AtomicU64::new(0);

/// 判断为拖动选择的最小距离（像素）
const MIN_DRAG_DISTANCE: i32 = 5;
/// 双击的最大时间间隔（毫秒）
const DOUBLE_CLICK_MS: u64 = 500;

/// 记录鼠标按下状态（保存上一次状态，更新本次状态）
fn record_mouse_press() {
    // 先保存上一次的按下位置和时间
    PREV_MOUSE_PRESS_TIME.store(MOUSE_PRESS_TIME.load(Ordering::Relaxed), Ordering::Relaxed);
    PREV_MOUSE_PRESS_X.store(MOUSE_PRESS_X.load(Ordering::Relaxed), Ordering::Relaxed);
    PREV_MOUSE_PRESS_Y.store(MOUSE_PRESS_Y.load(Ordering::Relaxed), Ordering::Relaxed);

    // 记录本次鼠标按下位置
    let mouse = Mouse::new();
    if let Ok((x, y)) = mouse.get_position() {
        MOUSE_PRESS_X.store(x as u64, Ordering::Relaxed);
        MOUSE_PRESS_Y.store(y as u64, Ordering::Relaxed);
    }

    // 记录本次鼠标按下时间
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    MOUSE_PRESS_TIME.store(now, Ordering::Relaxed);
}

/// 检查当前鼠标位置与按下位置的距离是否超过阈值（判断是拖动选择还是单纯点击）
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

/// 检查是否为双击选择（两次点击间隔小于阈值且位置接近）
fn is_double_click_selection() -> bool {
    let current_time = MOUSE_PRESS_TIME.load(Ordering::Relaxed);
    let prev_time = PREV_MOUSE_PRESS_TIME.load(Ordering::Relaxed);

    // 检查两次按下的时间间隔是否在双击阈值内
    if current_time > prev_time && current_time - prev_time < DOUBLE_CLICK_MS {
        // 检查两次按下的位置是否接近
        let press_x = MOUSE_PRESS_X.load(Ordering::Relaxed) as i32;
        let press_y = MOUSE_PRESS_Y.load(Ordering::Relaxed) as i32;
        let prev_x = PREV_MOUSE_PRESS_X.load(Ordering::Relaxed) as i32;
        let prev_y = PREV_MOUSE_PRESS_Y.load(Ordering::Relaxed) as i32;
        let dx = (press_x - prev_x).abs();
        let dy = (press_y - prev_y).abs();
        // 双击时两次点击位置应该很接近
        return dx <= MIN_DRAG_DISTANCE && dy <= MIN_DRAG_DISTANCE;
    }
    false
}

/// 判断是否应该触发选择逻辑（拖动选择或双击选择）
fn should_trigger_selection() -> bool {
    is_drag_selection() || is_double_click_selection()
}

fn trigger_selection_logic(handle: &AppHandle) {
    // 检查功能是否启用
    if !is_selection_toolbar_enabled(handle) {
        return;
    }

    let mouse = Mouse::new();
    // 使用 mouce 获取当前位置，替代之前的 Swift 传参
    if let Ok((x, y)) = mouse.get_position() {
        let handle_clone: AppHandle = handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            match get_ax_selected_text() {
                Some(text) => {
                    tracing::info!("-----> text: {}, x: {}, y: {}", text, x, y);

                    let _ = handle_clone.emit(
                        "global-selection",
                        serde_json::json!({
                            "text": text,
                            "x": x,
                            "y": y,
                            "app_info": { "name": "1", "bundle_id": "2", "pid": 123 }
                        }),
                    );

                    show_menu_panel(&handle_clone.clone(), x as f64, y as f64);
                }
                _ => (),
            }
        });
    }
}

// ============================================================================
// Keyboard 监听和处理模块
// ============================================================================
fn setup_keyboard_monitor(handle: AppHandle<Wry>) {
    std::thread::spawn(move || {
        if let Ok(tap) = core_graphics::event::CGEventTap::new(
            core_graphics::event::CGEventTapLocation::HID,
            core_graphics::event::CGEventTapPlacement::HeadInsertEventTap,
            core_graphics::event::CGEventTapOptions::Default,
            vec![CGEventType::KeyDown],
            move |_, _, event| {
                // 如果 content panel 可见，不处理键盘事件（允许复制等操作）
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

// ============================================================================
// 检查选中文本工具栏功能是否启用
// ============================================================================
fn is_selection_toolbar_enabled(handle: &AppHandle) -> bool {
    let settings_service: tauri::State<'_, SettingsService> = handle.state();
    match settings_service.get_settings() {
        Ok(settings) => settings.quick_tools.show_toolbar_on_selection,
        Err(_) => false,
    }
}
