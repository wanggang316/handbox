// Agent-run extra tools + the working_dir sandbox resolver.
//
// Two things live here:
//
//  1. `resolve_in_sandbox` — the path-containment SECURITY CORE consumed by
//     the approval layer (`agent_permission`). File access itself goes through
//     coding-agent's built-in tools (`select_enabled_tools` in
//     coding_agent_session.rs), but approval gating must decide whether a
//     model-supplied path stays inside the session's `working_dir`; this
//     resolver is that decision. We do NOT reuse hand-ai's path helpers: they
//     have no containment check and they expand `~` to the user's home —
//     exactly the escape this must forbid.
//  2. The presentational tools `render_card` / `render_app`, injected per-run
//     via `extra_tools` (see their section comments below).
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

use std::path::{Component, Path, PathBuf};

use hand_agent::{AgentTool, ToolResult};
use serde_json::json;

/// The render_card tool's fixed name. Always injected for interactive agent
/// runs (rides the `extra_tools` channel alongside MCP tools); the frontend
/// special-cases toolcall blocks with this name into an inline sandbox card.
pub const TOOL_RENDER_CARD: &str = "render_card";
/// The render_app tool's fixed name. Injected like [`TOOL_RENDER_CARD`]; the
/// frontend renders toolcall blocks with this name as a pill that opens the
/// right-side app panel (preview + source view).
pub const TOOL_RENDER_APP: &str = "render_app";

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

// ===========================================================================
// render_card — render a self-contained interactive HTML card to the user.
// ===========================================================================
//
// PRESENTATIONAL tool: execution only validates the arguments and acknowledges.
// The frontend renders the card from the toolcall block's `arguments` (which
// already flow through the message stream and persist with the transcript), so
// no content is transported through the result and no extra IPC exists. The
// sandbox iframe on the frontend blocks all external resources; the description
// teaches the model to inline everything.

/// Hard cap on the `html` argument, protecting the renderer (and the JSONL
/// transcript) from a runaway generation. Cards are small visual aids; 512 KB
/// is far above any legitimate one.
const RENDER_CARD_BYTE_CAP: usize = 512 * 1024;

/// Message for a missing / non-string / empty `html` argument.
const RENDER_CARD_INVALID_ARG_MSG: &str = "invalid html argument: must be a non-empty string";
/// Message when `html` exceeds [`RENDER_CARD_BYTE_CAP`].
const RENDER_CARD_TOO_LARGE_MSG: &str = "html too large: exceeds the 512 KB card limit";

/// Visual-quality doctrine shared by both presentational tools' descriptions.
///
/// Models left to their own devices produce "AI slop": emoji icons, one color
/// per item, gradient headers, cramped stacked boxes. These rules are the
/// distilled counter-doctrine (SVG for structure, meaning-bearing accents,
/// hierarchy from spacing not color) and are what makes generated output look
/// designed; keep them in sync across render_card / render_app.
const VISUAL_DOCTRINE: &str = "\
Visual standard — the output must look designed, not generated:\n\
- ABSOLUTELY NO emoji anywhere in the output: not in the title, headings, \
labels, or nodes. Draw all structure (arrows, connectors, nodes, charts) as \
inline SVG; arrows are SVG paths, never unicode characters; if an icon is \
essential, inline a small SVG.\n\
- Restrained palette: neutral surfaces and text from the theme variables, plus \
at most two accent colors that CARRY MEANING (e.g. gray = plaintext, green = \
encrypted). Never one color per item, never rainbow section headers.\n\
- Hierarchy comes from size, weight and spacing — not from more colors. \
Secondary text is muted (~60% opacity); keep text contrast at least 4.5:1.\n\
- Surfaces: hairline 1px borders (var(--hairline)), subtle rounded corners, \
flat or barely tinted fills. No gradients, no heavy shadows, no decorative \
clutter.\n\
- Generous whitespace on an 8px rhythm. When colors or line styles carry \
meaning, end with one small legend line.\n\
Theme variables are provided and adapt to light/dark automatically: \
--base-100/--base-200/--base-300 (surfaces), --base-content (text), \
--hairline (borders), --primary, --info, --success, --warning, --error.";

