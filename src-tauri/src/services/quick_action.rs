//! Global shortcut for the Quick Action overlay, registered through
//! `tauri-plugin-global-shortcut`. The handler runs only on key-DOWN
//! (`ShortcutState::Pressed`) so a single keypress does not toggle twice.
//!
//! [`CURRENT_ACCELERATOR`] is the process-wide source of truth: re-registering
//! unregisters the recorded combination first, so a replaced accelerator stops
//! firing (live rebind from the settings page relies on this).

use crate::models::error::AppError;

#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const LOG_PREFIX: &str = "[QuickActionShortcut::register]";

/// Accelerator currently registered; `None` means nothing is registered.
#[cfg(target_os = "macos")]
static CURRENT_ACCELERATOR: Mutex<Option<String>> = Mutex::new(None);

/// Parses an accelerator string, returning a structured [`AppError`] instead of
/// panicking on malformed input.
#[cfg(target_os = "macos")]
fn parse_accelerator(accelerator: &str) -> Result<Shortcut, AppError> {
    use std::str::FromStr;

    Shortcut::from_str(accelerator).map_err(|e| {
        let message = format!("无法解析全局快捷键 \"{accelerator}\": {e}");
        tracing::error!("{LOG_PREFIX} {message}");
        AppError::with_hint(
            "QUICK_ACTION_SHORTCUT_INVALID",
            &message,
            "请检查快捷键格式（如 \"CmdOrCtrl+Shift+Space\"）",
        )
    })
}

/// Registers (or re-registers) the Quick Action global shortcut. Failures come
/// back as structured [`AppError`]s and the caller decides whether they are
/// fatal; the startup path only logs and continues.
#[cfg(target_os = "macos")]
pub fn register_shortcut(app: &AppHandle, accelerator: &str) -> Result<(), AppError> {
    let shortcut = parse_accelerator(accelerator)?;

    let gs = app.global_shortcut();

    // Unregister the recorded accelerator so a replaced combination stops firing.
    let mut current = CURRENT_ACCELERATOR.lock().map_err(|_| {
        let message = "全局快捷键状态锁已损坏".to_string();
        tracing::error!("{LOG_PREFIX} {message}");
        AppError::internal_error(&message)
    })?;

    if let Some(previous) = current.take() {
        if previous != accelerator {
            if let Ok(prev_shortcut) = parse_accelerator(&previous) {
                if let Err(e) = gs.unregister(prev_shortcut) {
                    // A failed unregister must not block the new registration.
                    tracing::warn!(
                        "{LOG_PREFIX} failed to unregister previous \"{previous}\": {e}"
                    );
                }
            }
        }
    }

    let handle = app.clone();
    gs.on_shortcut(shortcut, move |_app, _shortcut, event| {
        // Key-DOWN only, otherwise down/up would toggle twice per keypress.
        if event.state != ShortcutState::Pressed {
            return;
        }
        toggle_overlay(&handle);
    })
    .map_err(|e| {
        let message = format!("注册全局快捷键 \"{accelerator}\" 失败: {e}");
        tracing::error!("{LOG_PREFIX} {message}");
        AppError::with_hint(
            "QUICK_ACTION_SHORTCUT_REGISTER_FAILED",
            &message,
            "该快捷键可能已被其他应用占用，请更换组合",
        )
    })?;

    *current = Some(accelerator.to_string());
    tracing::info!("{LOG_PREFIX} registered \"{accelerator}\"");
    Ok(())
}

/// Unregisters the Quick Action global shortcut; an idempotent no-op when
/// nothing is recorded. Unregister failures are logged and swallowed so
/// disabling the feature always succeeds.
#[cfg(target_os = "macos")]
pub fn unregister_shortcut(app: &AppHandle) -> Result<(), AppError> {
    let mut current = CURRENT_ACCELERATOR.lock().map_err(|_| {
        let message = "全局快捷键状态锁已损坏".to_string();
        tracing::error!("{LOG_PREFIX} {message}");
        AppError::internal_error(&message)
    })?;

    if let Some(previous) = current.take() {
        if let Ok(shortcut) = parse_accelerator(&previous) {
            if let Err(e) = app.global_shortcut().unregister(shortcut) {
                tracing::warn!("{LOG_PREFIX} failed to unregister \"{previous}\": {e}");
            }
        }
        tracing::info!("{LOG_PREFIX} unregistered \"{previous}\"");
    }
    Ok(())
}

/// Hides the overlay when visible, otherwise shows it at the current cursor
/// position. Never re-shows an already visible panel.
#[cfg(target_os = "macos")]
fn toggle_overlay(app: &AppHandle) {
    use crate::services::selection::{
        hide_quick_action_panel, quick_action_panel::is_panel_visible, show_quick_action_panel,
    };

    if is_panel_visible() {
        tracing::info!("{LOG_PREFIX} hotkey toggle -> hide");
        hide_quick_action_panel(app);
    } else {
        let (x, y) = match app.cursor_position() {
            Ok(pos) => (pos.x, pos.y),
            Err(e) => {
                tracing::warn!(
                    "{LOG_PREFIX} failed to read cursor position, defaulting to (0,0): {e}"
                );
                (0.0, 0.0)
            }
        };
        tracing::info!("{LOG_PREFIX} hotkey toggle -> show at ({x}, {y})");
        show_quick_action_panel(app, x, y);
    }
}

/// No-op off macOS: the overlay is an NSPanel, so the shortcut is macOS-only.
#[cfg(not(target_os = "macos"))]
pub fn register_shortcut(_app: &tauri::AppHandle, _accelerator: &str) -> Result<(), AppError> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn unregister_shortcut(_app: &tauri::AppHandle) -> Result<(), AppError> {
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn parse_accelerator_rejects_malformed_string_with_structured_error() {
        let err = parse_accelerator("ThisIsNotAValidAccelerator!!").unwrap_err();
        assert_eq!(err.code, "QUICK_ACTION_SHORTCUT_INVALID");
        assert!(
            err.message.contains("ThisIsNotAValidAccelerator!!"),
            "message should echo the offending accelerator, got: {}",
            err.message
        );
        assert!(err.hint.is_some(), "structured error must carry a hint");
    }

    #[test]
    fn parse_accelerator_rejects_empty_string() {
        let err = parse_accelerator("").unwrap_err();
        assert_eq!(err.code, "QUICK_ACTION_SHORTCUT_INVALID");
    }

    // The configured default accelerator must parse cleanly (guards against a
    // regression in the default-shortcut string / plugin accelerator grammar).
    #[test]
    fn parse_accelerator_accepts_the_configured_default() {
        assert!(parse_accelerator("CmdOrCtrl+Shift+Space").is_ok());
    }
}
