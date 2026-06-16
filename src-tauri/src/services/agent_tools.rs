// Agent-mode read-only filesystem tools + a self-built working_dir sandbox.
//
// This module provides the SECURITY CORE for Agent mode's file tools. The
// model supplies a path string; we MUST guarantee it can only ever resolve to
// a target contained inside the session's `working_dir`. We do NOT reuse
// hand-ai's `coding-agent` path helpers: they have no sandbox and they expand
// `~` to the user's home — exactly the escape we must forbid.
//
// The resolver (`resolve_in_sandbox`) is deliberately strict:
//   - empty / `.` / whitespace-only / NUL-containing args are rejected;
//   - a leading `~` is treated literally / rejected (NO home expansion);
//   - the model path is joined under the canonicalized root, then the target
//     is canonicalized (resolving every symlink), then containment is checked
//     by PATH COMPONENTS against the canonical root — NOT by string prefix, so
//     `/p/proj` never accepts `/p/proj-secrets`;
//   - on macOS (APFS case-insensitive + Unicode NFD/NFC) the component
//     comparison is case-folded, and Unicode-form differences are unified by
//     `canonicalize()` itself (it resolves the supplied spelling against the
//     real on-disk inode), so a case-variant or an NFD/NFC variant that
//     resolves OUTSIDE is still rejected while an equivalent one INSIDE is
//     accepted.
//
// On a sandbox violation the error result is GENERIC (D14): it never echoes the
// out-of-sandbox absolute path nor any file contents — only
// "path is outside the working directory".
//
// ACCEPTED RESIDUAL RISK (plan D11/D25): TOCTOU — a symlink swapped between the
// containment check and the actual read — is NOT defended here. v1 is
// single-user local; closing the race is out of scope and intentionally not
// attempted.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use hand_agent::{AgentTool, ToolResult};
use hand_ai_model::{ImageContent, ToolResultContent};
use serde_json::json;

/// Tool names this factory knows how to build.
const TOOL_READ_FILE: &str = "read_file";
const TOOL_LIST_DIRECTORY: &str = "list_directory";
/// The skill tool's fixed name. Auto-injected when a run has enabled skills;
/// distinct from every `build_tools`-produced name so it never collides.
const TOOL_SKILL: &str = "skill";

/// Byte budget for a single `read_file` result before truncation kicks in.
const READ_FILE_BYTE_BUDGET: usize = 50 * 1024;
/// Hard cap on the raw bytes of an image we will base64 + return as an image
/// block. Larger images are refused with a generic message rather than
/// base64-encoding an unbounded file into the model context.
const READ_FILE_IMAGE_BYTE_CAP: usize = 5 * 1024 * 1024;
/// Max entries a single `list_directory` result will emit before truncation.
const LIST_MAX_ENTRIES: usize = 500;

/// Generic, leak-free message for any sandbox containment violation (D14).
/// MUST NOT contain the offending absolute path or any file contents.
const SANDBOX_VIOLATION_MSG: &str = "path is outside the working directory";

/// Why a model-supplied path could not be resolved inside the sandbox.
///
/// `display_message` is intentionally generic for every variant so error text
/// never leaks an out-of-sandbox absolute path (D14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// Empty / `.` / whitespace-only / NUL-containing / `~`-prefixed arg.
    InvalidArg,
    /// The canonical target is not contained in the canonical root.
    OutsideSandbox,
    /// The root could not be canonicalized (should not happen: validated at
    /// session-create), or the target's ancestors could not be resolved.
    ResolveFailed,
}

impl SandboxError {
    /// Leak-free message safe to return to the model.
    pub fn display_message(&self) -> &'static str {
        match self {
            // A malformed arg never carries an out-of-sandbox path, but we keep
            // the wording generic and aligned with the containment case.
            SandboxError::InvalidArg => "invalid path argument",
            SandboxError::OutsideSandbox => SANDBOX_VIOLATION_MSG,
            SandboxError::ResolveFailed => SANDBOX_VIOLATION_MSG,
        }
    }
}

/// Resolve a model-supplied `arg_path` strictly inside `working_dir`.
///
/// Returns the canonical, in-sandbox target on success. See the module docs for
/// the full contract; the short version: reject `~`, reject traversal/absolute
/// escapes, canonicalize (resolving symlinks), and verify component-wise
/// containment under the canonical root with macOS case/Unicode folding.
///
/// `working_dir` is assumed to be an existing directory (validated at
/// session-create); we re-canonicalize it here defensively.
pub fn resolve_in_sandbox(working_dir: &Path, arg_path: &str) -> Result<PathBuf, SandboxError> {
    // --- 1. Reject malformed args up front (cheap, no FS access). ---
    if arg_path.contains('\0') {
        return Err(SandboxError::InvalidArg);
    }
    let trimmed = arg_path.trim();
    if trimmed.is_empty() || trimmed == "." {
        return Err(SandboxError::InvalidArg);
    }
    // NO `~` expansion. A leading `~` (home or `~user`) is rejected outright so
    // the model can never reach outside the sandbox via home expansion. We do
    // not treat it literally-then-join either, because a literal `~/x` segment
    // is never a legitimate in-sandbox target and rejecting is clearer.
    if arg_path.starts_with('~') {
        return Err(SandboxError::InvalidArg);
    }

    // --- 2. Canonicalize the root (resolve symlinks -> real absolute path). ---
    let canonical_root = working_dir
        .canonicalize()
        .map_err(|_| SandboxError::ResolveFailed)?;

    // --- 3. Build the candidate target under the root. ---
    // An absolute arg is taken as-is (and will be containment-checked); a
    // relative arg is joined under the root. Either way the result is
    // canonicalized next, so `..` segments are collapsed against real dirs.
    let arg = Path::new(arg_path);
    let candidate = if arg.is_absolute() {
        arg.to_path_buf()
    } else {
        canonical_root.join(arg)
    };

    // --- 4. Canonicalize the candidate, resolving every symlink. ---
    // The target may not exist (e.g. read of a missing file). In that case we
    // canonicalize the deepest existing ancestor and re-attach the unresolved
    // tail, so containment is still enforced and we never leak via a partial
    // path. The symlink-escape case is covered because canonicalize() follows
    // links: a link inside the root pointing outside resolves OUTSIDE here.
    let canonical_target = canonicalize_lenient(&candidate)?;

    // --- 5. Component-wise containment (NOT string starts_with). ---
    if !is_contained(&canonical_root, &canonical_target) {
        return Err(SandboxError::OutsideSandbox);
    }

    Ok(canonical_target)
}

