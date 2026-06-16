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
//! This feature enforces the boundary for ALL of the READ-ONLY path tools:
//! `read`, `ls`, `grep`, and `find`. They share one `path` argument and one
//! containment rule, so they ride a single sandbox table. `write`/`edit` get
//! the same path boundary in a later milestone (M2), and `bash` is
//! intentionally NOT path-sandboxed here (it is gated by the approval flow,
//! not by path containment). The deny/approval surface
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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use hand_coding_agent::core::extensions::api::{
    ExtensionCapabilities, ToolCallEvent, ToolResultEvent,
};
use hand_coding_agent::{
    Extension, ExtensionContext, ExtensionError, ExtensionManifest, HookDecision,
};
use tokio::sync::oneshot;

use crate::services::agent_tools::resolve_in_sandbox;

/// Emitter handle the [`PermissionExtension`] uses to push an approval request
/// to the frontend. Constructed by the IPC layer to wrap `window.emit(
/// "agent_approval_request", ..)`; a unit test injects a recording fake. When
/// absent (no UI to consult — e.g. a test or a headless construction) the
/// extension fails CLOSED: every dangerous tool is denied (the M1 safety
/// posture), never silently allowed.
pub type ApprovalEmitter = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// Stable extension name, used in diagnostics and the manifest.
const EXTENSION_NAME: &str = "handbox-sandbox";

/// Stable name of the M1 dangerous-tool deny stub, used in diagnostics and the
/// manifest. Kept distinct from [`EXTENSION_NAME`] so both extensions coexist
/// in the same hook chain (see [`DangerousDenyExtension`]).
const DANGEROUS_DENY_EXTENSION_NAME: &str = "handbox-dangerous-deny";

/// Generic, leak-free reason returned to the model when a tool call's path
/// argument resolves outside the working directory. MUST NOT echo the
/// offending absolute path (D14) — the resolver enforces the same discipline
/// for the tool-result message, and we mirror it here for the hook reason.
const OUT_OF_SANDBOX_REASON: &str = "blocked: path is outside the working directory";

/// The read-only tools whose `path` argument this feature confines to the
/// working directory. `write`/`edit` are added by the M2 boundary; `bash` is
/// never path-sandboxed (it is approval-gated instead).
///
/// All four declare the SAME string `path` parameter resolved relative to the
/// cwd (verified against the coding-agent tool schemas: read/ls, and
/// grep/find's `path` arg — "Directory or file to search in (default: cwd)").
/// `ls`/`grep`/`find` may omit `path` (they then operate on the cwd itself,
/// which is in bounds), so a missing/non-string `path` is treated as in-bounds
/// rather than a violation. `grep`/`find` are confined here because the
/// upstream tools apply NO containment of their own: an absolute `path` is used
/// as-is and a leading `~` would expand to `$HOME`, letting an unsandboxed
/// `grep`/`find` read file contents / list filenames anywhere on disk — the
/// exact escape this boundary forbids for `read`/`ls`.
const PATH_SANDBOXED_TOOLS: &[&str] = &["read", "ls", "grep", "find"];

/// The dangerous, side-effecting tools the M1 deny stub blocks outright:
/// `write`/`edit` mutate the filesystem and `bash` runs arbitrary subprocesses.
/// Until the M2 approval UI exists there is no safe way to consent to these, so
/// every call is denied. The read-only tools (`read`/`ls`/`grep`/`find`) are
/// absent from this deny table and pass straight through it — but they are NOT
/// unguarded: each is path-confined to the working directory by
/// [`SandboxExtension`] / [`PATH_SANDBOXED_TOOLS`], so an out-of-cwd target is
/// still `Cancel`led there.
const DANGEROUS_TOOLS: &[&str] = &["write", "edit", "bash"];

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
                    "Confines read-only file tools (read/ls/grep/find) to the session working directory."
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
    /// absent/non-string `path` (e.g. `ls`/`grep`/`find` over the cwd) is
    /// allowed.
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

