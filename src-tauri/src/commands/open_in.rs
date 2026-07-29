//! "Open in ...": open an agent session's working directory in an external
//! editor / terminal / file manager.
//!
//! - Probing happens in the backend: scan the standard app dirs; an installed
//!   `.app` is an available target (deterministic, unit-testable, no subprocess).
//! - Launching goes through the backend via `open(1)`, bypassing the
//!   opener/shell plugins' capability scope — the working dir is an arbitrary
//!   user path, which the opener's `$APPDATA/**` scope does not cover, while a
//!   backend process is not capability-constrained.
//! - The registry is macOS-only (`cfg`-gated); other platforms get a single
//!   "file manager" target.

use crate::models::AppError;
use std::path::Path;

/// An "open in" target shown in the frontend dropdown.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInTarget {
    /// Stable id the frontend passes back to `open_in_open`.
    pub id: String,
    /// Display name (e.g. "Visual Studio Code").
    pub name: String,
    /// Category for grouping / icon selection: `editor` / `terminal` / `system`.
    pub kind: String,
    /// App icon as a `data:image/png;base64,...` URI; `None` makes the frontend
    /// fall back to its built-in icon.
    pub icon: Option<String>,
}

/// Id of the always-available Finder / system file manager target.
const SYSTEM_TARGET_ID: &str = "system";

/// Finder app path, used to fetch the real icon for the system target.
#[cfg(target_os = "macos")]
const FINDER_APP_PATH: &str = "/System/Library/CoreServices/Finder.app";

#[cfg(target_os = "macos")]
struct AppEntry {
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    /// Candidate `.app` bundle names (with suffix); any hit counts as installed.
    bundles: &'static [&'static str],
}

/// Known editor / terminal registry (macOS); adding an editor is one new entry.
#[cfg(target_os = "macos")]
const REGISTRY: &[AppEntry] = &[
    // Editors
    AppEntry {
        id: "vscode",
        name: "Visual Studio Code",
        kind: "editor",
        bundles: &["Visual Studio Code.app"],
    },
    AppEntry {
        id: "cursor",
        name: "Cursor",
        kind: "editor",
        bundles: &["Cursor.app"],
    },
    AppEntry {
        id: "zed",
        name: "Zed",
        kind: "editor",
        bundles: &["Zed.app"],
    },
    AppEntry {
        id: "windsurf",
        name: "Windsurf",
        kind: "editor",
        bundles: &["Windsurf.app"],
    },
    AppEntry {
        id: "sublime",
        name: "Sublime Text",
        kind: "editor",
        bundles: &["Sublime Text.app"],
    },
    AppEntry {
        id: "vscodium",
        name: "VSCodium",
        kind: "editor",
        bundles: &["VSCodium.app"],
    },
    AppEntry {
        id: "intellij",
        name: "IntelliJ IDEA",
        kind: "editor",
        bundles: &["IntelliJ IDEA.app", "IntelliJ IDEA CE.app"],
    },
    AppEntry {
        id: "webstorm",
        name: "WebStorm",
        kind: "editor",
        bundles: &["WebStorm.app"],
    },
    AppEntry {
        id: "pycharm",
        name: "PyCharm",
        kind: "editor",
        bundles: &["PyCharm.app", "PyCharm CE.app"],
    },
    AppEntry {
        id: "goland",
        name: "GoLand",
        kind: "editor",
        bundles: &["GoLand.app"],
    },
    AppEntry {
        id: "rustrover",
        name: "RustRover",
        kind: "editor",
        bundles: &["RustRover.app"],
    },
    AppEntry {
        id: "nova",
        name: "Nova",
        kind: "editor",
        bundles: &["Nova.app"],
    },
    AppEntry {
        id: "xcode",
        name: "Xcode",
        kind: "editor",
        bundles: &["Xcode.app"],
    },
    // Terminals
    AppEntry {
        id: "iterm",
        name: "iTerm",
        kind: "terminal",
        bundles: &["iTerm.app"],
    },
    AppEntry {
        id: "ghostty",
        name: "Ghostty",
        kind: "terminal",
        bundles: &["Ghostty.app"],
    },
    AppEntry {
        id: "warp",
        name: "Warp",
        kind: "terminal",
        bundles: &["Warp.app"],
    },
    AppEntry {
        id: "wezterm",
        name: "WezTerm",
        kind: "terminal",
        bundles: &["WezTerm.app"],
    },
    AppEntry {
        id: "kitty",
        name: "kitty",
        kind: "terminal",
        bundles: &["kitty.app"],
    },
    AppEntry {
        id: "alacritty",
        name: "Alacritty",
        kind: "terminal",
        bundles: &["Alacritty.app"],
    },
    AppEntry {
        id: "terminal",
        name: "Terminal",
        kind: "terminal",
        bundles: &["Terminal.app"],
    },
];

