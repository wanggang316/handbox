// macOS system selection observer for external text actions.

#[cfg(target_os = "macos")]
use core_foundation::base::{CFRange, CFRelease, CFTypeRef, TCFType};
#[cfg(target_os = "macos")]
use core_foundation::boolean::CFBoolean;
#[cfg(target_os = "macos")]
use core_foundation::dictionary::CFDictionary;
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
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
use objc2_app_kit::NSWorkspace;
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager};
use serde::Serialize;
#[cfg(target_os = "macos")]
use once_cell::sync::Lazy;

const OVERLAY_WINDOW_LABEL: &str = "selection_overlay";
const OVERLAY_WIDTH: f64 = 420.0;
const OVERLAY_HEIGHT: f64 = 260.0;
const OVERLAY_PADDING: f64 = 12.0;
const POLL_INTERVAL_MS: u64 = 250;

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
pub fn start_selection_observer(app: AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        ensure_accessibility_permission();

        let app_identifier = app_handle.config().identifier.clone();
        let mut last_signature: Option<String> = None;
        let mut overlay_visible = false;

        let mut interval = tokio::time::interval(Duration::from_millis(POLL_INTERVAL_MS));
        tracing::info!("Selection observer started, polling every {}ms", POLL_INTERVAL_MS);
        loop {
            interval.tick().await;

            let snapshot = fetch_selection_snapshot();
            let Some(snapshot) = snapshot else {
                if overlay_visible {
                    tracing::debug!("No selection detected, hiding overlay");
                    hide_overlay_window(&app_handle);
                    overlay_visible = false;
                    last_signature = None;
                }
                continue;
            };

            if let Some(bundle_id) = snapshot.payload.source_bundle_id.as_ref() {
                tracing::debug!(
                    "Selection from bundle_id: {}, app_identifier: {}",
                    bundle_id,
                    app_identifier
                );
                if bundle_id == &app_identifier {
                    tracing::debug!("Ignoring selection from our own app");
                    if overlay_visible {
                        continue;
                    }
                    last_signature = None;
                    continue;
                }
            }

            if last_signature.as_ref() == Some(&snapshot.signature) {
                continue;
            }

            last_signature = Some(snapshot.signature);
            overlay_visible = true;
            show_overlay_window(&app_handle, &snapshot.payload);
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn start_selection_observer(_app: tauri::AppHandle) {}

#[cfg(target_os = "macos")]
pub fn get_last_payload_json() -> Option<serde_json::Value> {
    let payload = LAST_SELECTION_PAYLOAD
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    payload.and_then(|value| serde_json::to_value(value).ok())
}

#[cfg(target_os = "macos")]
fn ensure_accessibility_permission() {
    let trusted = ax_is_process_trusted(true);
    if !trusted {
        tracing::warn!("Accessibility permission not granted yet.");
    }
}

#[cfg(target_os = "macos")]
fn show_overlay_window(app: &AppHandle, payload: &SelectionPayload) {
    tracing::info!("show_overlay_window called for text: '{}'", &payload.text[..payload.text.len().min(50)]);

    let Some(window) = app.get_webview_window(OVERLAY_WINDOW_LABEL) else {
        tracing::error!("Selection overlay window not found!");
        return;
    };

    let (x, y) = compute_overlay_position(payload.rect.as_ref());

    if let Err(error) = window.set_size(LogicalSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT)) {
        tracing::error!("Failed to resize overlay window: {error}");
        return;
    }

    if let Err(error) = window.set_position(LogicalPosition::new(x, y)) {
        tracing::error!("Failed to position overlay window: {error}");
    }
    if let Err(error) = window.show() {
        tracing::error!("Failed to show overlay window: {error}");
    } else {
        tracing::info!("Overlay window shown successfully");
    }
    tracing::info!(
        "Selection overlay shown (text_len={}, x={}, y={})",
        payload.text.len(),
        x,
        y
    );
}

#[cfg(target_os = "macos")]
fn hide_overlay_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_WINDOW_LABEL) {
        let _ = window.hide();
    }
}

#[cfg(target_os = "macos")]
fn compute_overlay_position(rect: Option<&SelectionRect>) -> (f64, f64) {
    let display_bounds = CGDisplay::main().bounds();
    let screen_width = display_bounds.size.width;
    let screen_height = display_bounds.size.height;

    tracing::info!(
        "Screen bounds: width={}, height={}",
        screen_width,
        screen_height
    );
    tracing::info!("Selection rect: {:?}", rect);

    let (anchor_x, anchor_y, anchor_height) = if let Some(rect) = rect {
        (rect.x + rect.width / 2.0, rect.y, rect.height)
    } else if let Some(point) = current_mouse_location() {
        tracing::debug!("Using mouse location: {:?}", point);
        (point.x, point.y, 0.0)
    } else {
        tracing::debug!("Using screen center as fallback");
        (screen_width / 2.0, screen_height / 2.0, 0.0)
    };

    let mut x = anchor_x - OVERLAY_WIDTH / 2.0;
    let mut y = anchor_y - OVERLAY_HEIGHT - 8.0;

    tracing::info!(
        "Initial position: x={}, y={} (anchor_x={}, anchor_y={}, anchor_height={})",
        x,
        y,
        anchor_x,
        anchor_y,
        anchor_height
    );

    if y < OVERLAY_PADDING {
        y = anchor_y + anchor_height + 8.0;
        tracing::info!("Adjusted y to below selection: {}", y);
    }

    x = x.clamp(
        OVERLAY_PADDING,
        screen_width - OVERLAY_WIDTH - OVERLAY_PADDING,
    );
    y = y.clamp(
        OVERLAY_PADDING,
        screen_height - OVERLAY_HEIGHT - OVERLAY_PADDING,
    );

    tracing::info!("Final clamped position: x={}, y={}", x, y);

    (x, y)
}