/// Tier-1 extension that denies the dangerous, side-effecting tools
/// (`write`/`edit`/`bash`) for the M1 milestone via the `before_tool_call` hook.
///
/// WHY DENY OUTRIGHT (the M1 stub):
/// M1 ships no approval surface, so there is no safe way for the user to consent
/// to a filesystem mutation or a subprocess. Rather than let these run
/// unguarded, every call is [`HookDecision::Cancel`]ed — the only gate that
/// actually stops a tool: the tool never executes (no file written/edited, no
/// subprocess spawned) and the model receives an error result instead. This is
/// the deny half of D14 / VAL-CATOOLS-013.
///
/// COMPOSITION:
/// This extension is registered ALONGSIDE [`SandboxExtension`] in
/// [`crate::services::coding_agent_session::build_agent_session`], not in place
/// of it. The host dispatches every registered extension in registration order
/// and the FIRST `Cancel` wins, so the two compose without either knowing about
/// the other: the sandbox confines read-only paths, this stub blocks the
/// dangerous tools.
///
/// M2 HAND-OFF:
/// M2 REPLACES this stub with an approval extension that, instead of an
/// unconditional `Cancel`, prompts the user and awaits a decision (deny → the
/// same `Cancel`; allow → `Continue`). The reason wording here already speaks
/// the "requires approval / not yet available" language so the M1 → M2
/// transition is a behavioral swap (await a decision) rather than a vocabulary
/// change. The decision is factored into [`DangerousDenyExtension::decide`], the
/// single point M2 grows an `await` against.
pub struct DangerousDenyExtension {
    manifest: ExtensionManifest,
}

