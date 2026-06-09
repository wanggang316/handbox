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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use hand_agent::{AgentTool, ToolResult};
use serde_json::json;

/// Tool names this factory knows how to build.
const TOOL_READ_FILE: &str = "read_file";
const TOOL_LIST_DIRECTORY: &str = "list_directory";
const TOOL_WEB_FETCH: &str = "web_fetch";

/// Byte budget for a single `read_file` result before truncation kicks in.
const READ_FILE_BYTE_BUDGET: usize = 50 * 1024;
/// Max entries a single `list_directory` result will emit before truncation.
const LIST_MAX_ENTRIES: usize = 500;

/// Wall-clock budget for a single `web_fetch` request (connect + read).
const WEB_FETCH_TIMEOUT: Duration = Duration::from_secs(20);
/// Hard cap on the response body we will read/return (defense-in-depth against
/// memory exhaustion + matches VAL-TOOLS-006's bounds requirement).
const WEB_FETCH_BYTE_CAP: usize = 256 * 1024;
/// Max redirect hops to follow before giving up (each hop is re-validated).
const WEB_FETCH_MAX_REDIRECTS: usize = 5;

/// Generic, leak-free message for any blocked-URL / SSRF rejection (D14/D19).
/// MUST NOT echo any local content, target host, or resolved IP.
const URL_NOT_ALLOWED_MSG: &str = "URL not allowed";
/// Generic message for a network/fetch failure that isn't a policy rejection.
const FETCH_FAILED_MSG: &str = "fetch failed";

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
///
/// Designed to extend cleanly: the next feature adds `web_fetch`, which does
/// not require a sandbox root and so will be appended regardless of
/// `working_dir`.
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
            // web_fetch needs no sandbox root — it reaches the (public) network,
            // not the filesystem — so it is available regardless of working_dir.
            TOOL_WEB_FETCH => {
                tools.push(make_web_fetch_tool());
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

    // Binary-safe: lossy-decode so we never crash on non-UTF-8 / image bytes.
    // (The image-result-block UI is a later feature; here a placeholder/text
    // representation is sufficient and must not panic.)
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
// web_fetch — SSRF-guarded HTTP(S) fetcher (SECURITY CORE, D13/D19)
// ===========================================================================
//
// The model supplies a URL. Before ANY network I/O we must guarantee the
// request can only ever reach a *public* http(s) endpoint:
//
//   1. Scheme allowlist: only `http` / `https`.
//   2. Host/IP denylist on the RESOLVED IP (not just the literal host string):
//      loopback, link-local (incl. cloud-metadata 169.254.169.254), RFC1918
//      private, unspecified, and IPv6 unique-local are all rejected.
//   3. Encoding-bypass coverage: IPv4-mapped IPv6, decimal/octal/hex integer
//      IPs, trailing-dot hosts, and userinfo-`@` confusion.
//   4. Redirect re-validation: every hop's target is re-checked (no reqwest
//      default redirect-follow).
//   5. Bounds: request timeout + response byte cap.
//   6. Error hygiene: a blocked URL returns a GENERIC message that never echoes
//      the target/IP or any fetched content.
//
// The decision logic is factored into PURE functions (`classify_scheme`,
// `is_blocked_ip`, `parse_literal_ip`, `normalize_host`, `redirect_is_allowed`)
// so every bypass class is unit-testable without real DNS or sockets. The live
// fetch is a thin wrapper around the guard.
//
// ACCEPTED RESIDUAL RISK (plan): DNS rebinding / TOCTOU — the host could
// re-resolve to a blocked IP between our resolve-check and reqwest's own
// connect. For v1 (single-user local), resolve -> classify -> reject-or-fetch
// is the accepted guard; fully closing the race (e.g. pinning the validated IP
// into the connection) is out of scope and intentionally not attempted.

/// Why a `web_fetch` target was refused before/while fetching. Every variant
/// maps to the SAME generic, leak-free message so error text never reveals the
/// target host, the resolved IP, or any local content (D14/D19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchGuardError {
    /// Scheme is not in the http/https allowlist.
    SchemeNotAllowed,
    /// The URL could not be parsed into a usable http(s) target.
    InvalidUrl,
    /// The (literal or resolved) host maps to a blocked IP.
    BlockedHost,
    /// DNS produced no usable address for the host.
    NoResolvableAddress,
}

