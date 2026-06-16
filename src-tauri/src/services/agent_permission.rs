//! agent_permission — HandBox-side permission / boundary extension for the
//! coding-agent's `before_tool_call` hook chain.
//!
//! WHY THIS EXISTS
//! ----------------
//! The vendored coding agent (`hand-ai`) does NOT sandbox its file tools: a
//! `read`/`ls` (and `write`/`edit`) call resolves an absolute path as-is and
//! expands a leading `~` to `$HOME` — exactly the escape HandBox must forbid.
//! Rather than fork the upstream tools, HandBox re-imposes the `working_dir`
//! boundary from the OUTSIDE: a Tier-1 [`Extension`] registered on the session
//! inspects each tool call in `on_before_tool_call` and [`HookDecision::Cancel`]s
//! any call whose path argument resolves outside the session's working
//! directory. `Cancel` is the only gate that actually stops a tool from
//! running — the tool never executes and the model receives an error result —
//! so out-of-sandbox content is never read out (D14 / VAL-CATOOLS-014).
//!
//! SCOPE OF THIS FEATURE
//! ----------------------
//! This feature enforces the boundary for the READ-ONLY tools `read` and `ls`
//! only. `write`/`edit` get the same path boundary in a later milestone (M2),
//! and `bash` is intentionally NOT path-sandboxed here (it is gated by the
//! approval flow, not by path containment). The deny/approval surface
//! (m1-dangerous-deny-stub, M2 approval) layers ON TOP of this extension —
//! see [`SandboxExtension`] for the extension points designed for that reuse.
//!
//! HOW THE BOUNDARY IS JUDGED
//! --------------------------
//! Path containment reuses [`agent_tools::resolve_in_sandbox`], HandBox's
//! existing strict resolver: it rejects `~`, collapses `..`, canonicalizes
//! (resolving symlinks), and verifies component-wise containment under the
//! canonical root. The `Cancel` reason is GENERIC — it never echoes the
//! out-of-sandbox absolute path — matching the resolver's leak-free contract.

use std::path::PathBuf;

use async_trait::async_trait;
use hand_coding_agent::core::extensions::api::{
    ExtensionCapabilities, ToolCallEvent, ToolResultEvent,
};
use hand_coding_agent::{
    Extension, ExtensionContext, ExtensionError, ExtensionManifest, HookDecision,
};

use crate::services::agent_tools::resolve_in_sandbox;

/// Stable extension name, used in diagnostics and the manifest.
const EXTENSION_NAME: &str = "handbox-sandbox";

/// Generic, leak-free reason returned to the model when a tool call's path
/// argument resolves outside the working directory. MUST NOT echo the
/// offending absolute path (D14) — the resolver enforces the same discipline
/// for the tool-result message, and we mirror it here for the hook reason.
const OUT_OF_SANDBOX_REASON: &str = "blocked: path is outside the working directory";

/// The read-only tools whose `path` argument this feature confines to the
/// working directory. `write`/`edit` are added by the M2 boundary; `bash` is
/// never path-sandboxed (it is approval-gated instead).
///
/// Both `read` and `ls` declare a string `path` parameter resolved relative to
/// the cwd. `ls` may omit `path` (it then lists the cwd itself, which is in
/// bounds), so a missing/non-string `path` is treated as in-bounds rather than
/// a violation.
const PATH_SANDBOXED_TOOLS: &[&str] = &["read", "ls"];

/// Tier-1 extension that re-imposes the `working_dir` sandbox boundary on the
/// coding agent's read-only file tools via the `before_tool_call` hook.
///
/// EXTENSIBILITY (the M1 deny-stub / M2 approval hand-off point):
/// - The extension captures the session's `working_dir` at construction, so
///   every hook invocation judges against a stable, known root rather than
///   trusting per-event context.
/// - The "which tools are path-confined" decision is a single table
///   ([`PATH_SANDBOXED_TOOLS`]); widening it to `write`/`edit` for M2 is a
///   one-line change plus their (identical) path-argument extraction.
/// - The per-call decision is factored into [`SandboxExtension::decide`], a
///   pure-ish function over `(tool_name, arguments)` returning a
///   [`HookDecision`]. A deny-stub or approval extension can be registered as
///   an ADDITIONAL extension in the same chain (the host calls every
///   registered extension in order and the FIRST `Cancel` wins), so it never
///   has to modify this type — it composes alongside it.
pub struct SandboxExtension {
    manifest: ExtensionManifest,
    /// The session's working directory: the sandbox root every path argument
    /// is confined to. Captured at registration so the boundary does not
    /// depend on mutable per-event context.
    working_dir: PathBuf,
}