#[cfg(target_os = "macos")]
fn fetch_selection_snapshot() -> Option<SelectionSnapshot> {
    let (focused_element, selected_text, raw_text, selection_rect, window_title) =
        get_focused_selection()?;
    if selected_text.trim().is_empty() {
        unsafe { CFRelease(focused_element as CFTypeRef) };
        return None;
    }

    let app_info = frontmost_app_info();

    let (source_url, source_tab_title) =
        match app_info.bundle_id.as_deref().and_then(browser_tab_info) {
            Some((url, title)) => (Some(url), title),
            None => (None, None),
        };

    let source_domain = source_url
        .as_deref()
        .and_then(|url| url::Url::parse(url).ok())
        .and_then(|parsed| parsed.domain().map(|d| d.to_string()));

    let payload = SelectionPayload {
        text: selected_text.trim().to_string(),
        raw_text,
        rect: selection_rect,
        source_app_name: app_info.name,
        source_bundle_id: app_info.bundle_id,
        source_pid: app_info.pid,
        source_app_path: app_info.path,
        source_app_version: None,
        source_window_title: window_title,
        source_url,
        source_domain,
        source_tab_title,
        capture_method: Some("accessibility".to_string()),
        locale: None,
        input_language: None,
    };

    if let Ok(mut slot) = LAST_SELECTION_PAYLOAD.lock() {
        *slot = Some(payload.clone());
    }

    tracing::info!(
        "Selection captured (app={:?}, bundle_id={:?}, text_len={})",
        payload.source_app_name,
        payload.source_bundle_id,
        payload.text.len()
    );

    let signature = format!(
        "{}:{}:{:?}:{:?}",
        payload.text,
        payload.source_bundle_id.as_deref().unwrap_or(""),
        payload.rect.as_ref().map(|r| (r.x, r.y, r.width, r.height)),
        payload.source_url.as_deref()
    );

    unsafe { CFRelease(focused_element as CFTypeRef) };

    Some(SelectionSnapshot { payload, signature })
}

