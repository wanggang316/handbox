// macOS system selection observer for external text actions.

use accessibility::{AXAttribute, AXUIElement};
use core_foundation::base::TCFType;
#[cfg(target_os = "macos")]
use core_foundation::base::{CFRange, CFRelease, CFTypeRef};
#[cfg(target_os = "macos")]
use core_foundation::boolean::CFBoolean;
#[cfg(target_os = "macos")]
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
#[cfg(target_os = "macos")]
use core_foundation::string::CFStringRef;
#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
use core_graphics::display::CGDisplay;
#[cfg(target_os = "macos")]
use core_graphics::event::CGEvent;
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
#[cfg(target_os = "macos")]
use core_graphics::geometry::CGRect;
#[cfg(target_os = "macos")]
use objc2::rc::autoreleasepool;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSEvent, NSWorkspace};
#[cfg(target_os = "macos")]
use once_cell::sync::Lazy;
use rdev::{listen, Event, EventType, Key};
use serde::Serialize;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
#[cfg(target_os = "macos")]
use std::sync::Mutex;
use std::thread;
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Emitter};

const MENU_WIDTH: f64 = 360.0;
const OVERLAY_VERTICAL_GAP: f64 = 24.0;
const SELECTION_HOVER_PADDING: f64 = 6.0;
const MOUSE_STILL_THRESHOLD: f64 = 2.0;
const POLL_INTERVAL_MS: u64 = 100;
const HOVER_DELAY_MS: u64 = 300;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SelectionRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionPayload {
    text: String,
    raw_text: String,
    rect: Option<SelectionRect>,
    source_app_name: Option<String>,
    source_bundle_id: Option<String>,
    source_pid: Option<i64>,
    source_app_path: Option<String>,
    source_app_version: Option<String>,
    source_window_title: Option<String>,
    source_url: Option<String>,
    source_domain: Option<String>,
    source_tab_title: Option<String>,
    capture_method: Option<String>,
    locale: Option<String>,
    input_language: Option<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct SelectionSnapshot {
    payload: SelectionPayload,
    signature: String,
}

#[cfg(target_os = "macos")]
static LAST_SELECTION_PAYLOAD: Lazy<Mutex<Option<SelectionPayload>>> =
    Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "macos")]
