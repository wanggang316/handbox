//! "Open in ..." —— 把 Agent 会话的工作目录在外部 editor / terminal / 文件管理器中打开。
//!
//! 设计取向（对齐 codans 的 NSWorkspace 思路，落到 Tauri/Rust）：
//! - **探测交给后端**：扫描标准 app 目录，命中已安装的 `.app` 即视为可用 target
//!   （确定性、可单测、零子进程）。
//! - **启动交给后端 `open(1)`**：用进程直呼系统 launcher，绕开 opener/shell 插件的
//!   capability scope —— 工作目录是任意用户路径，opener 的 `$APPDATA/**` scope 不适用，
//!   而后端进程不受 capability 约束。
//! - 注册表 macOS 专属（`cfg` 门控）；其它平台仅给一个「文件管理器」target。

use crate::models::AppError;
use std::path::Path;

/// 一个可见于前端下拉的「打开目标」。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInTarget {
    /// 稳定 id，前端回灌给 `open_in_open` 以解析启动方式。
    pub id: String,
    /// 展示名（如 "Visual Studio Code"）。
    pub name: String,
    /// 分类，便于前端分组 / 选图标：`editor` / `terminal` / `system`。
    pub kind: String,
    /// 应用图标（`data:image/png;base64,...`）。取不到时为 `None`，前端回退到内置图标。
    pub icon: Option<String>,
}

/// Finder / 系统文件管理器这个「总是可用」的 target id。
const SYSTEM_TARGET_ID: &str = "system";

/// Finder 应用路径，用于给「系统」target 取真实图标（macOS）。
#[cfg(target_os = "macos")]
const FINDER_APP_PATH: &str = "/System/Library/CoreServices/Finder.app";

// ---------------------------------------------------------------------------
// macOS：注册表 + 探测
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
struct AppEntry {
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    /// `.app` bundle 名候选（含 `.app` 后缀），任一命中即视为已安装。
    bundles: &'static [&'static str],
}

/// 已知 editor / terminal 注册表（macOS）。新增编辑器只需在此追加一行。
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

/// 探测 `.app` 时扫描的标准目录（含 Apple 自带 app 与用户级安装）。
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

/// 在标准目录中解析某 entry 的 `.app` 绝对路径；任一 bundle 候选命中即返回。
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
    // Finder 总在首位（必装，作为兜底）。
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

/// 图标渲染像素尺寸（约 32pt 显示的 @2x），固定输出以约束 PNG 体积。
#[cfg(target_os = "macos")]
const ICON_RENDER_PX: isize = 64;

/// 取某个 `.app` 的图标并编码为 PNG data URI。
///
/// 走 AppKit `NSWorkspace::iconForFile`（而非读 `.icns`）—— 现代应用图标常存于
/// 编译后的 `Assets.car` 里，只有 Launch Services 解析得到的 `NSImage` 才完整。
/// 把 `NSImage` 重绘进一个固定 `ICON_RENDER_PX` 像素的离屏 bitmap 再编码 PNG：
/// 源图常含 512/1024px 表示，直接编码会得到 MB 级体积；定尺重绘把单图压到约几 KB。
/// 取不到返回 `None`。
#[cfg(target_os = "macos")]
#[allow(deprecated)] // iconForFile: 在新 SDK 标记弃用，但替代 API 需 UTType，成本更高。
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

        // 建一个 ICON_RENDER_PX² 的空 RGBA(8bit) 离屏 bitmap（planes=nil → 自动分配）。
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

        // 用该 bitmap 作后备建离屏图形上下文，把 image 画满整个矩形。
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

// ---------------------------------------------------------------------------
// 启动
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
async fn launch(dir: &Path, target_id: &str) -> Result<(), AppError> {
    use tokio::process::Command;

    let status = if target_id == SYSTEM_TARGET_ID {
        // `open <dir>` 在 Finder 中打开该文件夹窗口。
        Command::new("open").arg(dir).status().await
    } else {
        let entry = REGISTRY
            .iter()
            .find(|e| e.id == target_id)
            .ok_or_else(|| AppError::validation_error(&format!("未知打开目标: {target_id}")))?;
        let app = resolve_app_path(entry.bundles).ok_or_else(|| {
            AppError::validation_error(&format!("应用未安装: {}", entry.name))
        })?;
        // `open -a <app> <dir>`：用指定 app 打开工作目录。
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

    // Windows 的 `explorer` 即使成功也常返回非零退出码，故只判进程能否拉起。
    #[cfg(target_os = "windows")]
    let spawned = Command::new("explorer").arg(dir).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let spawned = Command::new("xdg-open").arg(dir).spawn();

    spawned
        .map(|_| ())
        .map_err(|e| AppError::internal_error(&format!("启动失败: {e}")))
}

// ---------------------------------------------------------------------------
// IPC 命令
// ---------------------------------------------------------------------------

/// 列出当前系统可用的「打开目标」（已安装的 editor / terminal + 系统文件管理器）。
#[tauri::command]
pub async fn open_in_list_targets() -> Result<Vec<OpenInTarget>, AppError> {
    Ok(list_targets())
}

/// 在指定 target 中打开目录 `path`（须为已存在目录）。`target_id` 来自
/// `open_in_list_targets` 返回项的 `id`；`"system"` 走系统文件管理器。
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

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

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
        // 不强求一定取到（无窗口服务器的极端环境可能取不到），但取到时必须是
        // 合法的 PNG data URI——以此守住 FFI / 编码路径不回归。
        if let Some(uri) = app_icon_data_uri(Path::new(FINDER_APP_PATH)) {
            assert!(uri.starts_with("data:image/png;base64,"));
            assert!(uri.len() > "data:image/png;base64,".len() + 16);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn list_targets_attaches_finder_icon() {
        // Finder 必装，其图标在常规 macOS 上应可取到。
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