/// Standard directories scanned for `.app` bundles (incl. Apple-shipped apps
/// and per-user installs).
#[cfg(target_os = "macos")]
fn app_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![
        std::path::PathBuf::from("/Applications"),
        std::path::PathBuf::from("/Applications/Utilities"),
        std::path::PathBuf::from("/System/Applications"),
        std::path::PathBuf::from("/System/Applications/Utilities"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(Path::new(&home).join("Applications"));
    }
    dirs
}

/// Resolves an entry's `.app` absolute path in the standard directories;
/// returns on the first bundle-candidate hit.
#[cfg(target_os = "macos")]
fn resolve_app_path(bundles: &[&str]) -> Option<std::path::PathBuf> {
    for dir in app_search_dirs() {
        for bundle in bundles {
            let candidate = dir.join(bundle);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn list_targets() -> Vec<OpenInTarget> {
    // Finder always comes first (always installed; the fallback).
    let mut out = vec![OpenInTarget {
        id: SYSTEM_TARGET_ID.to_string(),
        name: "Finder".to_string(),
        kind: "system".to_string(),
        icon: app_icon_data_uri(Path::new(FINDER_APP_PATH)),
    }];
    for entry in REGISTRY {
        if let Some(app_path) = resolve_app_path(entry.bundles) {
            out.push(OpenInTarget {
                id: entry.id.to_string(),
                name: entry.name.to_string(),
                kind: entry.kind.to_string(),
                icon: app_icon_data_uri(&app_path),
            });
        }
    }
    out
}

/// Icon render size in pixels (@2x for a ~32pt display); fixed to bound PNG size.
#[cfg(target_os = "macos")]
const ICON_RENDER_PX: isize = 64;

/// Fetches an `.app` icon and encodes it as a PNG data URI.
///
/// Uses AppKit `NSWorkspace::iconForFile` rather than reading `.icns`: modern
/// app icons often live in a compiled `Assets.car`, and only the Launch
/// Services-resolved `NSImage` is complete. The `NSImage` is redrawn into a
/// fixed `ICON_RENDER_PX` offscreen bitmap before PNG-encoding: sources often
/// carry 512/1024px representations that would encode to MB-scale output, while
/// the fixed-size redraw keeps each icon at a few KB. Returns `None` on failure.
#[cfg(target_os = "macos")]
#[allow(deprecated)] // iconForFile: is deprecated in newer SDKs, but the replacement needs UTType and costs more.
fn app_icon_data_uri(app_path: &Path) -> Option<String> {
    use base64::Engine;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::AnyObject;
    use objc2::AllocAnyThread;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSColorSpaceName, NSCompositingOperation,
        NSDeviceRGBColorSpace, NSGraphicsContext, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    let path_str = app_path.to_str()?;
    autoreleasepool(|_| {
        let workspace = NSWorkspace::sharedWorkspace();
        let ns_path = NSString::from_str(path_str);
        let image = workspace.iconForFile(&ns_path);

        // Empty ICON_RENDER_PX² RGBA(8-bit) offscreen bitmap (planes = null →
        // auto-allocated).
        let color_space: &NSColorSpaceName = unsafe { NSDeviceRGBColorSpace };
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                ICON_RENDER_PX,
                ICON_RENDER_PX,
                8,
                4,
                true,
                false,
                color_space,
                0,
                0,
            )
        }?;

        // Offscreen graphics context backed by the bitmap; draw the image over
        // the full rect.
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let target = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(ICON_RENDER_PX as f64, ICON_RENDER_PX as f64),
        );
        let zero = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        image.drawInRect_fromRect_operation_fraction(
            target,
            zero,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();

        let empty = NSDictionary::<NSString, AnyObject>::new();
        let png = unsafe {
            rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &empty)
        }?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(png.to_vec());
        Some(format!("data:image/png;base64,{encoded}"))
    })
}