/// Build the `render_card` tool.
///
/// The handler is pure argument validation — see the section comment for why
/// execution carries no content. Injected per-run via `extra_tools` so it
/// needs no `working_dir` and no enablement plumbing.
pub fn make_render_card_tool() -> AgentTool {
    AgentTool::simple(
        TOOL_RENDER_CARD,
        format!(
            "Render a self-contained visual inline in the conversation, \
             directly visible to the user. Use it when a picture explains \
             something better than prose.\n\
             \n\
             FIRST pick the representation that matches the information's \
             structure — never default to a stack of colored boxes:\n\
             - message exchange between parties -> sequence diagram (vertical \
             lifelines, SVG arrows between them)\n\
             - process / decision logic -> flowchart\n\
             - hierarchy / composition -> tree or mindmap\n\
             - phases over time -> timeline or Gantt\n\
             - data entities and relations -> ER / UML boxes with relationship \
             lines\n\
             - cause analysis -> fishbone\n\
             - alternatives -> side-by-side comparison\n\
             - quantities -> SVG bar / line chart\n\
             - none of these fit -> a free-form composition designed for this \
             specific content\n\
             \n\
             {VISUAL_DOCTRINE}\n\
             \n\
             The card has NO frame, title bar or background of its own — it \
             renders directly in the chat flow, so the composition must stand \
             on its own. It must fit the container width (no horizontal \
             overflow) and grow naturally in height; never create inner \
             scroll areas or fixed heights with overflow. It runs in a \
             sandboxed iframe with NO network access: inline ALL CSS and \
             JavaScript, and never reference external scripts, stylesheets, \
             fonts, or images. The card is already shown to the user — do \
             not repeat its content in your text reply."
        ),
        json!({
            "type": "object",
            "properties": {
                "html": {
                    "type": "string",
                    "description": "Self-contained HTML for the card body (inline CSS/JS only; no external resources)."
                },
                "title": {
                    "type": "string",
                    "description": "Optional short title; used as the card's accessible name and hover tooltip, not rendered visibly."
                }
            },
            "required": ["html"]
        }),
        "Render card",
        move |_tool_call_id, args| async move { execute_render_card(args) },
    )
}

/// `render_card` body: validate the arguments and acknowledge. The frontend
/// renders from the toolcall block's arguments, so a success result is a plain
/// acknowledgement string.
fn execute_render_card(args: serde_json::Value) -> ToolResult {
    let html = match args.get("html").and_then(|v| v.as_str()) {
        Some(h) if !h.trim().is_empty() => h,
        _ => return ToolResult::error(RENDER_CARD_INVALID_ARG_MSG),
    };

    if html.len() > RENDER_CARD_BYTE_CAP {
        return ToolResult::error(RENDER_CARD_TOO_LARGE_MSG);
    }

    ToolResult::text("Card rendered to the user.")
}

// ===========================================================================
// render_app — build a full HTML app shown in the side preview panel.
// ===========================================================================
//
// Same presentational contract as render_card: execution only validates and
// acknowledges, the frontend derives the app's current state by replaying the
// session's `render_app` toolcall blocks (create sets title+content, update
// replaces them). Content therefore persists with the transcript for free and
// survives reloads without any dedicated storage.

/// Hard cap on the `content` argument. Full applications are larger than
/// inline cards, but 1 MB is still far above any legitimate generation.
const RENDER_APP_BYTE_CAP: usize = 1024 * 1024;

/// Message for a `command` that is not `create` / `update`.
const RENDER_APP_INVALID_COMMAND_MSG: &str = "invalid command: must be \"create\" or \"update\"";
/// Message for a missing / non-string / empty `content` argument.
const RENDER_APP_INVALID_CONTENT_MSG: &str = "invalid content argument: must be a non-empty string";
/// Message for a `create` without a usable `title`.
const RENDER_APP_MISSING_TITLE_MSG: &str =
    "invalid title argument: create requires a non-empty title";
/// Message when `content` exceeds [`RENDER_APP_BYTE_CAP`].
const RENDER_APP_TOO_LARGE_MSG: &str = "content too large: exceeds the 1 MB app limit";