impl FetchGuardError {
    /// Leak-free message safe to return to the model.
    pub fn display_message(&self) -> &'static str {
        URL_NOT_ALLOWED_MSG
    }
}

/// Is `scheme` an allowed transport? Only `http` / `https` (case-insensitive).
/// Rejects `file`, `ftp`, `data`, `gopher`, etc. — the classic SSRF/LFI lever.
pub fn classify_scheme(scheme: &str) -> bool {
    matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
}

/// Normalize a host string for literal-IP parsing and DNS resolution:
/// strip surrounding IPv6 brackets, drop a single trailing dot (FQDN root /
/// `127.0.0.1.` bypass), and lowercase. Returns the cleaned host.
pub fn normalize_host(host: &str) -> String {
    let mut h = host.trim();
    // Strip a single bracket pair around an IPv6 literal: `[::1]` -> `::1`.
    if h.starts_with('[') && h.ends_with(']') && h.len() >= 2 {
        h = &h[1..h.len() - 1];
    }
    // Drop exactly one trailing dot so `127.0.0.1.` == `127.0.0.1` and
    // `metadata.google.internal.` doesn't dodge a domain check.
    let trimmed = h.strip_suffix('.').unwrap_or(h);
    trimmed.to_ascii_lowercase()
}

/// Parse a host string into a single literal `IpAddr`, covering the encoding
/// bypasses an attacker uses to dodge a naive "is it 127.0.0.1?" check:
///
///   - dotted IPv4 (`127.0.0.1`) and IPv6 (`::1`, `::ffff:127.0.0.1`);
///   - a bare decimal integer (`2130706433` == 127.0.0.1);
///   - a hex integer (`0x7f000001`);
///   - an octal integer (`0177.0.0.1` is handled by the dotted path below via
///     per-octet octal/hex parsing; a single `0177...` whole-number octal is
///     also handled).
///
/// Returns `None` when the host is a real domain name (resolve it via DNS).
/// `host` is assumed already normalized (no brackets, no trailing dot).
pub fn parse_literal_ip(host: &str) -> Option<IpAddr> {
    // 1. Standard textual forms first (covers `::ffff:127.0.0.1` natively).
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    // 2. A single whole-number integer IPv4 (decimal / hex / octal).
    if let Some(v4) = parse_u32_ipv4(host) {
        return Some(IpAddr::V4(v4));
    }
    // 3. Dotted form where each octet may be decimal / hex / octal
    //    (`0177.0.0.1`, `0x7f.0.0.1`, `127.000.000.001`).
    if let Some(v4) = parse_dotted_radix_ipv4(host) {
        return Some(IpAddr::V4(v4));
    }
    None
}

/// Parse a single whole-number integer host as IPv4: decimal, `0x`/`0X` hex,
/// or leading-`0` octal. `2130706433` / `0x7f000001` / `017700000001`.
fn parse_u32_ipv4(s: &str) -> Option<Ipv4Addr> {
    if s.is_empty() {
        return None;
    }
    let n = parse_radix_u32(s)?;
    Some(Ipv4Addr::from(n))
}

/// Parse a dotted host (2-4 parts) where each part may be decimal / hex / octal,
/// assembling a 32-bit IPv4. Only the 4-octet shape is accepted here (the
/// common bypass `0177.0.0.1`); other "inet_aton" shapes (1-3 parts) are
/// covered by `parse_u32_ipv4` for the single-number case.
fn parse_dotted_radix_ipv4(s: &str) -> Option<Ipv4Addr> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut octets = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        let v = parse_radix_u32(part)?;
        if v > 255 {
            return None;
        }
        octets[i] = v as u8;
    }
    Some(Ipv4Addr::from(octets))
}

