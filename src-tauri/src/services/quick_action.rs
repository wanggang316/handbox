//! Quick Action 全局快捷键注册。
//!
//! 把持久化的 `quickAction.shortcut` 加速键注册为系统级全局快捷键，按下即切换
//! Quick Action 浮层（可见则隐藏、隐藏则显示）。注册经
//! `tauri-plugin-global-shortcut`：解析加速键字符串 → `app.global_shortcut()
//! .on_shortcut(...)`，handler 仅在 key-DOWN（`ShortcutState::Pressed`）触发以
//! 避免一次按键被 down/up 触发两次。
//!
//! 注册的进程级单一事实来源是 [`CURRENT_ACCELERATOR`]：re-register 时先按它
//! 反注册旧组合，再注册新组合，使被替换的旧组合彻底失活（设置页 live rebind
//! 依赖此语义）。
//!
//! 所有失败路径返回项目统一的结构化错误 [`AppError`]，并经 `tracing` 以
//! `[QuickActionShortcut::register]` 前缀记录（契约声明的证据）。

use crate::models::error::AppError;

#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// `tracing` / 日志前缀；同时是 VAL-OVERLAY-016 声明的证据串。
const LOG_PREFIX: &str = "[QuickActionShortcut::register]";

/// 当前已注册的加速键字符串（进程级单一事实来源）。re-register 据此反注册旧
/// 组合。`None` 表示尚未注册任何组合。
#[cfg(target_os = "macos")]
static CURRENT_ACCELERATOR: Mutex<Option<String>> = Mutex::new(None);

/// 把加速键字符串解析为插件的 [`Shortcut`]；解析失败返回结构化 [`AppError`]
/// 而非 panic（VAL-OVERLAY-016）。这是纯解析步骤，便于对错误分支做单元测试。
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

/// 注册（或重注册）Quick Action 全局快捷键。
///
/// 流程：解析新加速键 → 反注册先前记录的加速键（若有）→ 用切换 handler 注册新
/// 加速键 → 更新进程级记录。任一步失败都返回结构化 [`AppError`] 并以
/// `[QuickActionShortcut::register]` 前缀记录；调用方决定是否阻断（启动路径选择
/// 仅记录并继续，VAL-OVERLAY-016）。
///
/// handler 仅在 key-DOWN 触发：按下时若面板可见则隐藏、否则按当前鼠标位置显示
/// （VAL-OVERLAY-002 toggle 语义）。
#[cfg(target_os = "macos")]
pub fn register_shortcut(app: &AppHandle, accelerator: &str) -> Result<(), AppError> {
    let shortcut = parse_accelerator(accelerator)?;

    let gs = app.global_shortcut();

    // 先反注册上一个记录的加速键，使被替换的旧组合失活（设置页 live rebind 依赖）。
    let mut current = CURRENT_ACCELERATOR.lock().map_err(|_| {
        let message = "全局快捷键状态锁已损坏".to_string();
        tracing::error!("{LOG_PREFIX} {message}");
        AppError::internal_error(&message)
    })?;

    if let Some(previous) = current.take() {
        if previous != accelerator {
            if let Ok(prev_shortcut) = parse_accelerator(&previous) {
                if let Err(e) = gs.unregister(prev_shortcut) {
                    // 旧组合反注册失败不阻断新组合注册，仅记录。
                    tracing::warn!(
                        "{LOG_PREFIX} failed to unregister previous \"{previous}\": {e}"
                    );
                }
            }
        }
    }

    let handle = app.clone();
    gs.on_shortcut(shortcut, move |_app, _shortcut, event| {
        // 仅响应 key-DOWN，避免 down/up 触发两次（VAL-OVERLAY-020）。
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

/// 切换 Quick Action 浮层：可见则隐藏、隐藏则按当前鼠标位置显示。复用面板模块的
/// 进程级可见性标志与 show/hide，绝不在已可见时再次 show。
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

/// 非 macOS 平台的 no-op stub：全局快捷键能力仅在 macOS 接线（NSPanel 浮层
/// 同样仅 macOS）。
#[cfg(not(target_os = "macos"))]
pub fn register_shortcut(_app: &tauri::AppHandle, _accelerator: &str) -> Result<(), AppError> {
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    // VAL-OVERLAY-016：错误分支——无法解析的加速键串返回结构化 AppError 而非
    // panic。
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
