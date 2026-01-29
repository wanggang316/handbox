#[derive(Debug, Clone)]
pub struct FrontmostAppInfo {
    pub name: String,
    pub bundle_id: String,
    pub pid: i32,
}

#[cfg(target_os = "macos")]
pub fn get_frontmost_app_info() -> Option<FrontmostAppInfo> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;

    let name = app
        .localizedName()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let bundle_id = app
        .bundleIdentifier()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown.app".to_string());
    let pid = app.processIdentifier() as i32;

    Some(FrontmostAppInfo {
        name,
        bundle_id,
        pid,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_app_info() -> Option<FrontmostAppInfo> {
    None
}