/// Parse an unsigned integer in decimal, `0x` hex, or leading-zero octal.
/// Rejects anything that isn't a clean integer in the detected radix.
fn parse_radix_u32(s: &str) -> Option<u32> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        if hex.is_empty() {
            return None;
        }
        return u32::from_str_radix(hex, 16).ok();
    }
    // Leading-zero (length > 1) means octal, as in inet_aton.
    if s.len() > 1 && s.starts_with('0') {
        return u32::from_str_radix(s, 8).ok();
    }
    s.parse::<u32>().ok()
}

/// THE classification core: is this resolved/literal IP forbidden for SSRF?
///
/// Rejects, for IPv4: loopback (127/8), link-local (169.254/16, INCLUDING the
/// cloud-metadata 169.254.169.254), RFC1918 private (10/8, 172.16/12,
/// 192.168/16), broadcast (255.255.255.255), and unspecified (0.0.0.0).
///
/// For IPv6: loopback (::1), unspecified (::), unique-local (fc00::/7), and
/// unicast link-local (fe80::/10). An IPv4-mapped IPv6 (`::ffff:a.b.c.d`) is
/// unwrapped to its v4 address and re-classified, so `::ffff:127.0.0.1` is
/// caught. The unstable `Ipv6Addr::is_unique_local` / `is_unicast_link_local`
/// helpers are inlined as manual bit checks to avoid a nightly dependency.
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(v6) => {
            // Native v6 self-addresses first: `::1`/`::` must be caught here.
            // (We must NOT route these through the v4 path: the deprecated
            // `to_ipv4()` would map `::1` to the harmless-looking `0.0.0.1`.)
            if is_blocked_ipv6(v6) {
                return true;
            }
            // Unwrap an IPv4-MAPPED address (`::ffff:a.b.c.d`) and re-classify
            // against the v4 rules — `::ffff:127.0.0.1` must be blocked. We use
            // `to_ipv4_mapped` (mapped form only), NOT the broader `to_ipv4`,
            // so we don't misclassify `::1`/`::` as a v4 address.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ipv4(v4);
            }
            false
        }
    }
}

fn is_blocked_ipv4(v4: Ipv4Addr) -> bool {
    v4.is_loopback()          // 127.0.0.0/8
        || v4.is_private()    // 10/8, 172.16/12, 192.168/16
        || v4.is_link_local() // 169.254.0.0/16 (incl. 169.254.169.254 metadata)
        || v4.is_broadcast()  // 255.255.255.255
        || v4.is_unspecified()// 0.0.0.0
        || v4.is_documentation()
}

fn is_blocked_ipv6(v6: Ipv6Addr) -> bool {
    if v6.is_loopback() || v6.is_unspecified() {
        return true;
    }
    let seg = v6.segments();
    // Unique-local fc00::/7 — top 7 bits are 1111110.
    let is_unique_local = (seg[0] & 0xfe00) == 0xfc00;
    // Unicast link-local fe80::/10 — top 10 bits are 1111111010.
    let is_link_local = (seg[0] & 0xffc0) == 0xfe80;
    is_unique_local || is_link_local
}

/// Resolve `host` to its addresses and reject if ANY resolved address is a
/// blocked IP. A literal-IP host is classified directly (no DNS). Returns the
/// validated, resolvable target on success.
///
/// `host` must already be normalized. This is the only function in the guard
/// that touches DNS; the classification it delegates to is pure + tested.
fn resolve_and_validate_host(host: &str, port: u16) -> Result<(), FetchGuardError> {
    if host.is_empty() {
        return Err(FetchGuardError::InvalidUrl);
    }
    // Literal IP (incl. all the integer/dotted-radix bypass forms): classify
    // without DNS.
    if let Some(ip) = parse_literal_ip(host) {
        return if is_blocked_ip(ip) {
            Err(FetchGuardError::BlockedHost)
        } else {
            Ok(())
        };
    }
    // Real domain: resolve and check EVERY address.
    let addrs: Vec<_> = (host, port)
        .to_socket_addrs()
        .map_err(|_| FetchGuardError::NoResolvableAddress)?
        .collect();
    if addrs.is_empty() {
        return Err(FetchGuardError::NoResolvableAddress);
    }
    if addrs.iter().any(|sa| is_blocked_ip(sa.ip())) {
        return Err(FetchGuardError::BlockedHost);
    }
    Ok(())
}