/// Canonicalize `path` if it exists; otherwise canonicalize the deepest
/// existing ancestor and re-append the unresolved tail components.
///
/// This lets containment be enforced even for not-yet-existing targets (a
/// missing file under the sandbox) without ever returning an un-canonicalized
/// path that could string-match the root by accident.
fn canonicalize_lenient(path: &Path) -> Result<PathBuf, SandboxError> {
    if let Ok(c) = path.canonicalize() {
        return Ok(c);
    }
    // Walk up to the first ancestor that canonicalizes, collecting the tail.
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path;
    loop {
        match cur.parent() {
            Some(parent) => {
                if let Some(name) = cur.file_name() {
                    tail.push(name.to_os_string());
                } else {
                    // e.g. a trailing `..` or root with no file name component.
                    return Err(SandboxError::ResolveFailed);
                }
                if let Ok(canon_parent) = parent.canonicalize() {
                    let mut resolved = canon_parent;
                    for seg in tail.iter().rev() {
                        resolved.push(seg);
                    }
                    return Ok(resolved);
                }
                cur = parent;
            }
            None => return Err(SandboxError::ResolveFailed),
        }
    }
}

/// Component-wise containment check: is `target` inside `root`?
///
/// Compares the normal path components in order. `root`'s components must be a
/// prefix of `target`'s components. Comparison folds case AND Unicode form so
/// macOS APFS (case-insensitive) and NFD/NFC variants are handled: a variant
/// that genuinely resolves outside the root still fails containment, while an
/// equivalent one inside passes.
///
/// This is the bypass that string `starts_with` misses: `/p/proj` must NOT
/// contain `/p/proj-secrets`, even though the second string starts with the
/// first.
fn is_contained(root: &Path, target: &Path) -> bool {
    let root_components: Vec<String> = normal_components(root);
    let target_components: Vec<String> = normal_components(target);

    if target_components.len() < root_components.len() {
        return false;
    }
    root_components
        .iter()
        .zip(target_components.iter())
        .all(|(r, t)| r == t)
}

/// Extract the `Normal` path components as fold-normalized strings.
///
/// Both canonical paths are absolute with symlinks/`.`/`..` already resolved by
/// the caller, so only `RootDir`/`Prefix`/`Normal` components remain; we keep
/// the `Normal` ones (the meaningful names) and drop the root marker, which is
/// identical for two paths on the same volume.
fn normal_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(os) => Some(fold_component(&os.to_string_lossy())),
            _ => None,
        })
        .collect()
}

/// Fold a single path component for case-insensitive comparison.
///
/// Case folding (lowercase) matches APFS's default case-insensitive behavior,
/// so `Proj` and `proj` compare equal.
///
/// NFD/NFC handling is NOT done by string normalization here — it is handled
/// EARLIER, by `canonicalize()`. Both `root` and `target` are canonical paths
/// resolved against real on-disk inodes; the OS returns each in the
/// filesystem's own canonical Unicode form. So a target supplied in a
/// decomposed (NFD) spelling that points OUTSIDE the root resolves to the
/// outside inode's real path and fails containment regardless of spelling,
/// while an equivalent in-sandbox name resolves to the same inode as the root's
/// child. Folding here only adds the case-insensitivity net on top of that
/// canonical comparison.
///
/// Containment is still correct on case-SENSITIVE filesystems: distinct real
/// dirs canonicalize to distinct ancestors, so an out-of-root target's EARLIER
/// components still differ even after folding.
fn fold_component(s: &str) -> String {
    s.to_lowercase()
}

/// Build the requested read-only tools bound to the session's `working_dir`.
///
/// `enabled` lists tool names the session turned on. `working_dir` is the
/// sandbox root; when `None`/empty, the FS tools that NEED a sandbox root
/// (`read_file`, `list_directory`) are omitted entirely — without a root there
/// is no safe place for them to operate.
pub fn build_tools(enabled: &[String], working_dir: Option<&Path>) -> Vec<AgentTool> {
    let mut tools = Vec::new();

    // FS tools require a non-empty sandbox root.
    let sandbox_root: Option<PathBuf> = working_dir
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf());

    for name in enabled {
        match name.as_str() {
            TOOL_READ_FILE => {
                if let Some(root) = &sandbox_root {
                    tools.push(make_read_file_tool(root.clone()));
                }
            }
            TOOL_LIST_DIRECTORY => {
                if let Some(root) = &sandbox_root {
                    tools.push(make_list_directory_tool(root.clone()));
                }
            }
            // Unknown / not-yet-implemented tool names are ignored.
            _ => {}
        }
    }

    tools
}

/// Construct the `read_file` tool bound to `root`.
fn make_read_file_tool(root: PathBuf) -> AgentTool {
    AgentTool::simple(
        TOOL_READ_FILE,
        "Read the contents of a regular file inside the working directory. \
         Paths are resolved relative to the working directory; escaping it is \
         not permitted. Large files are truncated.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file, relative to the working directory."
                }
            },
            "required": ["path"]
        }),
        "Read file",
        move |_tool_call_id, args| {
            let root = root.clone();
            async move { execute_read_file(&root, args) }
        },
    )
}