#[cfg(target_os = "macos")]
fn get_focused_selection() -> Option<(
    AXUIElementRef,
    String,
    String,
    Option<SelectionRect>,
    Option<String>,
)> {
    unsafe {
        let system = AXUIElementCreateSystemWide();
        if system.is_null() {
            return None;
        }

        let focused_ref = ax_copy_attribute(system, &CFString::new("AXFocusedUIElement"))?;
        CFRelease(system as CFTypeRef);

        let focused = focused_ref as AXUIElementRef;
        let raw_text = ax_copy_string_attribute(focused, "AXSelectedText")?;
        let range = ax_copy_text_range(focused);
        let rect = range.and_then(|range| ax_copy_bounds_for_range(focused, range));
        let window_title = ax_copy_window_title(focused);

        let normalized_rect = rect.map(|rect| rect_to_top_left(rect));

        Some((
            focused,
            raw_text.trim().to_string(),
            raw_text,
            normalized_rect,
            window_title,
        ))
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_window_title(element: AXUIElementRef) -> Option<String> {
    unsafe {
        let window_ref = ax_copy_attribute(element, &CFString::new("AXWindow"))?;
        let title = ax_copy_string_attribute(window_ref as AXUIElementRef, "AXTitle");
        CFRelease(window_ref);
        title
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_text_range(element: AXUIElementRef) -> Option<CFRange> {
    unsafe {
        let range_ref = ax_copy_attribute(element, &CFString::new("AXSelectedTextRange"))?;
        let ax_value = range_ref as AXValueRef;
        let mut range = CFRange {
            location: 0,
            length: 0,
        };
        let success = AXValueGetValue(
            ax_value,
            K_AX_VALUE_TYPE_CF_RANGE,
            &mut range as *mut _ as *mut c_void,
        );
        CFRelease(range_ref);
        if success {
            Some(range)
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_bounds_for_range(element: AXUIElementRef, range: CFRange) -> Option<CGRect> {
    unsafe {
        let value_ref = AXValueCreate(
            K_AX_VALUE_TYPE_CF_RANGE,
            &range as *const _ as *const c_void,
        );
        if value_ref.is_null() {
            return None;
        }
        let bounds_ref = ax_copy_parameterized_attribute(
            element,
            &CFString::new("AXBoundsForRange"),
            value_ref as CFTypeRef,
        );
        CFRelease(value_ref as CFTypeRef);

        let bounds_ref = bounds_ref?;
        let mut rect = CGRect::default();
        let success = AXValueGetValue(
            bounds_ref as AXValueRef,
            K_AX_VALUE_TYPE_CG_RECT,
            &mut rect as *mut _ as *mut c_void,
        );
        CFRelease(bounds_ref);
        if success {
            Some(rect)
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_string_attribute(element: AXUIElementRef, name: &str) -> Option<String> {
    unsafe {
        let value = ax_copy_attribute(element, &CFString::new(name))?;
        let cf_string = CFString::wrap_under_create_rule(value as CFStringRef);
        Some(cf_string.to_string())
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_attribute(element: AXUIElementRef, attribute: &CFString) -> Option<CFTypeRef> {
    unsafe {
        let mut value: CFTypeRef = std::ptr::null();
        let result =
            AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value);
        if result == K_AX_ERROR_SUCCESS && !value.is_null() {
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn ax_copy_parameterized_attribute(
    element: AXUIElementRef,
    attribute: &CFString,
    parameter: CFTypeRef,
) -> Option<CFTypeRef> {
    unsafe {
        let mut value: CFTypeRef = std::ptr::null();
        let result = AXUIElementCopyParameterizedAttributeValue(
            element,
            attribute.as_concrete_TypeRef(),
            parameter,
            &mut value,
        );
        if result == K_AX_ERROR_SUCCESS && !value.is_null() {
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn rect_to_top_left(rect: CGRect) -> SelectionRect {
    let screen_height = CGDisplay::main().bounds().size.height;
    SelectionRect {
        x: rect.origin.x,
        y: screen_height - rect.origin.y - rect.size.height,
        width: rect.size.width,
        height: rect.size.height,
    }
}

#[cfg(target_os = "macos")]
fn current_mouse_location() -> Option<SelectionRect> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let point = event.location();
    let screen_height = CGDisplay::main().bounds().size.height;
    Some(SelectionRect {
        x: point.x,
        y: screen_height - point.y,
        width: 0.0,
        height: 0.0,
    })
}

#[cfg(target_os = "macos")]
fn browser_tab_info(bundle_id: &str) -> Option<(String, Option<String>)> {
    let (app_name, title_key) = match bundle_id {
        "com.apple.Safari" => ("Safari", "name"),
        "com.google.Chrome" => ("Google Chrome", "title"),
        "com.microsoft.Edge" => ("Microsoft Edge", "title"),
        "com.brave.Browser" => ("Brave Browser", "title"),
        "com.operasoftware.Opera" => ("Opera", "title"),
        _ => return None,
    };

    let script = format!(
        "tell application \"{app}\" to if (exists front window) then return (URL of active tab of front window) & \"\\n\" & ({title} of active tab of front window) else return \"\"",
        app = app_name,
        title = title_key
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty());
    let url = lines.next()?.to_string();
    let title = lines.next().map(|line| line.to_string());
    Some((url, title))
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct AppInfo {
    name: Option<String>,
    bundle_id: Option<String>,
    pid: Option<i64>,
    path: Option<String>,
}

#[cfg(target_os = "macos")]
fn frontmost_app_info() -> AppInfo {
    autoreleasepool(|pool| {
        let workspace = NSWorkspace::sharedWorkspace();
        let Some(app) = workspace.frontmostApplication() else {
            return AppInfo::default();
        };

        let name = app
            .localizedName()
            .map(|value| unsafe { value.to_str(pool) }.to_string());
        let bundle_id = app
            .bundleIdentifier()
            .map(|value| unsafe { value.to_str(pool) }.to_string());
        let pid: i32 = app.processIdentifier();
        let path = app
            .bundleURL()
            .and_then(|url| url.path())
            .map(|value| unsafe { value.to_str(pool) }.to_string());

        AppInfo {
            name,
            bundle_id,
            pid: if pid < 0 { None } else { Some(pid as i64) },
            path,
        }
    })
}

#[cfg(target_os = "macos")]
fn ax_is_process_trusted(prompt: bool) -> bool {
    unsafe {
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = if prompt {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };
        let options = CFDictionary::from_CFType_pairs(&[(key, value)]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as CFTypeRef)
    }
}

#[cfg(target_os = "macos")]
type AXUIElementRef = *const c_void;
#[cfg(target_os = "macos")]
type AXValueRef = *const c_void;
#[cfg(target_os = "macos")]
type AXError = i32;

#[cfg(target_os = "macos")]
const K_AX_ERROR_SUCCESS: AXError = 0;
#[cfg(target_os = "macos")]
const K_AX_VALUE_TYPE_CF_RANGE: u32 = 4;
#[cfg(target_os = "macos")]
const K_AX_VALUE_TYPE_CG_RECT: u32 = 3;

#[cfg(target_os = "macos")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXValueCreate(the_type: u32, value: *const c_void) -> AXValueRef;
    fn AXValueGetValue(value: AXValueRef, the_type: u32, value_ptr: *mut c_void) -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
}