/// Pure guard for a parsed URL: validate the scheme and extract a normalized
/// `(host, port)` ready for resolution. Splitting this out lets the redirect
/// path re-use the exact same scheme + host-extraction logic the initial
/// request uses. Userinfo-`@` confusion is neutralized here because
/// `reqwest::Url::host_str()` returns the host AFTER any `user:pass@`.
fn scheme_and_host_from_url(url: &reqwest::Url) -> Result<(String, u16), FetchGuardError> {
    if !classify_scheme(url.scheme()) {
        return Err(FetchGuardError::SchemeNotAllowed);
    }
    let raw_host = url.host_str().ok_or(FetchGuardError::InvalidUrl)?;
    let host = normalize_host(raw_host);
    if host.is_empty() {
        return Err(FetchGuardError::InvalidUrl);
    }
    let port = url
        .port_or_known_default()
        .unwrap_or(if url.scheme() == "https" { 443 } else { 80 });
    Ok((host, port))
}

/// Full guard decision for a single URL (initial or post-redirect target):
/// scheme allowlist -> host extraction -> resolve-and-classify. Returns `Ok`
/// only when the target is a public http(s) endpoint.
fn validate_url(url: &reqwest::Url) -> Result<(), FetchGuardError> {
    let (host, port) = scheme_and_host_from_url(url)?;
    resolve_and_validate_host(&host, port)
}

/// Redirect-policy decision: may we follow a redirect to `target`? Re-applies
/// the FULL guard (scheme + host/IP) to the hop target, so an
/// `https://public -> file://` or `-> 169.254.169.254` redirect is refused.
/// Pure over the parsed target URL (DNS aside), so it is unit-testable.
fn redirect_is_allowed(target: &reqwest::Url) -> bool {
    validate_url(target).is_ok()
}

/// Build the `web_fetch` tool. Unlike the FS tools it needs no sandbox root,
/// so it is always available when enabled.
fn make_web_fetch_tool() -> AgentTool {
    AgentTool::simple(
        TOOL_WEB_FETCH,
        "Fetch an http(s) URL and return its readable text content. Only public \
         http/https endpoints are reachable; local, loopback, private, and \
         cloud-metadata addresses are blocked. Large responses are truncated.",
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Absolute http(s) URL to fetch."
                }
            },
            "required": ["url"]
        }),
        "Fetch URL",
        move |_tool_call_id, args| async move { execute_web_fetch(args).await },
    )
}

/// `web_fetch` body: parse + guard the URL, fetch with a custom redirect policy
/// that re-validates every hop, then return bounded readable text. All policy
/// rejections collapse to a generic message (no leak of host/IP/content).
async fn execute_web_fetch(args: serde_json::Value) -> ToolResult {
    let url_str = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return ToolResult::error("Missing required parameter: url"),
    };

    let url = match reqwest::Url::parse(url_str) {
        Ok(u) => u,
        // A relative / malformed URL is treated as not-allowed (generic).
        Err(_) => return ToolResult::error(FetchGuardError::InvalidUrl.display_message()),
    };

    // Guard the INITIAL target before any network I/O.
    if let Err(e) = validate_url(&url) {
        return ToolResult::error(e.display_message());
    }

    fetch_guarded_text(url).await
}

