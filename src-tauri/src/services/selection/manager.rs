
use mouce::{Mouse, MouseActions};
#[cfg(target_os = "macos")]
use tauri::{AppHandle, Emitter, Manager, Wry};
use core_graphics::event::{CGEventType, EventField};

use crate::services::SettingsService;
use crate::utils::accessibility::get_ax_selected_text;
use crate::services::selection::menu_panel::init_panel as init_menu_panel;
use crate::services::selection::content_panel::init_panel as init_content_panel;
use crate::services::selection::menu_panel::hide_panel as hide_menu_panel;
// use crate::services::selection::content_panel::hide_panel as hide_content_panel;
use crate::services::selection::menu_panel::show_panel as show_menu_panel;
// use crate::services::selection::content_panel::show_panel as show_content_panel;

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
                mouce::common::MouseEvent::Press(mouce::common::MouseButton::Left) => {
                    hide_menu_panel(&handle_clone);
                }
                // 3. 左键松开：这是你划词逻辑的触发点
                mouce::common::MouseEvent::Release(mouce::common::MouseButton::Left) => {
                    trigger_selection_logic(&handle_clone);
                }
                mouce::common::MouseEvent::RelativeMove(x, y) => {
                    // tracing::info!("======> x: {}, y: {}", x, y);
                }
                mouce::common::MouseEvent::AbsoluteMove(x, y) => {
                    // tracing::info!("-----> x: {}, y: {}", x, y);
                }
                _ => {}
            }
        })).expect("无法启动 mouce hook");
    });
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

                    show_menu_panel(&handle_clone.clone());
                }
                _ => (),
            }
        });
    }
}

/// 检查选中文本工具栏功能是否启用
fn is_selection_toolbar_enabled(handle: &AppHandle) -> bool {
    let settings_service: tauri::State<'_, SettingsService> = handle.state();
    match settings_service.get_settings() {
        Ok(settings) => settings.quick_tools.show_toolbar_on_selection,
        Err(_) => false,
    }
}

/// Keyboard 监听和处理模块
fn setup_keyboard_monitor(handle: AppHandle<Wry>) {
    std::thread::spawn(move || {
        if let Ok(tap) = core_graphics::event::CGEventTap::new(
            core_graphics::event::CGEventTapLocation::HID,
            core_graphics::event::CGEventTapPlacement::HeadInsertEventTap,
            core_graphics::event::CGEventTapOptions::Default,
            vec![CGEventType::KeyDown],
            move |_, _, event| {
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
                    core_foundation::runloop::kCFRunLoopCommonModes
                );

                tap.enable();
                core_foundation::runloop::CFRunLoopRun();
            }
        }
    });
}