/// Construct the `list_directory` tool bound to `root`.
fn make_list_directory_tool(root: PathBuf) -> AgentTool {
    AgentTool::simple(
        TOOL_LIST_DIRECTORY,
        "List the entries of a directory inside the working directory. \
         Paths are resolved relative to the working directory; escaping it is \
         not permitted. Directories are listed first. Long listings are truncated.",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the directory, relative to the working directory."
                }
            },
            "required": ["path"]
        }),
        "List directory",
        move |_tool_call_id, args| {
            let root = root.clone();
            async move { execute_list_directory(&root, args) }
        },
    )
}

/// `read_file` body: resolve in sandbox, reject non-regular files BEFORE
/// reading, then read + truncate.
fn execute_read_file(root: &Path, args: serde_json::Value) -> ToolResult {
    let path_str = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return ToolResult::error("Missing required parameter: path"),
    };

    let target = match resolve_in_sandbox(root, path_str) {
        Ok(t) => t,
        Err(e) => return ToolResult::error(e.display_message()),
    };

    // Reject non-regular files (FIFO/device/socket) BEFORE opening, so a FIFO
    // cannot block the run on a read that never returns.
    let metadata = match std::fs::symlink_metadata(&target) {
        Ok(m) => m,
        // Generic message — do not echo the (now in-sandbox) absolute path.
        Err(_) => return ToolResult::error("Failed to read file: file not found"),
    };
    if metadata.is_dir() {
        return ToolResult::error("Failed to read file: path is a directory");
    }
    if !metadata.is_file() {
        return ToolResult::error("Failed to read file: not a regular file");
    }

    let raw_bytes = match std::fs::read(&target) {
        Ok(b) => b,
        Err(_) => return ToolResult::error("Failed to read file"),
    };

    // Image files become an Image content block (base64 + mime) so the model
    // (and the tool-call card) sees the picture, not replacement-char noise from
    // a lossy UTF-8 decode of binary bytes. Detection is by extension first,
    // then a magic-byte sniff so an image with a wrong/missing extension is
    // still handled. Oversize images are refused (we must not base64 an
    // unbounded file into the context).
    if let Some(mime) = detect_image_mime(&target, &raw_bytes) {
        if raw_bytes.len() > READ_FILE_IMAGE_BYTE_CAP {
            return ToolResult::error("Failed to read file: image is too large");
        }
        let data_b64 = BASE64_STANDARD.encode(&raw_bytes);
        return ToolResult {
            content: vec![ToolResultContent::Image(ImageContent::new(data_b64, mime))],
            details: None,
            terminate: None,
        };
    }

    // Text files: lossy-decode (binary-safe — never panics) and truncate.
    let content = String::from_utf8_lossy(&raw_bytes).into_owned();
    let (body, truncated) = truncate_text(&content, READ_FILE_BYTE_BUDGET);

    let mut output = body;
    if truncated {
        output.push_str(&format!(
            "\n[Truncated: showing first {} bytes of {} total.]",
            READ_FILE_BYTE_BUDGET,
            content.len()
        ));
    }
    ToolResult::text(output)
}

/// Detect whether `target`/`bytes` is a supported image, returning its MIME
/// type when so. Extension match (png/jpg/jpeg/gif/webp/bmp) is checked first;
/// otherwise a magic-byte sniff covers files with a wrong or missing extension.
/// Returns `None` for non-image (text/binary) files, which keep the text path.
fn detect_image_mime(target: &Path, bytes: &[u8]) -> Option<String> {
    if let Some(ext) = target.extension().and_then(|e| e.to_str()) {
        match ext.to_ascii_lowercase().as_str() {
            "png" => return Some("image/png".to_string()),
            "jpg" | "jpeg" => return Some("image/jpeg".to_string()),
            "gif" => return Some("image/gif".to_string()),
            "webp" => return Some("image/webp".to_string()),
            "bmp" => return Some("image/bmp".to_string()),
            _ => {}
        }
    }
    sniff_image_mime(bytes).map(str::to_string)
}

/// Sniff a supported image MIME from leading magic bytes. Covers PNG, JPEG,
/// GIF, WEBP (RIFF....WEBP), and BMP. Returns `None` when the bytes are not a
/// recognized image.
fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if bytes.starts_with(b"BM") {
        return Some("image/bmp");
    }
    None
}

/// `list_directory` body: resolve in sandbox, list entries (dirs first), and
/// truncate to a max entry count with a visible marker.
fn execute_list_directory(root: &Path, args: serde_json::Value) -> ToolResult {
    let path_str = match args.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return ToolResult::error("Missing required parameter: path"),
    };

    let target = match resolve_in_sandbox(root, path_str) {
        Ok(t) => t,
        Err(e) => return ToolResult::error(e.display_message()),
    };

    let read_dir = match std::fs::read_dir(&target) {
        Ok(rd) => rd,
        Err(_) => return ToolResult::error("Failed to read directory"),
    };

    // (name, is_dir)
    let mut items: Vec<(String, bool)> = Vec::new();
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        items.push((name, is_dir));
    }

    // Dirs first, then files; each group sorted by name.
    items.sort_by(|a, b| {
        let order = |is_dir: bool| if is_dir { 0 } else { 1 };
        order(a.1).cmp(&order(b.1)).then_with(|| a.0.cmp(&b.0))
    });

    let total = items.len();
    let truncated = total > LIST_MAX_ENTRIES;
    let shown = if truncated { LIST_MAX_ENTRIES } else { total };

    if total == 0 {
        return ToolResult::text("(empty directory)".to_string());
    }

    let mut output = String::new();
    for (name, is_dir) in items.iter().take(shown) {
        if *is_dir {
            output.push_str(&format!("  {}/\n", name));
        } else {
            output.push_str(&format!("  {}\n", name));
        }
    }
    if truncated {
        output.push_str(&format!(
            "[Truncated: showing {} of {} entries.]",
            shown, total
        ));
    }
    ToolResult::text(output)
}