/// Perform the actual (thin) fetch with a custom redirect policy that
/// re-validates every hop. The security is in the guard; this just wires reqwest
/// with the timeout, the byte cap, and the per-hop re-validation.
async fn fetch_guarded_text(url: reqwest::Url) -> ToolResult {
    // Custom redirect policy: re-apply the full guard to each hop target. A
    // blocked or non-http(s) redirect target stops the chain. We DO NOT use
    // reqwest's default follow (which would happily chase a downgrade).
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= WEB_FETCH_MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        if redirect_is_allowed(attempt.url()) {
            attempt.follow()
        } else {
            // Stop rather than follow; surfaced to the caller as a generic
            // failure below (the response will be the redirect itself, which we
            // treat as non-fetchable / blocked).
            attempt.stop()
        }
    });

    let client = match reqwest::Client::builder()
        .timeout(WEB_FETCH_TIMEOUT)
        .redirect(policy)
        .build()
    {
        Ok(c) => c,
        Err(_) => return ToolResult::error(FETCH_FAILED_MSG),
    };

    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(_) => return ToolResult::error(FETCH_FAILED_MSG),
    };

    // A redirect that our policy `stop`-ped surfaces as a 3xx response with a
    // Location we refused to follow. Treat any non-success as a generic failure
    // (never echo the redirect Location, which could be the blocked host).
    if !resp.status().is_success() {
        if resp.status().is_redirection() {
            return ToolResult::error(URL_NOT_ALLOWED_MSG);
        }
        return ToolResult::error(FETCH_FAILED_MSG);
    }

    // Stream the body up to the byte cap, then stop (defense against a huge or
    // never-ending response). We must `use` StreamExt for `.next()`.
    use futures::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(_) => return ToolResult::error(FETCH_FAILED_MSG),
        };
        let remaining = WEB_FETCH_BYTE_CAP.saturating_sub(buf.len());
        if chunk.len() >= remaining {
            buf.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }
        buf.extend_from_slice(&chunk);
    }

    let text = html_to_readable_text(&String::from_utf8_lossy(&buf));
    let (body, body_truncated) = truncate_text(&text, WEB_FETCH_BYTE_CAP);
    let mut output = body;
    if truncated || body_truncated {
        output.push_str("\n[Truncated: response exceeded the size limit.]");
    }
    ToolResult::text(output)
}