/// Build the `render_app` tool.
///
/// Injected per-run via `extra_tools`, alongside [`make_render_card_tool`].
pub fn make_render_app_tool() -> AgentTool {
    AgentTool::simple(
        TOOL_RENDER_APP,
        format!(
            "Create or update a complete, self-contained HTML application \
             shown to the user in a side panel with a live preview and a \
             source view. Use it for substantial interactive deliverables — \
             dashboards, games, simulations, full pages — while render_card \
             stays for small inline visual aids. `command: \"create\"` starts \
             a new app (requires title and content); `command: \"update\"` \
             replaces the app's content after user feedback (resend the FULL \
             document, not a diff). `content` must be one complete HTML \
             document with ALL CSS and JavaScript inlined: the panel is a \
             sandboxed iframe with NO network access, so never reference \
             external scripts, stylesheets, fonts, or images.\n\
             \n\
             {VISUAL_DOCTRINE}\n\
             \n\
             The panel is already visible to the user — do not repeat the \
             source code in your text reply."
        ),
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["create", "update"],
                    "description": "create: start a new app; update: replace the current app's content."
                },
                "title": {
                    "type": "string",
                    "description": "Short app title shown in the panel header. Required for create; optional for update (keeps the existing title)."
                },
                "content": {
                    "type": "string",
                    "description": "The complete HTML document (inline CSS/JS only; no external resources)."
                }
            },
            "required": ["command", "content"]
        }),
        "Render app",
        move |_tool_call_id, args| async move { execute_render_app(args) },
    )
}