impl DangerousDenyExtension {
    /// Construct the M1 dangerous-tool deny stub. It holds no per-session state:
    /// the deny is unconditional, judged purely from the tool name.
    pub fn new() -> Self {
        Self {
            manifest: ExtensionManifest {
                name: DANGEROUS_DENY_EXTENSION_NAME.to_string(),
                version: "0.1.0".to_string(),
                description: Some(
                    "Denies dangerous tools (write/edit/bash) until the M2 approval flow exists."
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
        }
    }

    /// Decide whether a tool call may proceed under the M1 deny stub.
    ///
    /// Pure over `tool_name`, so it is unit-testable without a live session.
    /// A tool in [`DANGEROUS_TOOLS`] is [`HookDecision::Cancel`]ed with a reason
    /// that carries the "requires approval (not yet available)" semantics M2
    /// builds on; every other tool [`HookDecision::Continue`]s untouched (the
    /// sandbox extension still judges read-only paths separately).
    fn decide(&self, tool_name: &str) -> HookDecision {
        if DANGEROUS_TOOLS.contains(&tool_name) {
            HookDecision::Cancel(format!("{tool_name} requires approval (not yet available)"))
        } else {
            HookDecision::Continue
        }
    }
}

impl Default for DangerousDenyExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Extension for DangerousDenyExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    async fn on_before_tool_call(
        &self,
        _cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> Result<HookDecision, ExtensionError> {
        Ok(self.decide(&event.tool_name))
    }

    async fn on_after_tool_call(
        &self,
        _cx: &ExtensionContext,
        _event: &ToolResultEvent,
    ) -> Result<(), ExtensionError> {
        Ok(())
    }
}

/// Stable name of the M2 permission extension, used in diagnostics and the
/// manifest. Distinct from [`EXTENSION_NAME`] / [`DANGEROUS_DENY_EXTENSION_NAME`]
/// so it coexists in the hook chain.
const PERMISSION_EXTENSION_NAME: &str = "handbox-permission";

/// Tauri event name the frontend listens on for an approval request. Carries
/// `{ sessionId, callId, toolName, args, requestId }`. The frontend (m2-approval
/// -modal) renders the prompt and answers via the `agent_approval_respond` IPC.
pub const APPROVAL_REQUEST_EVENT: &str = "agent_approval_request";

/// Process-level `request_id → oneshot::Sender<bool>` registry of approval
/// decisions still awaiting a user answer.
///
/// WHY PROCESS-LEVEL (mirrors `coding_agent_runtime::run_controls`):
/// the `PermissionExtension` is owned by the `AgentSession`, which the driver
/// task owns for the turn — there is no instance-level place to hang per-request
/// state the stateless `agent_approval_respond` command can reach. A
/// `OnceLock<Mutex<HashMap<..>>>` gives both the extension (insert + await) and
/// the command (remove + send) a shared rendezvous keyed by `request_id`.
///
/// LIFECYCLE: `on_before_tool_call` inserts a sender then awaits the matching
/// receiver; [`respond_to_approval`] removes the sender and `send`s the
/// decision, waking the await. A `request_id` is a fresh uuid, so entries never
/// collide. If the receiver is dropped before a response arrives (e.g. the run
/// is aborted), `respond` finds no entry — a clean no-op.
fn pending_approvals() -> &'static Mutex<HashMap<String, oneshot::Sender<bool>>> {
    static PENDING: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve a pending approval: wake the awaiting `on_before_tool_call` with the
/// user's `allow` decision.
///
/// IDEMPOTENT / leak-free: the sender is REMOVED from the registry before being
/// used, so the FIRST response for a `request_id` wins and a duplicate or
/// unknown `request_id` is a clean no-op (nothing to remove, nothing to send).
/// A send failure (the receiver was already dropped — the awaiting tool call
/// was abandoned) is likewise ignored: there is no live awaiter to wake.
///
/// This is the body the `agent_approval_respond` IPC command delegates to; it is
/// `pub` so the command (and unit tests) can drive the rendezvous without a live
/// session.
pub fn respond_to_approval(request_id: &str, allow: bool) {
    let sender = pending_approvals().lock().unwrap().remove(request_id);
    if let Some(sender) = sender {
        // The receiver may already be gone (run aborted); a failed send means
        // there is no awaiter to wake, which is fine.
        let _ = sender.send(allow);
    }
}

/// Tier-1 extension that gates the dangerous, side-effecting tools
/// (`write`/`edit`/`bash`) behind an ASYNCHRONOUS user approval — the M2
/// replacement for the M1 [`DangerousDenyExtension`] unconditional deny.
///
/// FLOW (`on_before_tool_call` for a dangerous tool):
/// 1. mint a fresh `request_id` (uuid v4);
/// 2. register a `oneshot::Sender<bool>` under it in [`pending_approvals`];
/// 3. emit `agent_approval_request` `{ sessionId, callId, toolName, args,
///    requestId }` through the injected [`ApprovalEmitter`] so the frontend can
///    prompt the user;
/// 4. `await` the matching receiver. `Ok(true)` → [`HookDecision::Continue`]
///    (the tool runs); `Ok(false)` (the user denied) or `Err` (the sender was
///    dropped) → [`HookDecision::Cancel`] with a reason that speaks the
///    "denied / 被拒" semantics so the model can word its reply.
///
/// FAIL-CLOSED: when no emitter is wired (no approval UI — e.g. a unit test or a
/// headless construction) the extension does NOT await; it denies outright,
/// preserving the M1 safety posture that a dangerous tool never runs without an
/// explicit consent surface.
///
/// COMPOSITION & ORDERING: registered ALONGSIDE [`SandboxExtension`], AFTER it,
/// in [`crate::services::coding_agent_session::build_agent_session`]. The host
/// dispatches every registered extension in order and the FIRST `Cancel` wins,
/// so a sandbox escape (an out-of-cwd read/ls/grep/find) is `Cancel`led by the
/// sandbox FIRST and never reaches — never prompts — this approval gate. Only a
/// dangerous tool that clears the sandbox surfaces an approval request.
pub struct PermissionExtension {
    manifest: ExtensionManifest,
    /// Emitter pushing `agent_approval_request` to the frontend. `None` →
    /// fail-closed (deny every dangerous tool); see the type-level doc.
    emitter: Option<ApprovalEmitter>,
}

impl PermissionExtension {
    /// Construct a permission extension.
    ///
    /// `emitter` is the approval-request channel: `Some` wires the frontend
    /// prompt + await; `None` makes the extension fail closed (deny dangerous
    /// tools), the safe default when there is no UI to consult.
    pub fn new(emitter: Option<ApprovalEmitter>) -> Self {
        Self {
            manifest: ExtensionManifest {
                name: PERMISSION_EXTENSION_NAME.to_string(),
                version: "0.1.0".to_string(),
                description: Some(
                    "Gates dangerous tools (write/edit/bash) behind an async user approval."
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
            emitter,
        }
    }

    /// Request approval for one dangerous tool call and await the decision.
    ///
    /// Returns the resolved [`HookDecision`]: `Continue` on allow, `Cancel` on
    /// deny / fail-closed. Factored out of the trait method so the rendezvous is
    /// directly unit-testable. A non-dangerous tool never reaches here — the
    /// caller short-circuits it to `Continue`.
    async fn request_approval(
        &self,
        session_id: &str,
        call_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> HookDecision {
        // No approval surface → fail closed (M1 safety posture). Never await,
        // never run the tool.
        let Some(emitter) = &self.emitter else {
            return HookDecision::Cancel(deny_reason(tool_name));
        };

        // Mint a fresh request id and register the wake channel BEFORE emitting,
        // so a response that races back the instant the event is delivered
        // always finds a live entry to resolve.
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<bool>();
        pending_approvals()
            .lock()
            .unwrap()
            .insert(request_id.clone(), tx);

        emitter(serde_json::json!({
            "sessionId": session_id,
            "callId": call_id,
            "toolName": tool_name,
            "args": arguments,
            "requestId": request_id,
        }));

        // Await the user's decision. `Ok(true)` → allow; `Ok(false)` → deny;
        // `Err` (sender dropped without sending) → deny. On any deny path the
        // registry entry is already gone (removed by `respond_to_approval`), or
        // we remove it here to avoid leaking an orphaned sender if the receiver
        // resolved via `Err`.
        match rx.await {
            Ok(true) => HookDecision::Continue,
            Ok(false) => HookDecision::Cancel(deny_reason(tool_name)),
            Err(_) => {
                // Sender dropped before answering (e.g. run aborted): clean up
                // any lingering entry and deny.
                pending_approvals().lock().unwrap().remove(&request_id);
                HookDecision::Cancel(deny_reason(tool_name))
            }
        }
    }
}

/// The denial reason returned to the model when a dangerous tool is rejected or
/// runs without an approval surface. Carries the "denied / 被拒" semantics so
/// the model can word its reply (e.g. tell the user the action was refused).
fn deny_reason(tool_name: &str) -> String {
    format!("用户拒绝了 {tool_name}（denied）")
}

#[async_trait]
impl Extension for PermissionExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    async fn on_before_tool_call(
        &self,
        cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> Result<HookDecision, ExtensionError> {
        // Only the dangerous, side-effecting tools are approval-gated; read-only
        // / non-dangerous tools pass straight through (the sandbox judges their
        // paths separately, earlier in the chain).
        if !DANGEROUS_TOOLS.contains(&event.tool_name.as_str()) {
            return Ok(HookDecision::Continue);
        }
        Ok(self
            .request_approval(
                &cx.session_id,
                &event.call_id,
                &event.tool_name,
                &event.arguments,
            )
            .await)
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
        // `bash`/`write`/`edit` are NOT path-sandboxed here even with a
        // path-shaped arg (the deny stub gates them instead); an unrelated
        // tool likewise passes through untouched. `grep`/`find` are deliberately
        // ABSENT — they ARE path-confined now, see the grep/find tests below.
        for tool in ["bash", "write", "edit"] {
            let decision = decide_via_hook(
                &ext(&fx.root),
                &fx.root,
                tool,
                json!({ "path": "/etc/passwd", "command": "rm -rf /" }),
            )
            .await;
            assert!(
                matches!(decision, HookDecision::Continue),
                "{tool} must pass through the read/ls/grep/find path sandbox"
            );
        }
    }

    // -----------------------------------------------------------------------
    // VAL-CATOOLS-014 (grep/find coverage) — `grep`/`find` share the SAME
    // `path` argument and containment rule as `read`/`ls`. The upstream tools
    // apply no containment (absolute path used as-is, `~` expands to $HOME), so
    // without this boundary an injected `grep ~/.ssh/...` or `find /` would
    // read out-of-cwd file contents / filenames. Confining them here makes the
    // out-of-sandbox target `Cancel`led (never executed) just like read/ls.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn grep_absolute_outside_path_is_cancelled() {
        let fx = fixture();
        // `grep`'s `path` is "Directory or file to search in" — an absolute
        // out-of-cwd target (e.g. searching ~/.aws) must be refused so the
        // model never reads file CONTENTS outside the sandbox.
        let outside_dir = fx.outside_secret.parent().unwrap().to_path_buf();
        let abs = outside_dir.to_string_lossy().into_owned();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "grep",
            json!({ "pattern": "SECRET", "path": abs }),
        )
        .await;
        assert_cancel_no_leak(&decision, &outside_dir);
    }

    #[tokio::test]
    async fn grep_system_absolute_path_is_cancelled() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "grep",
            json!({ "pattern": "root", "path": "/etc" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn grep_tilde_path_is_cancelled_not_expanded() {
        let fx = fixture();
        // `~/...` must be REFUSED at the boundary, never expanded to $HOME.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "grep",
            json!({ "pattern": "id_rsa", "path": "~/.ssh" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn grep_without_path_continues() {
        let fx = fixture();
        // Omitting `path` defaults to the cwd (in bounds), so the call is
        // allowed to proceed.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "grep",
            json!({ "pattern": "hello" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn grep_inside_path_continues() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "grep",
            json!({ "pattern": "hello", "path": "sub" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn find_absolute_outside_path_is_cancelled() {
        let fx = fixture();
        // `find`'s `path` is "Directory to search in" — an absolute out-of-cwd
        // target must be refused so the model never lists FILENAMES outside the
        // sandbox.
        let outside_dir = fx.outside_secret.parent().unwrap().to_path_buf();
        let abs = outside_dir.to_string_lossy().into_owned();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "find",
            json!({ "pattern": "**/*", "path": abs }),
        )
        .await;
        assert_cancel_no_leak(&decision, &outside_dir);
    }

    #[tokio::test]
    async fn find_tilde_path_is_cancelled_not_expanded() {
        let fx = fixture();
        // `~` must be refused, never expanded to $HOME (upstream `find` would
        // expand it; the sandbox stops it first).
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "find",
            json!({ "pattern": "**/*.key", "path": "~" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn find_without_path_continues() {
        let fx = fixture();
        // Omitting `path` defaults to the cwd (in bounds).
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "find",
            json!({ "pattern": "**/*.txt" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn find_inside_path_continues() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "find",
            json!({ "pattern": "**/*", "path": "sub" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[test]
    fn path_sandboxed_tools_cover_all_read_only_tools() {
        // The four read-only path tools share one containment rule and ride one
        // table — pin the exact set so a regression (e.g. dropping grep/find,
        // the original Critical escape) fails loudly here.
        assert_eq!(PATH_SANDBOXED_TOOLS, &["read", "ls", "grep", "find"]);
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

    // -----------------------------------------------------------------------
    // VAL-CATOOLS-013 — under M1 the dangerous tools (write/edit/bash) are
    // denied at the before_tool_call hook (no file written/edited, no
    // subprocess), while read-only tools (read/ls/grep/find) pass through.
    // -----------------------------------------------------------------------

    /// Drive the real async hook for the deny stub, mirroring the host dispatch
    /// entry point rather than calling `decide` directly.
    async fn deny_decision(
        ext: &DangerousDenyExtension,
        tool: &str,
        args: serde_json::Value,
    ) -> HookDecision {
        let root = Path::new("/tmp");
        ext.on_before_tool_call(&cx(root), &call_event(tool, args))
            .await
            .expect("deny hook never returns Err")
    }

    #[tokio::test]
    async fn dangerous_tools_are_cancelled_with_approval_reason() {
        let ext = DangerousDenyExtension::new();
        // Realistic side-effecting arguments: the hook must Cancel BEFORE any
        // of these can take effect (no file write/edit, no subprocess).
        let cases = [
            ("write", json!({ "path": "out.txt", "content": "data" })),
            ("edit", json!({ "path": "out.txt", "old": "a", "new": "b" })),
            ("bash", json!({ "command": "rm -rf /" })),
        ];
        for (tool, args) in cases {
            let decision = deny_decision(&ext, tool, args).await;
            match decision {
                HookDecision::Cancel(reason) => {
                    // Reason must carry the "requires approval / not yet
                    // available" semantics the M2 approval flow builds on, and
                    // name the offending tool.
                    assert!(
                        reason.contains("approval"),
                        "{tool} cancel reason must speak the approval semantics, got: {reason:?}"
                    );
                    assert!(
                        reason.contains("not yet available"),
                        "{tool} cancel reason must mark approval as unavailable in M1, got: {reason:?}"
                    );
                    assert!(
                        reason.contains(tool),
                        "{tool} cancel reason should name the denied tool, got: {reason:?}"
                    );
                }
                other => panic!("{tool} must be Cancelled under the M1 deny stub, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn read_only_tools_pass_through_the_deny_stub() {
        let ext = DangerousDenyExtension::new();
        // read/ls/grep/find are not dangerous: the deny stub Continues them
        // (the sandbox extension still judges read/ls paths separately).
        for tool in ["read", "ls", "grep", "find"] {
            let decision = deny_decision(&ext, tool, json!({ "path": "inside.txt" })).await;
            assert!(
                matches!(decision, HookDecision::Continue),
                "{tool} must pass through the dangerous-tool deny stub"
            );
        }
    }

    #[test]
    fn deny_stub_manifest_declares_before_tool_call_capability() {
        let ext = DangerousDenyExtension::new();
        let m = ext.manifest();
        assert_eq!(m.name, DANGEROUS_DENY_EXTENSION_NAME);
        assert_ne!(
            m.name, EXTENSION_NAME,
            "the deny stub must have a name distinct from the sandbox so both coexist"
        );
        assert!(
            m.capabilities.before_tool_call,
            "the deny stub must declare the before_tool_call capability"
        );
    }

    // -----------------------------------------------------------------------
    // M2 PermissionExtension — async approval await + respond rendezvous.
    //
    // A dangerous tool call emits `agent_approval_request`, registers a oneshot,
    // and awaits the user's decision delivered via `respond_to_approval`
    // (`agent_approval_respond` IPC). The pending registry is process-global, so
    // each test mints its own tool/request and resolves it by the requestId the
    // fake emitter captured — never touching another test's entries.
    // -----------------------------------------------------------------------

    /// A fake [`ApprovalEmitter`] that records every emitted approval request
    /// into a shared `Vec`, so a test can read back the `agent_approval_request`
    /// payload (and its `requestId`) without a live Tauri window.
    fn recording_emitter() -> (
        ApprovalEmitter,
        Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    ) {
        let recorded: Arc<std::sync::Mutex<Vec<serde_json::Value>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&recorded);
        let emitter: ApprovalEmitter = Arc::new(move |payload| sink.lock().unwrap().push(payload));
        (emitter, recorded)
    }

    /// Spin until the recording emitter has captured exactly one approval
    /// request, then return its `requestId`. Bounded so a wiring regression
    /// (no request ever emitted) fails loudly instead of hanging.
    async fn await_request_id(recorded: &Arc<std::sync::Mutex<Vec<serde_json::Value>>>) -> String {
        for _ in 0..1000 {
            if let Some(req) = recorded.lock().unwrap().first() {
                return req
                    .get("requestId")
                    .and_then(|v| v.as_str())
                    .expect("approval request must carry a requestId")
                    .to_string();
            }
            tokio::task::yield_now().await;
        }
        panic!("no agent_approval_request was emitted within the bound");
    }

    /// Drive a dangerous `write` call through the real hook on a background task
    /// (it awaits the decision), resolve it via `respond_to_approval(.., allow)`
    /// once the request lands, and return the resolved decision. Mirrors the
    /// frontend round-trip: emit request → user answers → IPC responds → await
    /// resolves.
    async fn approve_via_respond(allow: bool) -> (HookDecision, serde_json::Value) {
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(Some(emitter)));

        let hook_ext = Arc::clone(&ext);
        let task = tokio::spawn(async move {
            hook_ext
                .on_before_tool_call(
                    &cx(Path::new("/tmp")),
                    &call_event("write", json!({ "path": "out.txt", "content": "data" })),
                )
                .await
                .expect("permission hook never returns Err")
        });

        let request_id = await_request_id(&recorded).await;
        respond_to_approval(&request_id, allow);

        let decision = task.await.expect("hook task joins");
        let request = recorded.lock().unwrap()[0].clone();
        (decision, request)
    }

    /// A dangerous tool emits an `agent_approval_request` carrying the tool name,
    /// call id, session id, args, and a request id; an allow response resolves
    /// the awaited hook to `Continue` (the tool runs).
    #[tokio::test]
    async fn dangerous_tool_emits_request_and_allow_resolves_to_continue() {
        let (decision, request) = approve_via_respond(true).await;

        // The emitted request is the `agent_approval_request` shape the frontend
        // consumes: { sessionId, callId, toolName, args, requestId }.
        assert_eq!(request.get("toolName").unwrap(), "write");
        assert_eq!(request.get("callId").unwrap(), "call-1");
        assert_eq!(request.get("sessionId").unwrap(), "test-session");
        assert_eq!(request.get("args").unwrap().get("path").unwrap(), "out.txt");
        assert!(
            request.get("requestId").and_then(|v| v.as_str()).is_some(),
            "request must carry a requestId"
        );

        // Allow → the awaited hook resolves to Continue (the tool executes).
        assert!(
            matches!(decision, HookDecision::Continue),
            "an allowed approval must resolve to Continue"
        );
    }

    /// A deny response resolves the awaited hook to `Cancel` (the tool does NOT
    /// run), with a reason carrying the denied / 被拒 semantics for the model.
    #[tokio::test]
    async fn deny_response_resolves_to_cancel_with_denied_reason() {
        let (decision, _request) = approve_via_respond(false).await;

        match decision {
            HookDecision::Cancel(reason) => {
                assert!(
                    reason.contains("denied"),
                    "deny reason must carry the denied semantics, got: {reason:?}"
                );
                assert!(
                    reason.contains("write"),
                    "deny reason should name the rejected tool, got: {reason:?}"
                );
            }
            other => panic!("a denied approval must Cancel, got {other:?}"),
        }
    }

    /// With NO emitter wired (no approval UI — a unit test / headless build) a
    /// dangerous tool is denied outright: the extension fails CLOSED, never
    /// awaits, never runs the tool (the M1 safety posture preserved). No request
    /// is emitted (there is no emitter to emit through).
    #[tokio::test]
    async fn no_emitter_fails_closed_to_cancel() {
        let ext = PermissionExtension::new(None);
        let decision = ext
            .on_before_tool_call(
                &cx(Path::new("/tmp")),
                &call_event("bash", json!({ "command": "rm -rf /" })),
            )
            .await
            .expect("permission hook never returns Err");

        match decision {
            HookDecision::Cancel(reason) => assert!(
                reason.contains("denied"),
                "fail-closed deny must carry the denied semantics, got: {reason:?}"
            ),
            other => panic!("no emitter must fail closed to Cancel, got {other:?}"),
        }
    }

    /// Read-only / non-dangerous tools are NOT approval-gated: the permission
    /// extension Continues them WITHOUT emitting an approval request (the
    /// sandbox judges their paths separately, earlier in the chain).
    #[tokio::test]
    async fn read_only_tool_continues_without_requesting_approval() {
        let (emitter, recorded) = recording_emitter();
        let ext = PermissionExtension::new(Some(emitter));

        for tool in ["read", "ls", "grep", "find"] {
            let decision = ext
                .on_before_tool_call(
                    &cx(Path::new("/tmp")),
                    &call_event(tool, json!({ "path": "inside.txt" })),
                )
                .await
                .expect("permission hook never returns Err");
            assert!(
                matches!(decision, HookDecision::Continue),
                "{tool} must pass through the approval gate untouched"
            );
        }

        assert!(
            recorded.lock().unwrap().is_empty(),
            "read-only tools must NOT emit an approval request"
        );
    }

    /// `respond_to_approval` is idempotent: the FIRST response for a request_id
    /// wins and a duplicate (or an unknown id) is a clean no-op. We register a
    /// real oneshot, respond twice, and assert only the first decision is
    /// delivered; the second respond — and a respond for an unknown id — do
    /// nothing and do not panic.
    #[tokio::test]
    async fn respond_is_idempotent_for_duplicate_and_unknown_ids() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<bool>();
        pending_approvals()
            .lock()
            .unwrap()
            .insert(request_id.clone(), tx);

        // First response wins (delivers `true`).
        respond_to_approval(&request_id, true);
        assert_eq!(rx.await, Ok(true), "the first response is delivered");

        // Duplicate response for the same id: the entry is already gone — a
        // clean no-op (no panic, nothing to deliver).
        respond_to_approval(&request_id, false);

        // Unknown id: likewise a clean no-op.
        respond_to_approval("no-such-request-id", true);

        // The registry holds no entry for this id afterwards.
        assert!(
            !pending_approvals()
                .lock()
                .unwrap()
                .contains_key(&request_id),
            "a resolved request leaves no lingering registry entry"
        );
    }

    #[test]
    fn permission_manifest_declares_before_tool_call_capability_and_distinct_name() {
        let ext = PermissionExtension::new(None);
        let m = ext.manifest();
        assert_eq!(m.name, PERMISSION_EXTENSION_NAME);
        assert_ne!(
            m.name, EXTENSION_NAME,
            "permission ext name must differ from the sandbox so both coexist"
        );
        assert_ne!(
            m.name, DANGEROUS_DENY_EXTENSION_NAME,
            "permission ext name must differ from the M1 deny stub"
        );
        assert!(
            m.capabilities.before_tool_call,
            "the permission extension must declare the before_tool_call capability"
        );
    }
}