#[cfg(not(target_os = "macos"))]
fn list_targets() -> Vec<OpenInTarget> {
    vec![OpenInTarget {
        id: SYSTEM_TARGET_ID.to_string(),
        name: "File Manager".to_string(),
        kind: "system".to_string(),
        icon: None,
    }]
}

#[cfg(target_os = "macos")]
async fn launch(dir: &Path, target_id: &str) -> Result<(), AppError> {
    use tokio::process::Command;

    let status = if target_id == SYSTEM_TARGET_ID {
        // `open <dir>` opens the folder in a Finder window.
        Command::new("open").arg(dir).status().await
    } else {
        let entry = REGISTRY
            .iter()
            .find(|e| e.id == target_id)
            .ok_or_else(|| AppError::validation_error(&format!("未知打开目标: {target_id}")))?;
        let app = resolve_app_path(entry.bundles).ok_or_else(|| {
            AppError::validation_error(&format!("应用未安装: {}", entry.name))
        })?;
        // `open -a <app> <dir>`: open the working dir with the given app.
        Command::new("open")
            .arg("-a")
            .arg(&app)
            .arg(dir)
            .status()
            .await
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(AppError::internal_error(&format!(
            "open 退出码非零: {s}"
        ))),
        Err(e) => Err(AppError::internal_error(&format!("启动失败: {e}"))),
    }
}

#[cfg(not(target_os = "macos"))]
async fn launch(dir: &Path, _target_id: &str) -> Result<(), AppError> {
    use tokio::process::Command;

    // Windows `explorer` often exits non-zero even on success, so only check
    // that the process spawns.
    #[cfg(target_os = "windows")]
    let spawned = Command::new("explorer").arg(dir).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let spawned = Command::new("xdg-open").arg(dir).spawn();

    spawned
        .map(|_| ())
        .map_err(|e| AppError::internal_error(&format!("启动失败: {e}")))
}

/// Lists the available open targets: installed editors / terminals plus the
/// system file manager.
#[tauri::command]
pub async fn open_in_list_targets() -> Result<Vec<OpenInTarget>, AppError> {
    Ok(list_targets())
}

/// Opens directory `path` (must be an existing directory) in the given target.
/// `target_id` comes from `open_in_list_targets`; `"system"` uses the system
/// file manager.
#[tauri::command]
pub async fn open_in_open(path: String, target_id: String) -> Result<(), AppError> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(AppError::validation_error(&format!(
            "工作目录不存在或不是目录: {path}"
        )));
    }
    launch(dir, &target_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_targets_puts_system_first() {
        let targets = list_targets();
        assert!(!targets.is_empty());
        assert_eq!(targets[0].id, SYSTEM_TARGET_ID);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registry_ids_are_unique() {
        let mut ids: Vec<&str> = REGISTRY.iter().map(|e| e.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "REGISTRY 中存在重复 id");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn registry_ids_never_collide_with_system() {
        assert!(
            REGISTRY.iter().all(|e| e.id != SYSTEM_TARGET_ID),
            "editor/terminal id 不得与系统 target id 冲突"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn resolve_app_path_returns_none_for_bogus_bundle() {
        assert!(resolve_app_path(&["__definitely_not_an_app__.app"]).is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_icon_data_uri_is_png_data_uri_when_available() {
        // The icon may be unavailable (e.g. no window server), but when present
        // it must be a valid PNG data URI — guarding the FFI / encoding path.
        if let Some(uri) = app_icon_data_uri(Path::new(FINDER_APP_PATH)) {
            assert!(uri.starts_with("data:image/png;base64,"));
            assert!(uri.len() > "data:image/png;base64,".len() + 16);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn list_targets_attaches_finder_icon() {
        // Finder is always installed; its icon should resolve on normal macOS.
        let targets = list_targets();
        let finder = targets
            .iter()
            .find(|t| t.id == SYSTEM_TARGET_ID)
            .expect("Finder target present");
        if let Some(icon) = &finder.icon {
            assert!(icon.starts_with("data:image/png;base64,"));
        }
    }




    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn open_in_open_rejects_nonexistent_dir() {
        let err = open_in_open(
            "/tmp/__handbox_openin_missing__/nope".to_string(),
            SYSTEM_TARGET_ID.to_string(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
}