static LAST_SELECTION_ANCHOR: Lazy<Mutex<Option<SelectionRect>>> = Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "macos")]
static LAST_SELECTION_SIGNATURE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "macos")]
static LAST_DISMISSED_SIGNATURE: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "macos")]
static OVERLAY_LOCKED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
pub fn set_overlay_locked(locked: bool) {
    OVERLAY_LOCKED.store(locked, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
pub fn dismiss_current_selection_signature() {
    let signature = LAST_SELECTION_SIGNATURE
        .lock()
        .ok()
        .and_then(|value| value.clone());
    if let Some(signature) = signature {
        if let Ok(mut slot) = LAST_DISMISSED_SIGNATURE.lock() {
            *slot = Some(signature);
        }
    }
}

pub fn get_ax_selected_text() -> Option<String> {
    let system_wide = AXUIElement::system_wide();

    // 1. 创建 CFString 对象，注意它不是普通的 &str
    let focused_key = CFString::from_static_string("AXFocusedUIElement");
    // 2. 传入 CFString 的引用
    let focused_attr = AXAttribute::new(&focused_key);

    let focused_cf = system_wide.attribute(&focused_attr).ok()?;
    let focused_element = focused_cf.downcast_into::<AXUIElement>()?;

    // 3. 同样的方式获取选中文字
    let selected_key = CFString::from_static_string("AXSelectedText");
    let selected_attr = AXAttribute::new(&selected_key);

    let text_cf_type = focused_element.attribute(&selected_attr).ok()?;
    let text_cf = text_cf_type.downcast_into::<CFString>()?;

    let text = text_cf.to_string().trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(target_os = "macos")]
pub fn start_selection_observer(app: AppHandle) {
    let (tx, rx) = std::sync::mpsc::channel::<(f64, f64)>();

    // 逻辑处理线程
    thread::spawn(move || {
        while let Ok((x, y)) = rx.recv() {
            // 给系统 UI 更新留出 200ms
            thread::sleep(std::time::Duration::from_millis(200));
            if let Some(text) = get_ax_selected_text() {
                // let _ = app_handle.emit("global-selection", (text, x, y));
                tracing::info!("-----> text: {}, x: {}, y: {}", text, x, y);
            }
        }
    });

    // 核心修复：只监听鼠标，彻底屏蔽键盘事件的下发
    thread::spawn(move || {
        let mut last_pos = (0.0, 0.0);

        // 建议在外部加上 catch_unwind，防止 rdev 内部因无法解析某些 macOS 按键而 Panic
        let _ = std::panic::catch_unwind(move || {
            let _ = listen(move |event: Event| {
                match event.event_type {
                    // 仅记录坐标
                    EventType::MouseMove { x, y } => {
                        last_pos = (x, y);
                    }
                    // 仅处理鼠标松开
                    EventType::ButtonRelease(rdev::Button::Left) => {
                        let _ = tx.send(last_pos);
                    }
                    // 对于任何键盘、功能键事件，直接退出，不进行任何解析
                    _ => return,
                }
            });
        });
    });

    // let (tx, rx) = mpsc::channel::<(f64, f64)>();

    // // 逻辑处理线程 (Worker)
    // thread::spawn(move || {
    //     while let Ok((x, y)) = rx.recv() {
    //         tracing::info!("----->>>>>> x: {}, y: {}", x, y);
    //         // 重点 1: 增加更长的延迟 (300ms+)，避开 Cmd+Tab 切换时的系统繁忙期
    //         thread::sleep(Duration::from_millis(2850));

    //         // 重点 2: 获取文字时增加安全检查，绝不使用 unwrap
    //         if let Some(text) = get_ax_selected_text() {
    //             tracing::info!("-----> text: {}, x: {}, y: {}", text, x, y);
    //             // let _ = app_handle.emit("global-selection", (text, x, y));
    //         }
    //     }
    // });

    // // 监听线程 (Listen)
    // thread::spawn(move || {
    //     let mut last_x = 0.0;
    //     let mut last_y = 0.0;
    //     // 重点 3: 追踪修饰键状态
    //     let mut is_modifier_pressed = false;

    //     let _ = listen(move |event| {
    //         match event.name {
    //             Some(string) => tracing::info!("----> User wrote {:?}", string),
    //             None => (),
    //         }

    //         match event.event_type {
    //             EventType::MouseMove { x, y } => {
    //                 last_x = x;
    //                 last_y = y;
    //             }
    //             // 重点 4: 记录修饰键（Command, Tab, Alt等）的按下状态
    //             EventType::KeyPress(key) => {
    //                 if matches!(
    //                     key,
    //                     Key::MetaLeft | Key::MetaRight | Key::Tab | Key::Alt | Key::ControlLeft
    //                 ) {
    //                     is_modifier_pressed = true;
    //                     tracing::info!("-----> key: {:?}", key);
    //                 }
    //             }
    //             EventType::KeyRelease(key) => {
    //                 if matches!(
    //                     key,
    //                     Key::MetaLeft | Key::MetaRight | Key::Tab | Key::Alt | Key::ControlLeft
    //                 ) {
    //                     is_modifier_pressed = false;
    //                     tracing::info!("-----> key: {:?}", key);
    //                 }
    //             }
    //             EventType::ButtonRelease(rdev::Button::Left) => {
    //                 // 重点 5: 如果此时修饰键被按住，或者系统处于切换状态，直接丢弃事件
    //                 if !is_modifier_pressed {
    //                     let _ = tx.send((last_x, last_y));
    //                 }
    //             }
    //             _ => {}
    //         }
    //     });
    // });

    // // 1. 在独立线程启动监听
    // thread::spawn(move || {
    //     use rdev::listen;

    //     let mut last_x = 0.0;
    //     let mut last_y = 0.0;

    //     // 使用 match 优雅处理错误，而不是 .expect()
    //     if let Err(error) = listen(move |event| {
    //         use rdev::EventType;

    //         match event.event_type {
    //             // 实时更新位置，不执行耗时操作
    //             EventType::MouseMove { x, y } => {
    //                 last_x = x;
    //                 last_y = y;
    //             }
    //             // 当左键松开时
    //             EventType::ButtonRelease(rdev::Button::Left) => {
    //                 let handle = app.clone();
    //                 let x = last_x;
    //                 let y = last_y;

    //                 // 关键：再开启一个逻辑线程，避免阻塞系统的输入流
    //                 thread::spawn(move || {
    //                     // 稍微延迟，等待系统写入选中文本
    //                     thread::sleep(Duration::from_millis(200));

    //                     // 执行 Accessibility 查询 (此处也要注意安全，不使用 unwrap)
    //                     if let Some(text) = get_ax_selected_text() {
    //                         tracing::info!("-----> text: {}, x: {}, y: {}", text, x, y);
    //                         // 发送给前端
    //                         // let _ = handle.emit("global-selection", (text, x, y));
    //                     }
    //                 });
    //             }
    //             _ => (),
    //         }
    //     }) {
    //         eprintln!("监听器运行中出错: {:?}", error);
    //     }
    // });

    // tracing::info!("🔧 Starting selection observer with thread-safe implementation");

    // // 使用 tokio channel 将事件从 rdev 线程安全地传递到主线程
    // let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(f64, f64)>();

    // // 在独立线程启动 rdev 监听（但不在监听器中做任何复杂操作）
    // thread::spawn(move || {
    //     let mut last_x = 0.0;
    //     let mut last_y = 0.0;
    //     let mut last_log_time = Instant::now();

    //     if let Err(e) = listen(move |event| {
    //         match event.event_type {
    //             EventType::MouseMove { x, y } => {
    //                 last_x = x;
    //                 last_y = y;

    //                 // 限流日志：每秒最多 1 条，避免高频阻塞
    //                 if last_log_time.elapsed() > Duration::from_secs(1) {
    //                     tracing::debug!("Mouse: ({:.0}, {:.0})", x, y);
    //                     last_log_time = Instant::now();
    //                 }
    //             }
    //             EventType::ButtonRelease(rdev::Button::Left) => {
    //                 // 通过 channel 发送到主线程，不在这里做任何阻塞操作
    //                 let _ = tx.send((last_x, last_y));
    //             }
    //             _ => {}
    //         }
    //     }) {
    //         tracing::error!("Failed to start rdev listener: {:?}", e);
    //     }
    // });

    // // 在 Tauri 的 async runtime 中处理事件（主线程安全）
    // tauri::async_runtime::spawn(async move {
    //     tracing::info!("✅ Selection event handler started");

    //     while let Some((x, y)) = rx.recv().await {
    //         let app_clone = app.clone();

    //         // 延迟等待系统完成文本选择
    //         tokio::time::sleep(Duration::from_millis(200)).await;

    //         // 在主线程获取选中文本（线程安全）
    //         // 使用 std channel 因为 run_on_main_thread 不返回值
    //         let (text_tx, text_rx) = std::sync::mpsc::channel();

    //         let _ = app_clone.run_on_main_thread(move || {
    //             let text = get_ax_selected_text();
    //             let _ = text_tx.send(text);
    //         });

    //         // 等待主线程返回结果，设置超时
    //         if let Ok(Some(text)) = text_rx.recv_timeout(Duration::from_secs(1)) {
    //             tracing::info!("📝 Selected text: '{}' at ({:.0}, {:.0})", text, x, y);

    //             // TODO: 显示选择面板
    //             // show_overlay_window(&app_clone, &payload);
    //         }
    //     }

    //     tracing::warn!("Selection event handler stopped");
    // });

    // let app_handle = app.clone();
    // tauri::async_runtime::spawn(async move {
    //     ensure_accessibility_permission();

    //     let app_identifier = app_handle.config().identifier.clone();
    //     let mut last_signature: Option<String> = None;
    //     let mut overlay_visible = false;
    //     let mut hover_started_at: Option<Instant> = None;
    //     let mut last_mouse_position: Option<SelectionRect> = None;

    //     let mut interval = tokio::time::interval(Duration::from_millis(POLL_INTERVAL_MS));
    //     tracing::info!(
    //         "Selection observer started, polling every {}ms",
    //         POLL_INTERVAL_MS
    //     );
    //     loop {
    //         interval.tick().await;

    //         let overlay_locked = OVERLAY_LOCKED.load(Ordering::SeqCst);
    //         let snapshot = fetch_selection_snapshot();
    //         let Some(snapshot) = snapshot else {
    //             if overlay_visible && !overlay_locked {
    //                 tracing::debug!("No selection detected, hiding overlay");
    //                 hide_overlay_window_and_restore(&app_handle);
    //                 overlay_visible = false;
    //             }
    //             if !overlay_locked {
    //                 hover_started_at = None;
    //                 last_mouse_position = None;
    //                 last_signature = None;
    //             }
    //             continue;
    //         };

    //         if let Some(bundle_id) = snapshot.payload.source_bundle_id.as_ref() {
    //             tracing::debug!(
    //                 "Selection from bundle_id: {}, app_identifier: {}",
    //                 bundle_id,
    //                 app_identifier
    //             );
    //             if bundle_id == &app_identifier {
    //                 tracing::debug!("Ignoring selection from our own app");
    //                 if overlay_visible {
    //                     continue;
    //                 }
    //                 last_signature = None;
    //                 continue;
    //             }
    //         }

    //         let signature_changed = last_signature
    //             .as_ref()
    //             .map(|signature| signature != &snapshot.signature)
    //             .unwrap_or(true);

    //         if signature_changed {
    //             if overlay_visible {
    //                 hide_overlay_window_and_restore(&app_handle);
    //                 overlay_visible = false;
    //             }
    //             hover_started_at = None;
    //             last_mouse_position = None;
    //             last_signature = Some(snapshot.signature.clone());
    //             if let Ok(mut slot) = LAST_DISMISSED_SIGNATURE.lock() {
    //                 *slot = None;
    //             }
    //         }

    //         let mut payload = snapshot.payload.clone();
    //         let rect_valid = payload
    //             .rect
    //             .as_ref()
    //             .map(is_valid_selection_rect)
    //             .unwrap_or(false);
    //         let mouse = current_mouse_location();
    //         let mut rect_from_mouse = false;

    //         if rect_valid {
    //             if let Ok(mut anchor) = LAST_SELECTION_ANCHOR.lock() {
    //                 *anchor = payload.rect.clone();
    //             }
    //         } else {
    //             let anchor = mouse
    //                 .clone()
    //                 .map(selection_rect_from_point)
    //                 .or_else(|| {
    //                     LAST_SELECTION_ANCHOR
    //                         .lock()
    //                         .ok()
    //                         .and_then(|slot| slot.clone())
    //                 })
    //                 .or_else(|| payload.rect.clone());

    //             if let Some(anchor) = anchor {
    //                 rect_from_mouse = mouse.is_some();
    //                 payload.rect = Some(anchor);
    //             }
    //         }

    //         if !overlay_visible {
    //             let is_hovering = if rect_from_mouse {
    //                 is_mouse_still(&mut last_mouse_position, mouse.clone())
    //             } else {
    //                 payload
    //                     .rect
    //                     .as_ref()
    //                     .map(is_mouse_over_selection)
    //                     .unwrap_or(false)
    //             };

    //             tracing::debug!(
    //                 "Overlay pending: hovering={}, rect={:?}, rect_from_mouse={}, mouse={:?}",
    //                 is_hovering,
    //                 payload.rect,
    //                 rect_from_mouse,
    //                 mouse
    //             );

    //             if is_hovering {
    //                 let elapsed = hover_started_at.get_or_insert_with(Instant::now).elapsed();
    //                 if elapsed >= Duration::from_millis(HOVER_DELAY_MS) {
    //                     let dismissed = LAST_DISMISSED_SIGNATURE
    //                         .lock()
    //                         .ok()
    //                         .and_then(|slot| slot.clone());
    //                     if dismissed.as_deref() == Some(snapshot.signature.as_str()) {
    //                         continue;
    //                     }
    //                     if let Ok(mut slot) = LAST_SELECTION_PAYLOAD.lock() {
    //                         *slot = Some(payload.clone());
    //                     }
    //                     overlay_visible = true;
    //                     show_overlay_window(&app_handle, &payload);
    //                 }
    //             } else {
    //                 hover_started_at = None;
    //             }
    //         } else if signature_changed {
    //             hover_started_at = None;
    //         }
    //     }
    // });
}

// #[cfg(not(target_os = "macos"))]
// pub fn start_selection_observer(_app: tauri::AppHandle) {}

// #[cfg(target_os = "macos")]
// pub fn get_last_payload_json() -> Option<serde_json::Value> {
//     let payload = LAST_SELECTION_PAYLOAD
//         .lock()
//         .ok()
//         .and_then(|slot| slot.clone());
//     payload.and_then(|value| serde_json::to_value(value).ok())
// }

// #[cfg(target_os = "macos")]
// fn ensure_accessibility_permission() {
//     let trusted = ax_is_process_trusted(true);
//     if !trusted {
//         tracing::warn!("Accessibility permission not granted yet.");
//     }
// }

// #[cfg(target_os = "macos")]
// fn show_overlay_window(app: &AppHandle, payload: &SelectionPayload) {
//     use crate::services::selection_panel::get_menu_panel;

//     tracing::info!(
//         "show_overlay_window called for text: '{}'",
//         payload.text.chars().take(50).collect::<String>()
//     );

//     let Some(panel) = get_menu_panel(app) else {
//         tracing::error!("Selection menu panel not found!");
//         return;
//     };

//     // 发送选择数据到面板
//     if let Some(window) = panel.to_window() {
//         let _ = window.emit("selection_update", payload.clone());
//     }

//     // 定位并显示面板（必须在主线程执行）
//     let panel_clone = panel.clone();

//     app.run_on_main_thread(move || {
//         // 使用鼠标位置定位面板
//         let mouse = NSEvent::mouseLocation();

//         // macOS 坐标系统：原点在左下角，y 轴向上
//         // Tauri window 坐标系统：原点在左上角，y 轴向下
//         // 需要转换坐标系统

//         // 获取屏幕高度（用于坐标转换）
//         use objc2::rc::Retained;
//         use objc2_app_kit::NSScreen;
//         use objc2_foundation::MainThreadMarker;

//         let screen_height = unsafe {
//             let mtm = MainThreadMarker::new_unchecked();
//             NSScreen::mainScreen(mtm)
//                 .map(|s: Retained<NSScreen>| s.frame().size.height)
//                 .unwrap_or(1080.0)
//         };

//         // 转换到 Tauri 坐标系统（左上角为原点）
//         // macOS: y 从下往上（原点在左下角），Tauri: y 从上往下（原点在左上角）
//         // 鼠标位置在选中文字附近，面板应该显示在选中文字下方（鼠标下方）
//         // 计算：screen_height - mouse.y = 从屏幕顶部到鼠标的距离
//         // 加上 OVERLAY_VERTICAL_GAP 让面板出现在鼠标下方一点
//         let tauri_y = screen_height - mouse.y + OVERLAY_VERTICAL_GAP;
//         let tauri_x = mouse.x - MENU_WIDTH / 2.0; // 水平居中

//         // 使用 window 的 set_position 方法
//         if let Some(window) = panel_clone.to_window() {
//             use tauri::{LogicalPosition, Position};
//             let position = Position::Logical(LogicalPosition::new(tauri_x, tauri_y));

//             if let Err(e) = window.set_position(position) {
//                 tracing::error!("Failed to set panel position: {}", e);
//             }
//         }

//         // 显示面板并使其可交互
//         panel_clone.show_and_make_key();

//         tracing::info!(
//             "Panel positioned at Tauri({}, {}), macOS mouse at ({}, {})",
//             tauri_x,
//             tauri_y,
//             mouse.x,
//             mouse.y
//         );
//     })
//     .ok();

//     tracing::info!(
//         "Selection menu panel shown (text_len={})",
//         payload.text.len()
//     );
// }

// #[cfg(target_os = "macos")]
// fn hide_overlay_window(app: &AppHandle) {
//     let app_clone = app.clone();
//     app.run_on_main_thread(move || {
//         use crate::services::selection_panel::hide_all_panels;
//         hide_all_panels(&app_clone);
//     })
//     .ok();
// }

// #[cfg(target_os = "macos")]
// pub fn hide_overlay_window_and_restore(app: &AppHandle) {
//     hide_overlay_window(app);
// }

// #[cfg(target_os = "macos")]
// fn is_valid_selection_rect(rect: &SelectionRect) -> bool {
//     rect.width > 1.0 && rect.height > 1.0
// }

// #[cfg(target_os = "macos")]
// fn selection_rect_from_point(point: SelectionRect) -> SelectionRect {
//     // 创建一个合理大小的模拟选区(宽100px, 高20px, 以鼠标为中心)
//     SelectionRect {
//         x: point.x - 50.0, // 鼠标水平居中
//         y: point.y - 10.0, // 鼠标垂直居中
//         width: 100.0,
//         height: 20.0,
//     }
// }

// #[cfg(target_os = "macos")]
// fn is_mouse_over_selection(rect: &SelectionRect) -> bool {
//     let Some(point) = current_mouse_location() else {
//         return false;
//     };

//     let min_x = rect.x - SELECTION_HOVER_PADDING;
//     let max_x = rect.x + rect.width + SELECTION_HOVER_PADDING;
//     let min_y = rect.y - SELECTION_HOVER_PADDING;
//     let max_y = rect.y + rect.height + SELECTION_HOVER_PADDING;

//     point.x >= min_x && point.x <= max_x && point.y >= min_y && point.y <= max_y
// }

// #[cfg(target_os = "macos")]
// fn is_mouse_still(
//     last_mouse_position: &mut Option<SelectionRect>,
//     current_mouse: Option<SelectionRect>,
// ) -> bool {
//     let Some(current) = current_mouse else {
//         *last_mouse_position = None;
//         return false;
//     };

//     let still = last_mouse_position
//         .as_ref()
//         .map(|prev| {
//             (prev.x - current.x).abs() <= MOUSE_STILL_THRESHOLD
//                 && (prev.y - current.y).abs() <= MOUSE_STILL_THRESHOLD
//         })
//         .unwrap_or(false);

//     *last_mouse_position = Some(current);
//     still
// }

// #[cfg(target_os = "macos")]
// fn fetch_selection_snapshot() -> Option<SelectionSnapshot> {
//     let (focused_element, selected_text, raw_text, selection_rect, window_title) =
//         get_focused_selection()?;
//     if selected_text.trim().is_empty() {
//         unsafe { CFRelease(focused_element as CFTypeRef) };
//         return None;
//     }

//     let app_info = frontmost_app_info();

//     let (source_url, source_tab_title) =
//         match app_info.bundle_id.as_deref().and_then(browser_tab_info) {
//             Some((url, title)) => (Some(url), title),
//             None => (None, None),
//         };

//     let source_domain = source_url
//         .as_deref()
//         .and_then(|url| url::Url::parse(url).ok())
//         .and_then(|parsed| parsed.domain().map(|d| d.to_string()));

//     let payload = SelectionPayload {
//         text: selected_text.trim().to_string(),
//         raw_text,
//         rect: selection_rect,
//         source_app_name: app_info.name,
//         source_bundle_id: app_info.bundle_id,
//         source_pid: app_info.pid,
//         source_app_path: app_info.path,
//         source_app_version: None,
//         source_window_title: window_title,
//         source_url,
//         source_domain,
//         source_tab_title,
//         capture_method: Some("accessibility".to_string()),
//         locale: None,
//         input_language: None,
//     };

//     if let Ok(mut slot) = LAST_SELECTION_PAYLOAD.lock() {
//         *slot = Some(payload.clone());
//     }

//     tracing::info!(
//         "Selection captured (app={:?}, bundle_id={:?}, text_len={})",
//         payload.source_app_name,
//         payload.source_bundle_id,
//         payload.text.len()
//     );

//     let signature = format!(
//         "{}:{}:{:?}:{:?}",
//         payload.text,
//         payload.source_bundle_id.as_deref().unwrap_or(""),
//         payload.rect.as_ref().map(|r| (r.x, r.y, r.width, r.height)),
//         payload.source_url.as_deref()
//     );
//     if let Ok(mut slot) = LAST_SELECTION_SIGNATURE.lock() {
//         *slot = Some(signature.clone());
//     }

//     unsafe { CFRelease(focused_element as CFTypeRef) };

//     Some(SelectionSnapshot { payload, signature })
// }

// #[cfg(target_os = "macos")]
// fn get_focused_selection() -> Option<(
//     AXUIElementRef,
//     String,
//     String,
//     Option<SelectionRect>,
//     Option<String>,
// )> {
//     unsafe {
//         let system = AXUIElementCreateSystemWide();
//         if system.is_null() {
//             return None;
//         }

//         let focused_ref = ax_copy_attribute(system, &CFString::new("AXFocusedUIElement"))?;
//         CFRelease(system as CFTypeRef);

//         let focused = focused_ref as AXUIElementRef;
//         let raw_text = ax_copy_string_attribute(focused, "AXSelectedText")?;
//         let range = ax_copy_text_range(focused);
//         let rect = range.and_then(|range| ax_copy_bounds_for_range(focused, range));
//         let window_title = ax_copy_window_title(focused);

//         let normalized_rect_before_filter = rect.and_then(normalize_selection_rect);
//         let normalized_rect = normalized_rect_before_filter
//             .clone()
//             .filter(is_valid_selection_rect);

//         let mouse = current_mouse_location();
//         tracing::info!(
//             "Selection rect: raw={:?}, normalized_before_filter={:?}, normalized_after_filter={:?}, mouse={:?}",
//             rect,
//             normalized_rect_before_filter,
//             normalized_rect,
//             mouse
//         );

//         Some((
//             focused,
//             raw_text.trim().to_string(),
//             raw_text,
//             normalized_rect,
//             window_title,
//         ))
//     }
// }

// #[cfg(target_os = "macos")]
// fn ax_copy_window_title(element: AXUIElementRef) -> Option<String> {
//     unsafe {
//         let window_ref = ax_copy_attribute(element, &CFString::new("AXWindow"))?;
//         let title = ax_copy_string_attribute(window_ref as AXUIElementRef, "AXTitle");
//         CFRelease(window_ref);
//         title
//     }
// }

// #[cfg(target_os = "macos")]
// fn ax_copy_text_range(element: AXUIElementRef) -> Option<CFRange> {
//     unsafe {
//         let range_ref = ax_copy_attribute(element, &CFString::new("AXSelectedTextRange"))?;
//         let ax_value = range_ref as AXValueRef;
//         let mut range = CFRange {
//             location: 0,
//             length: 0,
//         };
//         let success = AXValueGetValue(
//             ax_value,
//             K_AX_VALUE_TYPE_CF_RANGE,
//             &mut range as *mut _ as *mut c_void,
//         );
//         CFRelease(range_ref);
//         if success {
//             Some(range)
//         } else {
//             None
//         }
//     }
// }

// #[cfg(target_os = "macos")]
// fn ax_copy_bounds_for_range(element: AXUIElementRef, range: CFRange) -> Option<CGRect> {
//     unsafe {
//         let value_ref = AXValueCreate(
//             K_AX_VALUE_TYPE_CF_RANGE,
//             &range as *const _ as *const c_void,
//         );
//         if value_ref.is_null() {
//             return None;
//         }
//         let bounds_ref = ax_copy_parameterized_attribute(
//             element,
//             &CFString::new("AXBoundsForRange"),
//             value_ref as CFTypeRef,
//         );
//         CFRelease(value_ref as CFTypeRef);

//         let bounds_ref = bounds_ref?;
//         let mut rect = CGRect::default();
//         let success = AXValueGetValue(
//             bounds_ref as AXValueRef,
//             K_AX_VALUE_TYPE_CG_RECT,
//             &mut rect as *mut _ as *mut c_void,
//         );
//         CFRelease(bounds_ref);
//         if success {
//             Some(rect)
//         } else {
//             None
//         }
//     }
// }

// #[cfg(target_os = "macos")]
// fn ax_copy_string_attribute(element: AXUIElementRef, name: &str) -> Option<String> {
//     unsafe {
//         let value = ax_copy_attribute(element, &CFString::new(name))?;
//         let cf_string = CFString::wrap_under_create_rule(value as CFStringRef);
//         Some(cf_string.to_string())
//     }
// }

// #[cfg(target_os = "macos")]
// fn ax_copy_attribute(element: AXUIElementRef, attribute: &CFString) -> Option<CFTypeRef> {
//     unsafe {
//         let mut value: CFTypeRef = std::ptr::null();
//         let result =
//             AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value);
//         if result == K_AX_ERROR_SUCCESS && !value.is_null() {
//             Some(value)
//         } else {
//             None
//         }
//     }
// }

// #[cfg(target_os = "macos")]
// fn ax_copy_parameterized_attribute(
//     element: AXUIElementRef,
//     attribute: &CFString,
//     parameter: CFTypeRef,
// ) -> Option<CFTypeRef> {
//     unsafe {
//         let mut value: CFTypeRef = std::ptr::null();
//         let result = AXUIElementCopyParameterizedAttributeValue(
//             element,
//             attribute.as_concrete_TypeRef(),
//             parameter,
//             &mut value,
//         );
//         if result == K_AX_ERROR_SUCCESS && !value.is_null() {
//             Some(value)
//         } else {
//             None
//         }
//     }
// }

// #[cfg(target_os = "macos")]
// fn rect_to_top_left(rect: CGRect) -> SelectionRect {
//     let screen_height = CGDisplay::main().bounds().size.height;
//     SelectionRect {
//         x: rect.origin.x,
//         y: screen_height - rect.origin.y - rect.size.height,
//         width: rect.size.width,
//         height: rect.size.height,
//     }
// }

// #[cfg(target_os = "macos")]
// fn normalize_selection_rect(rect: CGRect) -> Option<SelectionRect> {
//     let top_left = rect_to_top_left(rect);
//     let as_is = SelectionRect {
//         x: rect.origin.x,
//         y: rect.origin.y,
//         width: rect.size.width,
//         height: rect.size.height,
//     };

//     tracing::info!(
//         "normalize_selection_rect: CGRect=({}, {}, {}x{}), as_is=({}, {}, {}x{}), top_left=({}, {}, {}x{})",
//         rect.origin.x, rect.origin.y, rect.size.width, rect.size.height,
//         as_is.x, as_is.y, as_is.width, as_is.height,
//         top_left.x, top_left.y, top_left.width, top_left.height
//     );

//     // 优先使用 top_left 坐标系(Tauri 标准)
//     // 如果有鼠标位置,验证哪个坐标系正确
//     if let Some(mouse) = current_mouse_location() {
//         tracing::info!("Mouse location: ({}, {})", mouse.x, mouse.y);

//         if is_point_in_rect(&mouse, &top_left) {
//             tracing::info!("Using top_left coordinate system");
//             return Some(top_left);
//         }
//         if is_point_in_rect(&mouse, &as_is) {
//             tracing::info!("Using as_is coordinate system");
//             return Some(as_is);
//         }
//         // 即使鼠标不在矩形内,也返回 top_left (用户可能移动了鼠标)
//         tracing::info!("Mouse not in rect, using top_left anyway");
//     }

//     Some(top_left)
// }

// #[cfg(target_os = "macos")]
// fn current_mouse_location() -> Option<SelectionRect> {
//     let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
//     let event = CGEvent::new(source).ok()?;
//     let point = event.location();
//     let screen_height = CGDisplay::main().bounds().size.height;
//     Some(SelectionRect {
//         x: point.x,
//         y: screen_height - point.y,
//         width: 0.0,
//         height: 0.0,
//     })
// }

// #[cfg(target_os = "macos")]
// fn is_point_in_rect(point: &SelectionRect, rect: &SelectionRect) -> bool {
//     let min_x = rect.x - SELECTION_HOVER_PADDING;
//     let max_x = rect.x + rect.width + SELECTION_HOVER_PADDING;
//     let min_y = rect.y - SELECTION_HOVER_PADDING;
//     let max_y = rect.y + rect.height + SELECTION_HOVER_PADDING;

//     point.x >= min_x && point.x <= max_x && point.y >= min_y && point.y <= max_y
// }

// #[cfg(target_os = "macos")]
// fn browser_tab_info(bundle_id: &str) -> Option<(String, Option<String>)> {
//     let (app_name, title_key) = match bundle_id {
//         "com.apple.Safari" => ("Safari", "name"),
//         "com.google.Chrome" => ("Google Chrome", "title"),
//         "com.microsoft.Edge" => ("Microsoft Edge", "title"),
//         "com.brave.Browser" => ("Brave Browser", "title"),
//         "com.operasoftware.Opera" => ("Opera", "title"),
//         _ => return None,
//     };

//     let script = format!(
//         "tell application \"{app}\" to if (exists front window) then return (URL of active tab of front window) & \"\\n\" & ({title} of active tab of front window) else return \"\"",
//         app = app_name,
//         title = title_key
//     );

//     let output = Command::new("osascript")
//         .arg("-e")
//         .arg(script)
//         .output()
//         .ok()?;
//     if !output.status.success() {
//         return None;
//     }

//     let stdout = String::from_utf8_lossy(&output.stdout);
//     let mut lines = stdout
//         .lines()
//         .map(|line| line.trim())
//         .filter(|line| !line.is_empty());
//     let url = lines.next()?.to_string();
//     let title = lines.next().map(|line| line.to_string());
//     Some((url, title))
// }

// #[cfg(target_os = "macos")]
// #[derive(Default)]
// struct AppInfo {
//     name: Option<String>,
//     bundle_id: Option<String>,
//     pid: Option<i64>,
//     path: Option<String>,
// }

// #[cfg(target_os = "macos")]
// fn frontmost_app_info() -> AppInfo {
//     autoreleasepool(|pool| {
//         let workspace = NSWorkspace::sharedWorkspace();
//         let Some(app) = workspace.frontmostApplication() else {
//             return AppInfo::default();
//         };

//         let name = app
//             .localizedName()
//             .map(|value| unsafe { value.to_str(pool) }.to_string());
//         let bundle_id = app
//             .bundleIdentifier()
//             .map(|value| unsafe { value.to_str(pool) }.to_string());
//         let pid: i32 = app.processIdentifier();
//         let path = app
//             .bundleURL()
//             .and_then(|url| url.path())
//             .map(|value| unsafe { value.to_str(pool) }.to_string());

//         AppInfo {
//             name,
//             bundle_id,
//             pid: if pid < 0 { None } else { Some(pid as i64) },
//             path,
//         }
//     })
// }

// #[cfg(target_os = "macos")]
// fn ax_is_process_trusted(prompt: bool) -> bool {
//     unsafe {
//         let key = CFString::new("AXTrustedCheckOptionPrompt");
//         let value = if prompt {
//             CFBoolean::true_value()
//         } else {
//             CFBoolean::false_value()
//         };
//         let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
//         AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as CFTypeRef)
//     }
// }

// #[cfg(target_os = "macos")]
// type AXUIElementRef = *const c_void;
// #[cfg(target_os = "macos")]
// type AXValueRef = *const c_void;
// #[cfg(target_os = "macos")]
// type AXError = i32;

// #[cfg(target_os = "macos")]
// const K_AX_ERROR_SUCCESS: AXError = 0;
// #[cfg(target_os = "macos")]
// const K_AX_VALUE_TYPE_CF_RANGE: u32 = 4;
// #[cfg(target_os = "macos")]
// const K_AX_VALUE_TYPE_CG_RECT: u32 = 3;

// #[cfg(target_os = "macos")]
// extern "C" {
//     fn AXUIElementCreateSystemWide() -> AXUIElementRef;
//     fn AXUIElementCopyAttributeValue(
//         element: AXUIElementRef,
//         attribute: CFStringRef,
//         value: *mut CFTypeRef,
//     ) -> AXError;
//     fn AXUIElementCopyParameterizedAttributeValue(
//         element: AXUIElementRef,
//         attribute: CFStringRef,
//         parameter: CFTypeRef,
//         value: *mut CFTypeRef,
//     ) -> AXError;
//     fn AXValueCreate(the_type: u32, value: *const c_void) -> AXValueRef;
//     fn AXValueGetValue(value: AXValueRef, the_type: u32, value_ptr: *mut c_void) -> bool;
//     fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
// }