/// Turn fetched markup into "readable text". This is deliberately light — the
/// GUARD is the point of this feature, not a full HTML renderer. We strip
/// `<script>`/`<style>` blocks and tags, collapse whitespace, and decode a few
/// common entities so the model gets prose rather than raw markup.
fn html_to_readable_text(input: &str) -> String {
    let without_blocks = strip_tag_block(&strip_tag_block(input, "script"), "style");

    let mut out = String::with_capacity(without_blocks.len());
    let mut in_tag = false;
    for ch in without_blocks.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse runs of whitespace (incl. the newlines left by stripped tags).
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove `<tag ...> ... </tag>` blocks (case-insensitive on the tag name).
fn strip_tag_block(input: &str, tag: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let open_needle = format!("<{tag}");
    let close_needle = format!("</{tag}>");
    let mut result = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(rel_open) = lower[cursor..].find(&open_needle) {
        let open = cursor + rel_open;
        result.push_str(&input[cursor..open]);
        // Find the matching close tag after the open; if missing, drop the rest.
        match lower[open..].find(&close_needle) {
            Some(rel_close) => {
                cursor = open + rel_close + close_needle.len();
            }
            None => {
                cursor = input.len();
                break;
            }
        }
    }
    result.push_str(&input[cursor..]);
    result
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

    // -----------------------------------------------------------------------
    // web_fetch — build_tools wiring.
    // -----------------------------------------------------------------------

    #[test]
    fn build_tools_includes_web_fetch_without_working_dir() {
        // web_fetch needs no sandbox root, so it appears even when working_dir
        // is None — unlike the FS tools.
        let enabled = vec!["web_fetch".to_string()];
        let tools = build_tools(&enabled, None);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["web_fetch"]);
    }

    #[test]
    fn build_tools_includes_web_fetch_alongside_fs_tools() {
        let fx = fixture();
        let enabled = vec!["read_file".to_string(), "web_fetch".to_string()];
        let tools = build_tools(&enabled, Some(&fx.root));
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"web_fetch"));
        assert_eq!(tools.len(), 2);
    }

    // -----------------------------------------------------------------------
    // VAL-TOOLS-010 — web_fetch SSRF guard.
    // Each bypass class is its own test (the security value is the enumeration).
    // The guard is factored into PURE functions so every class is testable
    // without real DNS or sockets.
    // -----------------------------------------------------------------------

    fn ip(s: &str) -> IpAddr {
        s.parse().expect("valid ip literal in test")
    }

    fn url(s: &str) -> reqwest::Url {
        reqwest::Url::parse(s).expect("valid url in test")
    }

    // ---- scheme allowlist (req 1) -------------------------------------

    #[test]
    fn scheme_allows_only_http_and_https() {
        assert!(classify_scheme("http"));
        assert!(classify_scheme("https"));
        assert!(classify_scheme("HTTP"));
        assert!(classify_scheme("HTTPS"));
        assert!(!classify_scheme("file"));
        assert!(!classify_scheme("ftp"));
        assert!(!classify_scheme("data"));
        assert!(!classify_scheme("gopher"));
        assert!(!classify_scheme("ws"));
    }

    #[test]
    fn validate_url_rejects_file_scheme() {
        let err = validate_url(&url("file:///etc/passwd")).unwrap_err();
        assert_eq!(err, FetchGuardError::SchemeNotAllowed);
    }

    #[test]
    fn validate_url_rejects_ftp_scheme() {
        let err = validate_url(&url("ftp://example.com/x")).unwrap_err();
        assert_eq!(err, FetchGuardError::SchemeNotAllowed);
    }

    #[test]
    fn execute_web_fetch_rejects_non_http_scheme_generically() {
        let result = tokio_test_block(execute_web_fetch(json!({"url": "file:///etc/passwd"})));
        // Generic message: no scheme, no path, no leak.
        assert_eq!(get_text(&result), URL_NOT_ALLOWED_MSG);
    }

    // ---- literal blocked IP classes (req 2) ---------------------------

    #[test]
    fn blocks_ipv4_loopback() {
        assert!(is_blocked_ip(ip("127.0.0.1")));
        assert!(is_blocked_ip(ip("127.10.20.30")));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        assert!(is_blocked_ip(ip("::1")));
    }

    #[test]
    fn blocks_link_local_and_cloud_metadata() {
        assert!(is_blocked_ip(ip("169.254.0.1")));
        // The AWS/GCP cloud-metadata endpoint — the canonical SSRF target.
        assert!(is_blocked_ip(ip("169.254.169.254")));
        assert!(is_blocked_ip(ip("fe80::1")));
    }

    #[test]
    fn blocks_rfc1918_private_ranges() {
        assert!(is_blocked_ip(ip("10.0.0.1")));
        assert!(is_blocked_ip(ip("172.16.0.1")));
        assert!(is_blocked_ip(ip("172.31.255.255")));
        assert!(is_blocked_ip(ip("192.168.1.1")));
    }

    #[test]
    fn blocks_unspecified_and_broadcast() {
        assert!(is_blocked_ip(ip("0.0.0.0")));
        assert!(is_blocked_ip(ip("255.255.255.255")));
        assert!(is_blocked_ip(ip("::")));
    }

    #[test]
    fn blocks_ipv6_unique_local() {
        // fc00::/7 — both fc00:: and fd00:: prefixes.
        assert!(is_blocked_ip(ip("fc00::1")));
        assert!(is_blocked_ip(ip("fd12:3456:789a::1")));
    }

    #[test]
    fn allows_public_ips() {
        // Sanity: the guard must NOT over-block real public addresses.
        assert!(!is_blocked_ip(ip("8.8.8.8")));
        assert!(!is_blocked_ip(ip("1.1.1.1")));
        assert!(!is_blocked_ip(ip("2606:4700:4700::1111"))); // Cloudflare DNS v6
    }

    // ---- encoding bypasses (req 3) ------------------------------------

    #[test]
    fn blocks_ipv4_mapped_ipv6_loopback() {
        // `::ffff:127.0.0.1` must unwrap to 127.0.0.1 and be blocked.
        assert!(is_blocked_ip(ip("::ffff:127.0.0.1")));
        // ...and the metadata endpoint via the mapped form.
        assert!(is_blocked_ip(ip("::ffff:169.254.169.254")));
    }

    #[test]
    fn parses_and_blocks_decimal_integer_ip() {
        // 2130706433 == 127.0.0.1
        let parsed = parse_literal_ip("2130706433").expect("decimal int IP");
        assert_eq!(parsed, ip("127.0.0.1"));
        assert!(is_blocked_ip(parsed));
    }

    #[test]
    fn parses_and_blocks_hex_integer_ip() {
        // 0x7f000001 == 127.0.0.1
        let parsed = parse_literal_ip("0x7f000001").expect("hex int IP");
        assert_eq!(parsed, ip("127.0.0.1"));
        assert!(is_blocked_ip(parsed));
    }

    #[test]
    fn parses_and_blocks_octal_dotted_ip() {
        // 0177.0.0.1 == 127.0.0.1 (octal first octet).
        let parsed = parse_literal_ip("0177.0.0.1").expect("octal dotted IP");
        assert_eq!(parsed, ip("127.0.0.1"));
        assert!(is_blocked_ip(parsed));
    }

    #[test]
    fn normalize_host_strips_trailing_dot() {
        // `127.0.0.1.` must normalize to `127.0.0.1` so the literal-IP check
        // catches it.
        assert_eq!(normalize_host("127.0.0.1."), "127.0.0.1");
        let parsed = parse_literal_ip(&normalize_host("127.0.0.1.")).expect("trailing-dot IP");
        assert!(is_blocked_ip(parsed));
    }

    #[test]
    fn normalize_host_strips_ipv6_brackets_and_lowercases() {
        assert_eq!(normalize_host("[::1]"), "::1");
        assert_eq!(normalize_host("Example.COM"), "example.com");
    }

    #[test]
    fn validate_url_neutralizes_userinfo_at_confusion() {
        // The REAL host is after the `@`: this must be classified as 127.0.0.1,
        // not as the decoy `expected-host`.
        let err = validate_url(&url("http://expected-host@127.0.0.1/")).unwrap_err();
        assert_eq!(err, FetchGuardError::BlockedHost);
    }

    #[test]
    fn validate_url_blocks_trailing_dot_loopback_host() {
        let err = validate_url(&url("http://127.0.0.1./")).unwrap_err();
        assert_eq!(err, FetchGuardError::BlockedHost);
    }

    #[test]
    fn validate_url_blocks_decimal_integer_host() {
        let err = validate_url(&url("http://2130706433/")).unwrap_err();
        assert_eq!(err, FetchGuardError::BlockedHost);
    }

    #[test]
    fn validate_url_blocks_bracketed_ipv6_loopback() {
        let err = validate_url(&url("http://[::1]/")).unwrap_err();
        assert_eq!(err, FetchGuardError::BlockedHost);
    }

    #[test]
    fn validate_url_blocks_ipv4_mapped_ipv6_host() {
        let err = validate_url(&url("http://[::ffff:127.0.0.1]/")).unwrap_err();
        assert_eq!(err, FetchGuardError::BlockedHost);
    }

    // resolved-IP-to-blocked: a literal-IP host is the deterministic, DNS-free
    // proxy for the resolved-IP path (the resolution branch delegates to the
    // SAME `is_blocked_ip`). We also assert the resolver branch directly.
    #[test]
    fn resolve_and_validate_blocks_literal_blocked_ip() {
        assert_eq!(
            resolve_and_validate_host("169.254.169.254", 80).unwrap_err(),
            FetchGuardError::BlockedHost
        );
    }

    #[test]
    fn resolve_and_validate_blocks_localhost_resolution() {
        // `localhost` resolves to a loopback address on every dev/CI host;
        // exercising the real resolver path proves resolved-IP classification.
        assert_eq!(
            resolve_and_validate_host("localhost", 80).unwrap_err(),
            FetchGuardError::BlockedHost
        );
    }

    #[test]
    fn resolve_and_validate_rejects_empty_host() {
        assert_eq!(
            resolve_and_validate_host("", 80).unwrap_err(),
            FetchGuardError::InvalidUrl
        );
    }

    // ---- redirect re-validation (req 4) -------------------------------

    #[test]
    fn redirect_to_blocked_host_is_refused() {
        assert!(!redirect_is_allowed(&url("http://127.0.0.1/")));
        assert!(!redirect_is_allowed(&url(
            "http://169.254.169.254/latest/meta-data/"
        )));
    }

    #[test]
    fn redirect_to_non_http_scheme_is_refused() {
        // An https -> file:// downgrade must be refused at the hop.
        assert!(!redirect_is_allowed(&url("file:///etc/passwd")));
        assert!(!redirect_is_allowed(&url("ftp://internal/")));
    }

    #[test]
    fn redirect_to_decimal_integer_loopback_is_refused() {
        assert!(!redirect_is_allowed(&url("http://2130706433/")));
    }

    #[test]
    fn redirect_to_public_host_is_allowed() {
        // A redirect to a real public host is fine (the guard is not a blanket
        // "no redirects" rule).
        assert!(redirect_is_allowed(&url("https://example.com/")));
    }

    // ---- error hygiene (req 5) ----------------------------------------

    #[test]
    fn blocked_url_error_is_generic_and_leak_free() {
        // The blocked-URL message must not echo the host/IP/path.
        let result = tokio_test_block(execute_web_fetch(
            json!({"url": "http://169.254.169.254/secret"}),
        ));
        let text = get_text(&result);
        assert_eq!(text, URL_NOT_ALLOWED_MSG);
        assert!(!text.contains("169.254"), "leaked target IP: {text:?}");
        assert!(!text.contains("secret"), "leaked target path: {text:?}");
    }

    #[test]
    fn execute_web_fetch_missing_url_param() {
        let result = tokio_test_block(execute_web_fetch(json!({})));
        assert!(get_text(&result).contains("Missing required parameter: url"));
    }

    #[test]
    fn execute_web_fetch_malformed_url_is_generic() {
        let result = tokio_test_block(execute_web_fetch(json!({"url": "not a url"})));
        assert_eq!(get_text(&result), URL_NOT_ALLOWED_MSG);
    }

    // ---- bounds (req VAL-TOOLS-006 web half) --------------------------

    #[test]
    fn web_fetch_bounds_constants_are_sane() {
        // The byte cap and timeout are the bounds VAL-TOOLS-006 requires; pin
        // them so a future edit can't silently disable the cap.
        assert!(WEB_FETCH_BYTE_CAP > 0);
        assert!(WEB_FETCH_TIMEOUT.as_secs() > 0);
        assert!(WEB_FETCH_MAX_REDIRECTS > 0);
    }

    // ---- readable-text extraction (req 6, happy-path code path) --------

    #[test]
    fn html_to_readable_text_strips_tags_and_scripts() {
        let html = "<html><head><style>.x{color:red}</style>\
                    <script>alert(1)</script></head>\
                    <body><h1>Title</h1><p>Hello &amp; welcome</p></body></html>";
        let text = html_to_readable_text(html);
        assert!(text.contains("Title"), "kept heading text: {text:?}");
        assert!(text.contains("Hello & welcome"), "decoded entity: {text:?}");
        assert!(!text.contains("alert"), "dropped script body: {text:?}");
        assert!(!text.contains("color:red"), "dropped style body: {text:?}");
        assert!(!text.contains('<'), "stripped all tags: {text:?}");
    }

    #[test]
    fn html_to_readable_text_collapses_whitespace() {
        let text = html_to_readable_text("<p>a</p>\n\n   <p>b</p>");
        assert_eq!(text, "a b");
    }

    // Minimal single-thread executor so the guard's async wrapper is exercised
    // without pulling in a new test-only crate (tokio is already a dep).
    fn tokio_test_block<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread runtime")
            .block_on(fut)
    }
}