impl SandboxExtension {
    /// Construct a sandbox extension bound to `working_dir` (the session cwd).
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            manifest: ExtensionManifest {
                name: EXTENSION_NAME.to_string(),
                version: "0.1.0".to_string(),
                description: Some(
                    "Confines read-only file tools (read/ls) to the session working directory."
                        .to_string(),
                ),
                capabilities: ExtensionCapabilities {
                    before_tool_call: true,
                    ..Default::default()
                },
                exec: None,
                env: Default::default(),
                slash_commands: Vec::new(),
                custom_tools: Vec::new(),
            },
            working_dir,
        }
    }

    /// Decide whether a tool call may proceed under the sandbox boundary.
    ///
    /// Pure over `(tool_name, arguments)` + the captured `working_dir`, so it
    /// is unit-testable without a live session. Tools not in
    /// [`PATH_SANDBOXED_TOOLS`] always [`HookDecision::Continue`]. For a
    /// confined tool, the `path` argument (when present and a string) is run
    /// through [`resolve_in_sandbox`]: an `Err` (escape / invalid / unresolved)
    /// becomes a generic [`HookDecision::Cancel`]; an in-bounds path or an
    /// absent/non-string `path` (e.g. `ls` of the cwd) is allowed.
    fn decide(&self, tool_name: &str, arguments: &serde_json::Value) -> HookDecision {
        if !PATH_SANDBOXED_TOOLS.contains(&tool_name) {
            return HookDecision::Continue;
        }

        // Extract the `path` argument. A missing / non-string `path` is not a
        // boundary violation: `ls` legitimately omits it (lists the cwd), and
        // `read` without `path` is a parameter error the tool reports itself.
        let path = match arguments.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return HookDecision::Continue,
        };

        match resolve_in_sandbox(&self.working_dir, path) {
            Ok(_) => HookDecision::Continue,
            // Generic reason — never echo the offending path (D14).
            Err(_) => HookDecision::Cancel(OUT_OF_SANDBOX_REASON.to_string()),
        }
    }
}