/// `render_app` body: validate the arguments and acknowledge. The frontend
/// renders the panel from the toolcall block's arguments.
fn execute_render_app(args: serde_json::Value) -> ToolResult {
    let command = match args.get("command").and_then(|v| v.as_str()) {
        Some(c @ ("create" | "update")) => c,
        _ => return ToolResult::error(RENDER_APP_INVALID_COMMAND_MSG),
    };

    let content = match args.get("content").and_then(|v| v.as_str()) {
        Some(c) if !c.trim().is_empty() => c,
        _ => return ToolResult::error(RENDER_APP_INVALID_CONTENT_MSG),
    };

    if content.len() > RENDER_APP_BYTE_CAP {
        return ToolResult::error(RENDER_APP_TOO_LARGE_MSG);
    }

    let has_title = args
        .get("title")
        .and_then(|v| v.as_str())
        .is_some_and(|t| !t.trim().is_empty());
    if command == "create" && !has_title {
        return ToolResult::error(RENDER_APP_MISSING_TITLE_MSG);
    }

    match command {
        "create" => ToolResult::text("App created and shown to the user in the preview panel."),
        _ => ToolResult::text("App updated in the preview panel."),
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

    /// A sandbox root plus a secret file OUTSIDE it (escape-vector target).
    struct Fixture {
        _outer: TempDir,
        root: PathBuf,
        outside_secret: PathBuf,
    }

    fn fixture() -> Fixture {
        let outer = TempDir::new().unwrap();
        let root = outer.path().join("proj");
        fs::create_dir(&root).unwrap();
        // A real subdir so `sub/../..` traversal resolves (and is then
        // rejected by containment, not as an unresolvable path).
        fs::create_dir(root.join("sub")).unwrap();

        // A secret file OUTSIDE the sandbox, as a sibling of the root.
        let outside_secret = outer.path().join("secret.txt");
        fs::write(&outside_secret, "TOP SECRET CONTENT").unwrap();

        Fixture {
            _outer: outer,
            root,
            outside_secret,
        }
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
        // The model-facing message stays generic: no path, no contents.
        assert_eq!(err.display_message(), SANDBOX_VIOLATION_MSG);
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

        // Absolute path to the real outside secret is rejected too.
        let abs = fx.outside_secret.to_string_lossy().into_owned();
        assert_eq!(
            resolve_in_sandbox(&fx.root, &abs).unwrap_err(),
            SandboxError::OutsideSandbox
        );
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

    // --- render_card ---

    #[test]
    fn render_card_valid_html_acknowledges() {
        let result = execute_render_card(json!({"html": "<div>hello</div>"}));
        assert_eq!(get_text(&result), "Card rendered to the user.");
    }

    #[test]
    fn render_card_title_is_optional_and_ignored_by_execution() {
        let result = execute_render_card(json!({"html": "<p>x</p>", "title": "Demo"}));
        assert_eq!(get_text(&result), "Card rendered to the user.");
    }

    #[test]
    fn render_card_bad_html_arg_yields_invalid_arg_error() {
        // Missing, non-string, empty, and whitespace-only all hit the same guard.
        assert_eq!(
            get_text(&execute_render_card(json!({}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
        assert_eq!(
            get_text(&execute_render_card(json!({"html": 42}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
        assert_eq!(
            get_text(&execute_render_card(json!({"html": ""}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
        assert_eq!(
            get_text(&execute_render_card(json!({"html": "   "}))),
            RENDER_CARD_INVALID_ARG_MSG
        );
    }

    #[test]
    fn render_card_oversized_html_is_rejected() {
        let oversized = "x".repeat(RENDER_CARD_BYTE_CAP + 1);
        let result = execute_render_card(json!({ "html": oversized }));
        assert_eq!(get_text(&result), RENDER_CARD_TOO_LARGE_MSG);
    }

    /// The tool resolves via the registered `execute` closure (end-to-end
    /// through `AgentTool`), proving the wiring like the skill tool test above.
    #[test]
    fn render_card_tool_execute_closure_resolves() {
        let tool = make_render_card_tool();
        assert_eq!(tool.name, TOOL_RENDER_CARD);
        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-rc".to_string(),
            args: json!({"html": "<b>ok</b>"}),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let result = tokio_test_block((tool.execute)(ctx)).expect("execute ok");
        assert_eq!(get_text(&result), "Card rendered to the user.");
    }

    // --- render_app ---

    #[test]
    fn render_app_create_acknowledges() {
        let result = execute_render_app(json!({
            "command": "create",
            "title": "Demo App",
            "content": "<!DOCTYPE html><html><body>hi</body></html>"
        }));
        assert_eq!(
            get_text(&result),
            "App created and shown to the user in the preview panel."
        );
    }

    #[test]
    fn render_app_update_acknowledges_without_title() {
        let result = execute_render_app(json!({
            "command": "update",
            "content": "<!DOCTYPE html><html><body>v2</body></html>"
        }));
        assert_eq!(get_text(&result), "App updated in the preview panel.");
    }

    #[test]
    fn render_app_bad_command_yields_invalid_command_error() {
        // Missing, non-string, and out-of-enum all hit the same guard.
        assert_eq!(
            get_text(&execute_render_app(json!({"content": "<p>x</p>"}))),
            RENDER_APP_INVALID_COMMAND_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": 1, "content": "<p>x</p>"})
            )),
            RENDER_APP_INVALID_COMMAND_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "delete", "content": "<p>x</p>"})
            )),
            RENDER_APP_INVALID_COMMAND_MSG
        );
    }

    #[test]
    fn render_app_bad_content_yields_invalid_content_error() {
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "title": "T"})
            )),
            RENDER_APP_INVALID_CONTENT_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "title": "T", "content": "   "})
            )),
            RENDER_APP_INVALID_CONTENT_MSG
        );
    }

    #[test]
    fn render_app_create_without_title_is_rejected() {
        // Missing, empty, and whitespace-only titles all fail create...
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "content": "<p>x</p>"})
            )),
            RENDER_APP_MISSING_TITLE_MSG
        );
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "create", "title": "  ", "content": "<p>x</p>"})
            )),
            RENDER_APP_MISSING_TITLE_MSG
        );
        // ...while update tolerates a missing title (keeps the existing one).
        assert_eq!(
            get_text(&execute_render_app(
                json!({"command": "update", "content": "<p>x</p>"})
            )),
            "App updated in the preview panel."
        );
    }

    #[test]
    fn render_app_oversized_content_is_rejected() {
        let oversized = "x".repeat(RENDER_APP_BYTE_CAP + 1);
        let result = execute_render_app(json!({
            "command": "update",
            "content": oversized
        }));
        assert_eq!(get_text(&result), RENDER_APP_TOO_LARGE_MSG);
    }

    #[test]
    fn render_app_tool_execute_closure_resolves() {
        let tool = make_render_app_tool();
        assert_eq!(tool.name, TOOL_RENDER_APP);
        let ctx = hand_agent::ToolExecuteCtx {
            tool_call_id: "tc-ra".to_string(),
            args: json!({"command": "create", "title": "T", "content": "<b>ok</b>"}),
            cancel: hand_agent::CancellationToken::new(),
            on_update: std::sync::Arc::new(|_: ToolResult| {}),
        };
        let result = tokio_test_block((tool.execute)(ctx)).expect("execute ok");
        assert_eq!(
            get_text(&result),
            "App created and shown to the user in the preview panel."
        );
    }
}