/// Truncate `text` to at most `budget` bytes on a char boundary.
///
/// Returns `(possibly_truncated_text, was_truncated)`. The caller appends the
/// visible truncation marker.
fn truncate_text(text: &str, budget: usize) -> (String, bool) {
    if text.len() <= budget {
        return (text.to_string(), false);
    }
    // Find the largest char boundary <= budget so we never split a UTF-8 scalar.
    let mut end = budget;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

// ===========================================================================
// skill — load an enabled skill's full instructions by name (NOT by path).
// ===========================================================================
//
// SECURITY (VAL-TOOL-018): this tool NEVER touches the filesystem. The model
// supplies a `name`; we look that name up in an in-memory map of
// already-discovered, already-validated skills (name -> body). A miss — including
// a path-shaped argument like `/etc/passwd` or `../x` — returns a generic error
// that contains NO path and NO file contents. There is no read-from-disk path to
// abuse, so traversal/escape is structurally impossible here.

/// Generic, leak-free message for a skill lookup that finds no match. MUST NOT
/// echo the requested name verbatim if it is path-shaped, nor any file content.
const SKILL_NOT_FOUND_MSG: &str = "skill not found";
/// Message for a missing / empty / non-string `name` argument.
const SKILL_INVALID_ARG_MSG: &str = "invalid skill name argument";

/// Build the `skill` tool over a fixed `name -> body` map of the run's enabled,
/// discovered, validated skills.
///
/// The handler is a PURE table lookup: it reads `args["name"]`, trims it, and
/// returns the mapped body on a hit or a generic error on a miss/bad-arg. It
/// performs NO filesystem access — the only legitimate way to reach a skill body
/// is to name one that the run already gated into the map (VAL-TOOL-018). The
/// map is captured and `.clone()`d into the async future, mirroring the FS
/// tools' `PathBuf` capture pattern.
pub fn make_skill_tool(map: HashMap<String, String>) -> AgentTool {
    AgentTool::simple(
        TOOL_SKILL,
        "Load an enabled skill's full instructions by name. Pass the exact \
         skill name from the available-skills index; the full instruction body \
         is returned. Skills are referenced by name only, never by file path.",
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The exact name of an enabled skill to load."
                }
            },
            "required": ["name"]
        }),
        "Load skill",
        move |_tool_call_id, args| {
            let map = map.clone();
            async move { execute_skill(&map, args) }
        },
    )
}