#[async_trait]
impl Extension for SandboxExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    async fn on_before_tool_call(
        &self,
        _cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> Result<HookDecision, ExtensionError> {
        Ok(self.decide(&event.tool_name, &event.arguments))
    }

    async fn on_after_tool_call(
        &self,
        _cx: &ExtensionContext,
        _event: &ToolResultEvent,
    ) -> Result<(), ExtensionError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// A sandbox root (`proj`) with an in-bounds file, plus a sibling secret
    /// OUTSIDE the root. Mirrors the `agent_tools` fixture so the extension is
    /// exercised against the same escape geometry the resolver defends.
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

        let outside_secret = outer.path().join("secret.txt");
        fs::write(&outside_secret, "TOP SECRET CONTENT").unwrap();

        Fixture {
            _outer: outer,
            root,
            outside_secret,
        }
    }

    fn ext(root: &Path) -> SandboxExtension {
        SandboxExtension::new(root.to_path_buf())
    }

    fn call_event(tool_name: &str, args: serde_json::Value) -> ToolCallEvent {
        ToolCallEvent {
            tool_name: tool_name.to_string(),
            arguments: args,
            call_id: "call-1".to_string(),
        }
    }

    fn cx(root: &Path) -> ExtensionContext {
        ExtensionContext {
            cwd: root.to_path_buf(),
            session_id: "test-session".to_string(),
            data_dir: root.join(".hand").join("data"),
        }
    }

    /// Drive the real hook path (async `on_before_tool_call`) rather than
    /// `decide` directly, so the test exercises the same entry point the host
    /// dispatch chain calls.
    async fn decide_via_hook(
        ext: &SandboxExtension,
        root: &Path,
        tool: &str,
        args: serde_json::Value,
    ) -> HookDecision {
        ext.on_before_tool_call(&cx(root), &call_event(tool, args))
            .await
            .expect("sandbox hook never returns Err")
    }

    fn assert_cancel_no_leak(decision: &HookDecision, abs_path: &Path) {
        match decision {
            HookDecision::Cancel(reason) => assert!(
                !reason.contains(&*abs_path.to_string_lossy()),
                "cancel reason leaked the out-of-sandbox absolute path: {reason:?}"
            ),
            other => panic!("expected Cancel, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // VAL-CATOOLS-014 — read/ls escaping working_dir via an absolute path or
    // `~` is cancelled by the sandbox; in-bounds paths are allowed.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn read_absolute_outside_path_is_cancelled() {
        let fx = fixture();
        let abs = fx.outside_secret.to_string_lossy().into_owned();
        let decision =
            decide_via_hook(&ext(&fx.root), &fx.root, "read", json!({ "path": abs })).await;
        assert_cancel_no_leak(&decision, &fx.outside_secret);
    }

    #[tokio::test]
    async fn read_system_absolute_path_is_cancelled() {
        let fx = fixture();
        // A classic out-of-cwd absolute target.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "read",
            json!({ "path": "/etc/hosts" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn read_tilde_path_is_cancelled_not_expanded() {
        let fx = fixture();
        // `~/...` must be REFUSED at the boundary, never expanded to $HOME —
        // upstream `read` would expand it; the sandbox stops it first.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "read",
            json!({ "path": "~/secret.txt" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn read_dotdot_traversal_is_cancelled() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "read",
            json!({ "path": "../secret.txt" }),
        )
        .await;
        assert_cancel_no_leak(&decision, &fx.outside_secret);
    }

    #[tokio::test]
    async fn ls_absolute_outside_path_is_cancelled() {
        let fx = fixture();
        let outside_dir = fx.outside_secret.parent().unwrap().to_path_buf();
        let abs = outside_dir.to_string_lossy().into_owned();
        let decision =
            decide_via_hook(&ext(&fx.root), &fx.root, "ls", json!({ "path": abs })).await;
        assert_cancel_no_leak(&decision, &outside_dir);
    }

    #[tokio::test]
    async fn ls_tilde_path_is_cancelled() {
        let fx = fixture();
        let decision =
            decide_via_hook(&ext(&fx.root), &fx.root, "ls", json!({ "path": "~" })).await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn read_inside_relative_path_continues() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "read",
            json!({ "path": "inside.txt" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn read_inside_absolute_path_continues() {
        let fx = fixture();
        // An absolute path that genuinely lives inside the root is allowed.
        let abs = fx.root.join("inside.txt").to_string_lossy().into_owned();
        let decision =
            decide_via_hook(&ext(&fx.root), &fx.root, "read", json!({ "path": abs })).await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn ls_inside_subdir_continues() {
        let fx = fixture();
        let decision =
            decide_via_hook(&ext(&fx.root), &fx.root, "ls", json!({ "path": "sub" })).await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn ls_without_path_continues() {
        let fx = fixture();
        // `ls` may omit `path` to list the cwd; that is in-bounds, not a
        // violation.
        let decision = decide_via_hook(&ext(&fx.root), &fx.root, "ls", json!({})).await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn non_sandboxed_tool_is_always_continued() {
        let fx = fixture();
        // `bash` is NOT path-sandboxed here even with a path-shaped arg; and an
        // unrelated tool likewise passes through untouched.
        for tool in ["bash", "grep", "write", "edit"] {
            let decision = decide_via_hook(
                &ext(&fx.root),
                &fx.root,
                tool,
                json!({ "path": "/etc/passwd", "command": "rm -rf /" }),
            )
            .await;
            assert!(
                matches!(decision, HookDecision::Continue),
                "{tool} must pass through the read/ls-only sandbox"
            );
        }
    }

    #[test]
    fn manifest_declares_before_tool_call_capability() {
        let ext = SandboxExtension::new(PathBuf::from("/tmp"));
        let m = ext.manifest();
        assert_eq!(m.name, EXTENSION_NAME);
        assert!(
            m.capabilities.before_tool_call,
            "the sandbox must declare the before_tool_call capability"
        );
    }
}