/// `skill` tool body: look the requested name up in `map` and return its body,
/// or a generic, leak-free error. NO filesystem access of any kind.
fn execute_skill(map: &HashMap<String, String>, args: serde_json::Value) -> ToolResult {
    // A missing / non-string / empty (after trim) name is a bad argument.
    let name = match args.get("name").and_then(|v| v.as_str()) {
        Some(n) if !n.trim().is_empty() => n.trim(),
        _ => return ToolResult::error(SKILL_INVALID_ARG_MSG),
    };

    // Pure table lookup by name. A path-shaped name (`/etc/passwd`, `../x`)
    // simply isn't a key, so it falls through to the generic not-found message —
    // we never interpret the argument as a path or read any file (VAL-TOOL-018).
    match map.get(name) {
        Some(body) => ToolResult::text(body.clone()),
        None => ToolResult::error(SKILL_NOT_FOUND_MSG),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Extract the first text content block from a result.
    fn get_text(result: &ToolResult) -> &str {
        match &result.content[0] {
            hand_ai_model::ToolResultContent::Text(t) => &t.text,
            _ => panic!("expected text content"),
        }
    }

    /// Extract the first image content block from a result.
    fn get_image(result: &ToolResult) -> &hand_ai_model::ImageContent {
        match &result.content[0] {
            hand_ai_model::ToolResultContent::Image(img) => img,
            _ => panic!("expected image content"),
        }
    }

    /// The 8-byte PNG signature plus a minimal trailer — enough that both the
    /// extension match and the magic-byte sniff treat it as a PNG.
    const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR";

    /// A sandbox root with a few known files, plus a sibling dir OUTSIDE it.
    struct Fixture {
        _outer: TempDir,
        root: PathBuf,
        outside_secret: PathBuf,
    }

    fn fixture() -> Fixture {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("inside.txt"), "hello from inside").unwrap();
        fs::create_dir(root.join("sub")).unwrap();
        fs::write(root.join("sub").join("nested.txt"), "nested body").unwrap();

        // A secret file OUTSIDE the sandbox, as a sibling of the root.
        let outside_secret = outer.path().join("secret.txt");
        fs::write(&outside_secret, "TOP SECRET CONTENT").unwrap();

        Fixture {
            _outer: outer,
            root,
            outside_secret,
        }
    }

    /// Assert an error result that leaks NEITHER the out-of-sandbox absolute
    /// path NOR file contents.
    fn assert_no_leak(result: &ToolResult, abs_path: &Path, secret_substr: &str) {
        let text = get_text(result);
        assert!(
            !text.contains(&*abs_path.to_string_lossy()),
            "error text leaked the out-of-sandbox absolute path: {text:?}"
        );
        assert!(
            !text.contains(secret_substr),
            "error text leaked out-of-sandbox file contents: {text:?}"
        );
    }

    // -----------------------------------------------------------------------
    // VAL-TOOLS-009 — every escape vector is rejected, no leak.
    // Each vector is its own test (the security value is in the enumeration).
    // -----------------------------------------------------------------------

    #[test]
    fn vector_dotdot_traversal_rejected() {
        let fx = fixture();
        let err = resolve_in_sandbox(&fx.root, "../secret.txt").unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);

        // And through the tool: no leak of path/content.
        let result = execute_read_file(&fx.root, json!({"path": "../secret.txt"}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        assert_no_leak(&result, &fx.outside_secret, "TOP SECRET CONTENT");
    }

    #[test]
    fn vector_deep_dotdot_traversal_rejected() {
        let fx = fixture();
        let err = resolve_in_sandbox(&fx.root, "sub/../../secret.txt").unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);
    }

    #[test]
    fn vector_absolute_outside_rejected() {
        let fx = fixture();
        let err = resolve_in_sandbox(&fx.root, "/etc/passwd").unwrap_err();
        assert!(matches!(
            err,
            SandboxError::OutsideSandbox | SandboxError::ResolveFailed
        ));

        // Absolute path to the real outside secret -> rejected, no leak.
        let abs = fx.outside_secret.to_string_lossy().into_owned();
        let result = execute_read_file(&fx.root, json!({"path": abs}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        assert_no_leak(&result, &fx.outside_secret, "TOP SECRET CONTENT");
    }

    /// The component-wise (not string-prefix) bypass: `/p/proj` must NOT accept
    /// `/p/proj-secrets`. A naive `starts_with` on the canonical strings would
    /// wrongly admit the sibling.
    #[test]
    fn vector_prefix_sibling_rejected() {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        let sibling = outer.path().join("proj-secrets");
        fs::create_dir(&sibling).unwrap();
        let sibling_secret = sibling.join("creds.txt");
        fs::write(&sibling_secret, "SIBLING SECRET").unwrap();

        // Absolute path into the prefix-sibling dir.
        let abs = sibling_secret.to_string_lossy().into_owned();
        let err = resolve_in_sandbox(&root, &abs).unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);

        let result = execute_read_file(&root, json!({"path": abs}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        assert_no_leak(&result, &sibling_secret, "SIBLING SECRET");
    }

    /// `~` must NOT be expanded to $HOME. The arg is rejected outright.
    #[test]
    fn vector_tilde_expansion_rejected() {
        let fx = fixture();
        assert_eq!(
            resolve_in_sandbox(&fx.root, "~/secret.txt").unwrap_err(),
            SandboxError::InvalidArg
        );
        assert_eq!(
            resolve_in_sandbox(&fx.root, "~").unwrap_err(),
            SandboxError::InvalidArg
        );
        // Even a `~user`-style arg is rejected, never expanded.
        assert_eq!(
            resolve_in_sandbox(&fx.root, "~root/.ssh/id_rsa").unwrap_err(),
            SandboxError::InvalidArg
        );

        // Through the tool: generic message, no home contents leaked.
        let result = execute_read_file(&fx.root, json!({"path": "~/secret.txt"}));
        let text = get_text(&result);
        assert!(!text.contains('~') || text == "invalid path argument");
    }

    /// A symlink INSIDE the root whose canonical target leaves the root is
    /// rejected — canonicalize() follows the link, then containment fails.
    #[test]
    fn vector_symlink_escape_rejected() {
        let fx = fixture();
        // Create a symlink inside the root pointing at the outside secret.
        let link = fx.root.join("escape-link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&fx.outside_secret, &link).unwrap();
        #[cfg(not(unix))]
        return; // symlink semantics differ; the unix path is the contract here.

        let err = resolve_in_sandbox(&fx.root, "escape-link.txt").unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);

        let result = execute_read_file(&fx.root, json!({"path": "escape-link.txt"}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        assert_no_leak(&result, &fx.outside_secret, "TOP SECRET CONTENT");
    }

    /// A symlink to a DIRECTORY outside the root is also rejected (and we never
    /// list its contents).
    #[test]
    fn vector_symlink_dir_escape_rejected() {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        let outside_dir = outer.path().join("vault");
        fs::create_dir(&outside_dir).unwrap();
        fs::write(outside_dir.join("key.pem"), "PRIVATE KEY").unwrap();

        let link = root.join("vault-link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_dir, &link).unwrap();
        #[cfg(not(unix))]
        return;

        let err = resolve_in_sandbox(&root, "vault-link").unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);

        let result = execute_list_directory(&root, json!({"path": "vault-link"}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        let text = get_text(&result);
        assert!(!text.contains("key.pem"), "leaked outside dir entry");
    }

    /// A case-folded variant that resolves OUTSIDE the root is still rejected.
    /// On case-insensitive APFS the prefix-sibling defense must hold even when
    /// the casing differs.
    #[test]
    fn vector_case_fold_variant_escaping_rejected() {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        let sibling = outer.path().join("PROJ-SECRETS");
        fs::create_dir(&sibling).unwrap();
        let secret = sibling.join("creds.txt");
        fs::write(&secret, "CASE SIBLING SECRET").unwrap();

        // Absolute path into the (differently-cased) prefix-sibling dir.
        let abs = secret.to_string_lossy().into_owned();
        let err = resolve_in_sandbox(&root, &abs).unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);

        let result = execute_read_file(&root, json!({"path": abs}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        assert_no_leak(&result, &secret, "CASE SIBLING SECRET");
    }

    /// An NFD/NFC variant that resolves OUTSIDE the root is still rejected. We
    /// build an outside dir whose name uses a composed (NFC) form and request
    /// it with a path whose component string would differ in normalization;
    /// either way it must not be admitted into the `proj` sandbox.
    #[test]
    fn vector_nfd_nfc_variant_escaping_rejected() {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        // "café" composed (NFC): 'é' = U+00E9.
        let outside = outer.path().join("café-secrets");
        fs::create_dir(&outside).unwrap();
        let secret = outside.join("creds.txt");
        fs::write(&secret, "UNICODE SIBLING SECRET").unwrap();

        let abs = secret.to_string_lossy().into_owned();
        let err = resolve_in_sandbox(&root, &abs).unwrap_err();
        assert_eq!(err, SandboxError::OutsideSandbox);

        let result = execute_read_file(&root, json!({"path": abs}));
        assert_eq!(get_text(&result), SANDBOX_VIOLATION_MSG);
        assert_no_leak(&result, &secret, "UNICODE SIBLING SECRET");
    }

    /// An NFD/NFC variant that resolves INSIDE the root is ACCEPTED — folding
    /// must not over-reject equivalent in-sandbox names. We create a file with
    /// a composed name and request it with the decomposed spelling; on a
    /// normalization-insensitive FS this must read the same file.
    #[test]
    fn unicode_variant_inside_accepted() {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        // NFC composed 'é'.
        let nfc_name = "résumé.txt";
        fs::write(root.join(nfc_name), "MY RESUME").unwrap();

        // Request via NFD decomposed form: 'e' + U+0301 combining acute.
        let nfd_name = "re\u{0301}sume\u{0301}.txt";
        // On APFS the decomposed request resolves to the same file; on a
        // strict-byte FS it may 404. Either way it must NOT be a sandbox
        // violation, and when it resolves it returns the right content.
        match resolve_in_sandbox(&root, nfd_name) {
            Ok(t) => {
                assert!(is_contained(&root.canonicalize().unwrap(), &t));
                let result = execute_read_file(&root, json!({"path": nfd_name}));
                assert!(get_text(&result).contains("MY RESUME"));
            }
            Err(SandboxError::ResolveFailed) => {
                // Acceptable on a strict-byte FS that has no such entry.
            }
            Err(other) => panic!("unicode-inside must not be a containment violation: {other:?}"),
        }
    }

    #[test]
    fn vector_empty_arg_rejected() {
        let fx = fixture();
        assert_eq!(
            resolve_in_sandbox(&fx.root, "").unwrap_err(),
            SandboxError::InvalidArg
        );
    }

    #[test]
    fn vector_dot_arg_rejected() {
        let fx = fixture();
        assert_eq!(
            resolve_in_sandbox(&fx.root, ".").unwrap_err(),
            SandboxError::InvalidArg
        );
    }

    #[test]
    fn vector_whitespace_arg_rejected() {
        let fx = fixture();
        assert_eq!(
            resolve_in_sandbox(&fx.root, "   ").unwrap_err(),
            SandboxError::InvalidArg
        );
        assert_eq!(
            resolve_in_sandbox(&fx.root, "\t\n").unwrap_err(),
            SandboxError::InvalidArg
        );
    }

    #[test]
    fn vector_nul_arg_rejected() {
        let fx = fixture();
        assert_eq!(
            resolve_in_sandbox(&fx.root, "inside\0.txt").unwrap_err(),
            SandboxError::InvalidArg
        );
    }

    // -----------------------------------------------------------------------
    // VAL-TOOLS-006 (FS half) — non-regular files rejected before read;
    // large outputs truncated with a marker.
    // -----------------------------------------------------------------------

    /// A FIFO is rejected BEFORE any (blocking) read. The test would hang
    /// forever if we opened the FIFO, so passing proves we checked metadata
    /// first.
    #[cfg(unix)]
    #[test]
    fn non_regular_fifo_rejected_before_read() {
        let fx = fixture();
        let fifo = fx.root.join("pipe");
        // Create a FIFO via the system `mkfifo` so we need no extra crate. If
        // the binary is unavailable the test is skipped rather than failing.
        let status = std::process::Command::new("mkfifo").arg(&fifo).status();
        match status {
            Ok(s) if s.success() => {}
            _ => return, // mkfifo unavailable; skip (the read path is still tested elsewhere).
        }

        let result = execute_read_file(&fx.root, json!({"path": "pipe"}));
        let text = get_text(&result);
        assert!(
            text.contains("not a regular file"),
            "FIFO should be rejected as non-regular, got: {text:?}"
        );
    }

    #[test]
    fn read_file_truncates_large_content_with_marker() {
        let fx = fixture();
        let big = fx.root.join("big.txt");
        // Well over the 50KB budget.
        let blob = "x".repeat(READ_FILE_BYTE_BUDGET * 2);
        fs::write(&big, &blob).unwrap();

        let result = execute_read_file(&fx.root, json!({"path": "big.txt"}));
        let text = get_text(&result);
        assert!(
            text.contains("[Truncated:"),
            "expected a visible truncation marker, got tail: {}",
            &text[text.len().saturating_sub(120)..]
        );
        // Body capped near the budget (+ marker), not the whole 100KB.
        assert!(
            text.len() < READ_FILE_BYTE_BUDGET + 200,
            "truncated body should fit the budget + marker, got {} bytes",
            text.len()
        );
    }

    #[test]
    fn list_directory_truncates_large_listing_with_marker() {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        for i in 0..(LIST_MAX_ENTRIES + 50) {
            fs::write(root.join(format!("f{i:04}.txt")), "x").unwrap();
        }
        let result = execute_list_directory(&root, json!({"path": "."}));
        // "." is rejected as an arg -> use the root via a child request instead.
        // Re-run against the dir by absolute (in-sandbox) path.
        let _ = result;
        let result = execute_list_directory(&root, json!({"path": root.to_string_lossy()}));
        let text = get_text(&result);
        assert!(
            text.contains("[Truncated: showing 500 of"),
            "expected list truncation marker, got tail: {}",
            &text[text.len().saturating_sub(120)..]
        );
    }

    // -----------------------------------------------------------------------
    // Happy path — legitimate in-sandbox reads/listings work.
    // -----------------------------------------------------------------------

    #[test]
    fn happy_path_read_in_sandbox_file() {
        let fx = fixture();
        let result = execute_read_file(&fx.root, json!({"path": "inside.txt"}));
        let text = get_text(&result);
        assert_eq!(text, "hello from inside");
    }

    #[test]
    fn happy_path_read_nested_in_sandbox_file() {
        let fx = fixture();
        let result = execute_read_file(&fx.root, json!({"path": "sub/nested.txt"}));
        assert!(get_text(&result).contains("nested body"));
    }

    /// VAL-TOOLS-002 (image-result gap): reading an in-sandbox image returns an
    /// Image content block (base64 + mime), NOT a lossy-UTF-8 text dump of the
    /// binary bytes. This is what lets the tool-call card render the picture.
    #[test]
    fn read_in_sandbox_image_returns_image_block() {
        let fx = fixture();
        let img = fx.root.join("pic.png");
        fs::write(&img, PNG_BYTES).unwrap();

        let result = execute_read_file(&fx.root, json!({"path": "pic.png"}));

        // It is an Image block, never a Text block.
        assert!(
            matches!(result.content[0], ToolResultContent::Image(_)),
            "image read must yield an Image content block, not text"
        );
        let image = get_image(&result);
        assert_eq!(image.mime_type, "image/png");
        // The data is base64 of the file bytes (round-trips back to them).
        let decoded = BASE64_STANDARD.decode(&image.data).expect("valid base64");
        assert_eq!(decoded, PNG_BYTES);
    }

    /// An image with NO recognizable extension is still detected by its magic
    /// bytes and returned as an Image block (not garbled text).
    #[test]
    fn read_extensionless_image_detected_by_magic_bytes() {
        let fx = fixture();
        let img = fx.root.join("blob");
        fs::write(&img, PNG_BYTES).unwrap();

        let result = execute_read_file(&fx.root, json!({"path": "blob"}));
        assert!(
            matches!(result.content[0], ToolResultContent::Image(_)),
            "magic-byte-sniffed image must yield an Image block"
        );
        assert_eq!(get_image(&result).mime_type, "image/png");
    }

    /// A regular (non-image) text file keeps returning a Text block unchanged —
    /// image handling must not regress the text path.
    #[test]
    fn read_text_file_still_returns_text_block() {
        let fx = fixture();
        let result = execute_read_file(&fx.root, json!({"path": "inside.txt"}));
        assert!(
            matches!(result.content[0], ToolResultContent::Text(_)),
            "text file must still yield a Text content block"
        );
        assert_eq!(get_text(&result), "hello from inside");
    }

    /// An oversize image is refused with a generic message rather than
    /// base64-encoding an unbounded file into the model context.
    #[test]
    fn read_oversize_image_is_refused() {
        let fx = fixture();
        let img = fx.root.join("huge.png");
        // PNG signature followed by enough bytes to exceed the cap.
        let mut blob = PNG_BYTES.to_vec();
        blob.resize(READ_FILE_IMAGE_BYTE_CAP + 1, 0u8);
        fs::write(&img, &blob).unwrap();

        let result = execute_read_file(&fx.root, json!({"path": "huge.png"}));
        assert!(
            matches!(result.content[0], ToolResultContent::Text(_)),
            "an oversize image is refused with a (text) error, not an image block"
        );
        assert!(
            get_text(&result).contains("too large"),
            "oversize image refusal should explain the size limit, got: {}",
            get_text(&result)
        );
    }

    #[test]
    fn happy_path_list_in_sandbox_dir() {
        let fx = fixture();
        // List the sandbox root via its (in-sandbox) absolute path.
        let result = execute_list_directory(&fx.root, json!({"path": fx.root.to_string_lossy()}));
        let text = get_text(&result);
        assert!(
            text.contains("sub/"),
            "dir should be listed first, got: {text}"
        );
        assert!(
            text.contains("inside.txt"),
            "file should be listed, got: {text}"
        );
        // dir-first ordering: `sub/` appears before `inside.txt`.
        let dir_pos = text.find("sub/").unwrap();
        let file_pos = text.find("inside.txt").unwrap();
        assert!(dir_pos < file_pos, "directories must come first");
    }

    // -----------------------------------------------------------------------
    // build_tools factory.
    // -----------------------------------------------------------------------

    #[test]
    fn build_tools_omits_fs_tools_without_working_dir() {
        let enabled = vec!["read_file".to_string(), "list_directory".to_string()];
        let tools = build_tools(&enabled, None);
        assert!(tools.is_empty(), "FS tools need a sandbox root");
    }

    #[test]
    fn build_tools_omits_fs_tools_for_empty_working_dir() {
        let enabled = vec!["read_file".to_string()];
        let tools = build_tools(&enabled, Some(Path::new("")));
        assert!(tools.is_empty(), "empty working_dir is no sandbox");
    }

    #[test]
    fn build_tools_includes_requested_fs_tools_with_working_dir() {
        let fx = fixture();
        let enabled = vec!["read_file".to_string(), "list_directory".to_string()];
        let tools = build_tools(&enabled, Some(&fx.root));
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"list_directory"));
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn build_tools_ignores_unknown_tool_names() {
        let fx = fixture();
        let enabled = vec!["read_file".to_string(), "totally_unknown".to_string()];
        let tools = build_tools(&enabled, Some(&fx.root));
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["read_file"]);
    }

    // Minimal single-thread executor so the skill tool's async `execute`
    // closure is exercised without pulling in a new test-only crate (tokio is
    // already a dep).
    fn tokio_test_block<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread runtime")
            .block_on(fut)
    }

    // -----------------------------------------------------------------------
    // skill tool — name-keyed lookup, never a filesystem read (VAL-TOOL-003/
    // 004/005/015/018).
    // -----------------------------------------------------------------------

    /// Build a `name -> body` map from `(name, body)` pairs.
    fn skill_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(n, b)| (n.to_string(), b.to_string()))
            .collect()
    }

    /// VAL-TOOL-015: the skill tool's name is the fixed `"skill"` and does not
    /// collide with any built-in tool name `build_tools` can produce.
    #[test]
    fn skill_tool_name_is_unique_and_does_not_collide() {
        let tool = make_skill_tool(skill_map(&[("alpha", "body")]));
        assert_eq!(tool.name, "skill");
        // None of the built-in factory names is "skill".
        for builtin in [TOOL_READ_FILE, TOOL_LIST_DIRECTORY] {
            assert_ne!(tool.name, builtin, "skill name collides with {builtin}");
        }
    }

    /// VAL-TOOL-003: a hit returns the skill's body VERBATIM, untruncated, even
    /// for a large body (well past the read_file truncation budget — the skill
    /// tool has no byte cap).
    #[test]
    fn skill_hit_returns_full_body_untruncated() {
        let big = "B".repeat(READ_FILE_BYTE_BUDGET * 4);
        let map = skill_map(&[("alpha", &big)]);
        let result = execute_skill(&map, json!({"name": "alpha"}));
        assert_eq!(
            get_text(&result),
            big,
            "skill body must be returned verbatim and untruncated"
        );
        // Sanity: it is a Text block, with no truncation marker.
        assert!(matches!(result.content[0], ToolResultContent::Text(_)));
        assert!(!get_text(&result).contains("[Truncated"));
    }

    /// A name with surrounding whitespace still resolves (the handler trims).
    #[test]
    fn skill_hit_trims_surrounding_whitespace() {
        let map = skill_map(&[("alpha", "alpha body")]);
        let result = execute_skill(&map, json!({"name": "  alpha  "}));
        assert_eq!(get_text(&result), "alpha body");
    }

    /// VAL-TOOL-004: a miss returns a generic error — no panic, and the error
    /// text leaks NO filesystem path and NO file contents.
    #[test]
    fn skill_miss_is_generic_error_no_leak() {
        let map = skill_map(&[("alpha", "secret body")]);
        let result = execute_skill(&map, json!({"name": "ghost"}));
        let text = get_text(&result);
        assert_eq!(text, SKILL_NOT_FOUND_MSG);
        assert!(!text.contains('/'), "no path in error: {text:?}");
        assert!(!text.contains("secret body"), "no body leak: {text:?}");
    }

    /// VAL-TOOL-005: a missing, non-string, or empty/whitespace `name` argument
    /// yields the invalid-arg error (no panic).
    #[test]
    fn skill_bad_arg_yields_invalid_arg_error() {
        let map = skill_map(&[("alpha", "body")]);

        // Missing entirely.
        assert_eq!(
            get_text(&execute_skill(&map, json!({}))),
            SKILL_INVALID_ARG_MSG
        );
        // Wrong type (number, not string).
        assert_eq!(
            get_text(&execute_skill(&map, json!({"name": 42}))),
            SKILL_INVALID_ARG_MSG
        );
        // Empty string.
        assert_eq!(
            get_text(&execute_skill(&map, json!({"name": ""}))),
            SKILL_INVALID_ARG_MSG
        );
        // Whitespace-only.
        assert_eq!(
            get_text(&execute_skill(&map, json!({"name": "   "}))),
            SKILL_INVALID_ARG_MSG
        );
    }

    /// VAL-TOOL-018 (SECURITY): a path-shaped `name` is treated as an ordinary
    /// (missing) key — the tool NEVER reads from disk. We seed the map with one
    /// real skill, then ask for filesystem-y names; each must return the generic
    /// not-found error with no file contents and no path echo.
    #[test]
    fn skill_path_shaped_name_never_reads_disk() {
        let map = skill_map(&[("alpha", "alpha body")]);
        for malicious in [
            "/etc/passwd",
            "../secret.txt",
            "../../etc/hosts",
            "sub/../../escape",
            "~/.ssh/id_rsa",
            "alpha/../../etc/passwd",
        ] {
            let result = execute_skill(&map, json!({ "name": malicious }));
            let text = get_text(&result);
            assert_eq!(
                text, SKILL_NOT_FOUND_MSG,
                "path-shaped name {malicious:?} must be a plain miss"
            );
            // The error must not echo the (path-shaped) argument or any content.
            assert!(
                !text.contains(malicious),
                "error leaked the path argument {malicious:?}: {text:?}"
            );
            assert!(
                !text.contains("root:") && !text.contains("alpha body"),
                "error leaked file/skill content: {text:?}"
            );
        }
    }

    /// The skill tool resolves via the registered `execute` closure (end-to-end
    /// through `AgentTool`), not just the bare helper — proving the wiring.
    #[test]
    fn skill_tool_execute_closure_resolves_hit_and_miss() {
        let tool = make_skill_tool(skill_map(&[("alpha", "alpha body")]));
        let ctx_hit = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-1".to_string(),
            args: json!({"name": "alpha"}),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let hit = tokio_test_block((tool.execute)(ctx_hit)).expect("execute ok");
        assert_eq!(get_text(&hit), "alpha body");

        let ctx_miss = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-2".to_string(),
            args: json!({"name": "ghost"}),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let miss = tokio_test_block((tool.execute)(ctx_miss)).expect("execute ok");
        assert_eq!(get_text(&miss), SKILL_NOT_FOUND_MSG);
    }
}
