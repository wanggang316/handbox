//! Permission / boundary extensions for the coding-agent's `before_tool_call`
//! hook chain. The vendored file tools confine nothing (absolute paths pass
//! through, `~` expands to `$HOME`), so HandBox re-imposes the `working_dir`
//! boundary from outside; `bash` is approval-gated instead.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use hand_coding_agent::core::extensions::api::{
    ExtensionCapabilities, ResultDecision, ToolCallEvent, ToolResultEvent,
};
use hand_coding_agent::{
    Extension, ExtensionContext, ExtensionError, ExtensionManifest, HookDecision,
};
use tokio::sync::oneshot;

use crate::services::sandbox::resolve_in_sandbox;

/// Emitter the [`PermissionExtension`] uses to push an approval request to the
/// frontend (wraps `window.emit("agent_approval_request", ..)`). Absent — no UI
/// to consult — the extension fails CLOSED: every dangerous tool is denied.
pub type ApprovalEmitter = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

const EXTENSION_NAME: &str = "handbox-sandbox";

/// Distinct from [`EXTENSION_NAME`] so both extensions coexist in the same hook
/// chain.
const DANGEROUS_DENY_EXTENSION_NAME: &str = "handbox-dangerous-deny";

/// Reason returned to the model when a path argument resolves outside the
/// working directory. MUST NOT echo the offending absolute path.
const OUT_OF_SANDBOX_REASON: &str = "blocked: path is outside the working directory";

/// Tools whose path argument is confined to the working directory. `bash` is
/// absent on purpose — an arbitrary shell command has no single path to confine,
/// so it is approval-gated instead. Omitting the path (`ls`/`grep`/`find` over
/// the cwd) is in-bounds.
const PATH_SANDBOXED_TOOLS: &[&str] = &["read", "ls", "grep", "find", "write", "edit"];

/// Candidate path-argument key(s) for a confined `tool_name`: most tools name
/// the target `path`, `edit` names it `file_path`. The first key present as a
/// string is the one judged.
fn path_arg_keys(tool_name: &str) -> &'static [&'static str] {
    match tool_name {
        "edit" => &["file_path"],
        _ => &["path"],
    }
}

/// Side-effecting tools (filesystem mutation / arbitrary subprocess) that are
/// approval-gated, or denied outright when no approval surface exists.
const DANGEROUS_TOOLS: &[&str] = &["write", "edit", "bash"];

/// Re-imposes the `working_dir` boundary on the agent's path-bearing file tools
/// via the `before_tool_call` hook. `working_dir` is captured at construction so
/// every invocation judges against a stable root, not per-event context.
pub struct SandboxExtension {
    manifest: ExtensionManifest,
    working_dir: PathBuf,
}

impl SandboxExtension {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            manifest: ExtensionManifest {
                name: EXTENSION_NAME.to_string(),
                version: "0.1.0".to_string(),
                description: Some(
                    "Confines path-bearing file tools (read/ls/grep/find/write/edit) to the session working directory."
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
                // Tier 1 runs in-process; the host applies these to subprocess RPC only.
                timeouts: Default::default(),
            },
            working_dir,
        }
    }

    /// Pure over `(tool_name, arguments)` + the captured `working_dir`, so the
    /// boundary is unit-testable without a live session.
    fn decide(&self, tool_name: &str, arguments: &serde_json::Value) -> HookDecision {
        if !PATH_SANDBOXED_TOOLS.contains(&tool_name) {
            return HookDecision::Continue;
        }

        // A missing / non-string path is not a violation: `ls` legitimately
        // omits it, and a required-but-absent path is a parameter error the tool
        // reports itself.
        let path = match path_arg_keys(tool_name)
            .iter()
            .find_map(|key| arguments.get(*key).and_then(|v| v.as_str()))
        {
            Some(p) => p,
            None => return HookDecision::Continue,
        };

        match resolve_in_sandbox(&self.working_dir, path) {
            Ok(_) => HookDecision::Continue,
            // Generic reason — never echo the offending path.
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
    ) -> Result<ResultDecision, ExtensionError> {
        // Permission is decided before the call; the result is none of its
        // business.
        Ok(ResultDecision::Continue)
    }
}

/// Unconditionally denies the dangerous tools via `before_tool_call`, for
/// deployments with no approval surface to consent through. Registered
/// ALONGSIDE [`SandboxExtension`] — the host dispatches in registration order
/// and the FIRST `Cancel` wins — so the two compose without knowing each other.
pub struct DangerousDenyExtension {
    manifest: ExtensionManifest,
}

impl DangerousDenyExtension {
    pub fn new() -> Self {
        Self {
            manifest: ExtensionManifest {
                name: DANGEROUS_DENY_EXTENSION_NAME.to_string(),
                version: "0.1.0".to_string(),
                description: Some(
                    "Denies dangerous tools (write/edit/bash) when no approval surface exists."
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
                // Tier 1 runs in-process; the host applies these to subprocess RPC only.
                timeouts: Default::default(),
            },
        }
    }

    /// Pure over `tool_name` so the deny is unit-testable without a live session;
    /// non-dangerous tools Continue untouched (the sandbox judges their paths).
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
    ) -> Result<ResultDecision, ExtensionError> {
        // Permission is decided before the call; the result is none of its
        // business.
        Ok(ResultDecision::Continue)
    }
}

/// Distinct from [`EXTENSION_NAME`] / [`DANGEROUS_DENY_EXTENSION_NAME`] so all
/// three coexist in the hook chain.
const PERMISSION_EXTENSION_NAME: &str = "handbox-permission";

/// Tauri event the frontend listens on for an approval request. Carries
/// `{ sessionId, callId, toolName, args, requestId }` and is answered via the
/// `agent_approval_respond` IPC.
pub const APPROVAL_REQUEST_EVENT: &str = "agent_approval_request";

/// The user's decision for one approval request. `AllowAlways` additionally
/// remembers the tool for the rest of the session (see [`session_allow_always`]);
/// `AllowOnce` leaves no memory. The snake_case wire values are the exact strings
/// the frontend sends to `agent_approval_respond`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Deny,
    AllowOnce,
    AllowAlways,
}

/// One pending approval: the wake channel plus the `(session_id, tool_name)` it
/// is for, so [`respond_to_approval`] — which only receives a `request_id` — can
/// record an `AllowAlways` against the right session and tool.
struct PendingApproval {
    session_id: String,
    tool_name: String,
    sender: oneshot::Sender<ApprovalDecision>,
}

/// Process-level `request_id → PendingApproval` registry of approvals awaiting a
/// user answer. Process-level because the extension is owned by the driver task
/// while the stateless `agent_approval_respond` command must reach the same
/// entries.
fn pending_approvals() -> &'static Mutex<HashMap<String, PendingApproval>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingApproval>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Process-level `session_id → Set<tool_name>` registry of tools the user chose
/// to ALWAYS allow. Per-session, so consent never leaks across sessions, and
/// IN-MEMORY ONLY — never persisted, so a restart forces the user to re-consent.
fn session_allow_always() -> &'static Mutex<HashMap<String, HashSet<String>>> {
    static ALLOW: OnceLock<Mutex<HashMap<String, HashSet<String>>>> = OnceLock::new();
    ALLOW.get_or_init(|| Mutex::new(HashMap::new()))
}

fn is_session_allow_always(session_id: &str, tool_name: &str) -> bool {
    session_allow_always()
        .lock()
        .unwrap()
        .get(session_id)
        .is_some_and(|tools| tools.contains(tool_name))
}

fn remember_session_allow_always(session_id: &str, tool_name: &str) {
    session_allow_always()
        .lock()
        .unwrap()
        .entry(session_id.to_string())
        .or_default()
        .insert(tool_name.to_string());
}

/// Deny EVERY pending approval for `session_id`, fail-closed: each entry is
/// removed and its sender DROPPED, so the awaiting hook's `rx.await` resolves
/// `Err` → [`HookDecision::Cancel`] and the dangerous tool never executes.
///
/// This is the abort path: the hook awaits on a BARE `rx.await` that does NOT
/// race the run's cancel token, so flipping the token alone cannot unblock a
/// turn parked on an approval. Dropping rather than `send(Deny)` also leaves the
/// always-allow set untouched — an aborted approval is not standing consent.
pub fn deny_pending_for_session(session_id: &str) {
    let mut pending = pending_approvals().lock().unwrap();
    // Collect first: we can't remove while iterating the borrowed map.
    let request_ids: Vec<String> = pending
        .iter()
        .filter(|(_, p)| p.session_id == session_id)
        .map(|(id, _)| id.clone())
        .collect();
    for request_id in request_ids {
        // Dropping the removed sender closes the oneshot and unblocks the awaiter.
        pending.remove(&request_id);
    }
}

/// Resolve a pending approval: record an `AllowAlways` on the session's
/// always-allow set, then wake the awaiting hook with `decision`.
///
/// IDEMPOTENT: the entry is removed before use, so the FIRST response for a
/// `request_id` wins and a duplicate or unknown id is a clean no-op. A send
/// failure (the awaiter is already gone) still records `AllowAlways` — standing
/// consent is independent of whether this particular await is alive.
pub fn respond_to_approval(request_id: &str, decision: ApprovalDecision) {
    let pending = pending_approvals().lock().unwrap().remove(request_id);
    if let Some(pending) = pending {
        // Record the session-scoped standing consent BEFORE waking the awaiter,
        // so a racing second call to the same tool sees the memory immediately.
        if decision == ApprovalDecision::AllowAlways {
            remember_session_allow_always(&pending.session_id, &pending.tool_name);
        }
        // The receiver may already be gone (run aborted) — nothing to wake.
        let _ = pending.sender.send(decision);
    }
}

/// Gates the dangerous tools behind an ASYNCHRONOUS user approval: the hook
/// emits `agent_approval_request` and parks until the frontend answers.
///
/// FAIL-CLOSED: with no emitter wired the extension denies outright rather than
/// awaiting, so a dangerous tool never runs without an explicit consent surface.
///
/// Registered AFTER [`SandboxExtension`] in the chain, where the first `Cancel`
/// wins, so a sandbox escape is stopped before it can prompt the user.
pub struct PermissionExtension {
    manifest: ExtensionManifest,
    /// The HandBox DB session id (UUID) all approval state is keyed off.
    ///
    /// NOT `cx.session_id`: that is the coding agent's internal per-turn
    /// in-memory id (every session is built with `no_session: true`), whereas
    /// `abort_run` / [`deny_pending_for_session`] are called with the HandBox
    /// UUID. Keying off `cx` would leave an abort unable to drop a parked
    /// approval and would degrade always-allow to per-turn.
    session_id: String,
    /// `None` → fail-closed: every dangerous tool is denied.
    emitter: Option<ApprovalEmitter>,
    /// Extra approval-gated tool names beyond [`DANGEROUS_TOOLS`]: the
    /// `mcp__{serverId}__{tool}` names of this session's manual-execution MCP
    /// servers. Auto-execution MCP tools are absent and pass straight through.
    approval_tools: HashSet<String>,
}

impl PermissionExtension {
    /// `session_id` MUST be the same HandBox UUID `abort_run` is called with, or
    /// an abort cannot unblock a parked approval await. `emitter: None` fails
    /// closed.
    pub fn new(session_id: String, emitter: Option<ApprovalEmitter>) -> Self {
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
                // Tier 1 runs in-process; the host applies these to subprocess RPC only.
                timeouts: Default::default(),
            },
            session_id,
            emitter,
            approval_tools: HashSet::new(),
        }
    }

    /// Bind this session's manual-server MCP tool names so they are
    /// approval-gated like the dangerous built-ins.
    pub fn with_approval_tools(mut self, approval_tools: HashSet<String>) -> Self {
        self.approval_tools = approval_tools;
        self
    }
}

/// Request approval for one tool call and await the decision: `Continue` on
/// allow, `Cancel` on deny or fail-closed. A tool already on the session
/// always-allow set short-circuits without emitting or awaiting.
///
/// `session_id` MUST be the HandBox UUID, never the per-event `cx.session_id`.
async fn request_approval(
    session_id: &str,
    emitter: Option<&ApprovalEmitter>,
    call_id: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> HookDecision {
    // A remembered tool runs without prompting — no event emitted, no await.
    if is_session_allow_always(session_id, tool_name) {
        return HookDecision::Continue;
    }

    // No approval surface → fail closed. Never await, never run the tool.
    let Some(emitter) = emitter else {
        return HookDecision::Cancel(deny_reason(tool_name));
    };

    // Register the wake channel BEFORE emitting, so a response that races
    // back the instant the event lands still finds a live entry to resolve.
    let request_id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel::<ApprovalDecision>();
    pending_approvals().lock().unwrap().insert(
        request_id.clone(),
        PendingApproval {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            sender: tx,
        },
    );

    emitter(serde_json::json!({
        "sessionId": session_id,
        "callId": call_id,
        "toolName": tool_name,
        "args": arguments,
        "requestId": request_id,
    }));

    // A dropped sender (`Err`) denies, so a lost response can never hang the
    // turn.
    match rx.await {
        Ok(ApprovalDecision::AllowOnce) | Ok(ApprovalDecision::AllowAlways) => {
            HookDecision::Continue
        }
        Ok(ApprovalDecision::Deny) => HookDecision::Cancel(deny_reason(tool_name)),
        Err(_) => {
            // Run aborted: clean up any lingering entry and deny.
            pending_approvals().lock().unwrap().remove(&request_id);
            HookDecision::Cancel(deny_reason(tool_name))
        }
    }
}

/// The denial reason handed to the model. Must read as a refusal rather than a
/// failure, so the model reports the action as refused, not as a malfunction.
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
        _cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> Result<HookDecision, ExtensionError> {
        // Dangerous built-ins and this session's manual-server MCP tools are
        // approval-gated; everything else passes straight through (the sandbox
        // judged paths earlier in the chain).
        if !DANGEROUS_TOOLS.contains(&event.tool_name.as_str())
            && !self.approval_tools.contains(&event.tool_name)
        {
            return Ok(HookDecision::Continue);
        }
        Ok(request_approval(
            &self.session_id,
            self.emitter.as_ref(),
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
    ) -> Result<ResultDecision, ExtensionError> {
        // Permission is decided before the call; the result is none of its
        // business.
        Ok(ResultDecision::Continue)
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
    /// OUTSIDE the root — the escape geometry the resolver defends.
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

    /// The approval registries are process-global and keyed by session id, so
    /// each scope test mints a fresh uuid session to stay isolated from the rest.
    fn cx_for_session(session_id: &str) -> ExtensionContext {
        let root = Path::new("/tmp");
        ExtensionContext {
            cwd: root.to_path_buf(),
            session_id: session_id.to_string(),
            data_dir: root.join(".hand").join("data"),
        }
    }

    /// Drive the real async hook rather than `decide`, so tests exercise the
    /// same entry point the host dispatch calls.
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
        // `~/...` must be refused at the boundary — upstream `read` would expand
        // it to $HOME; the sandbox stops it first.
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
        // `ls` may omit `path` to list the cwd — in-bounds, not a violation.
        let decision = decide_via_hook(&ext(&fx.root), &fx.root, "ls", json!({})).await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn non_sandboxed_tool_is_always_continued() {
        let fx = fixture();
        // `bash` is not path-sandboxed even with a path-shaped arg — it is
        // approval-gated instead. `write`/`edit` are absent because they ARE
        // path-confined.
        for tool in ["bash", "some_unrelated_tool"] {
            let decision = decide_via_hook(
                &ext(&fx.root),
                &fx.root,
                tool,
                json!({ "path": "/etc/passwd", "command": "rm -rf /" }),
            )
            .await;
            assert!(
                matches!(decision, HookDecision::Continue),
                "{tool} must pass through the path sandbox"
            );
        }
    }

    // `grep`/`find` share the `path` argument and containment rule of `read`/`ls`;
    // without the boundary an injected `grep ~/.ssh/...` or `find /` would reach
    // out-of-cwd contents and filenames.

    #[tokio::test]
    async fn grep_absolute_outside_path_is_cancelled() {
        let fx = fixture();
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
        // `~/...` must be refused at the boundary, never expanded to $HOME.
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
        // Omitting `path` defaults to the cwd — in bounds.
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
        // `~` must be refused, never expanded to $HOME.
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

    // `write`/`edit` are path-confined like the read-only set; an in-bounds
    // target still has to clear the approval gate downstream.

    #[tokio::test]
    async fn write_absolute_outside_path_is_cancelled() {
        let fx = fixture();
        let abs = fx.outside_secret.to_string_lossy().into_owned();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "write",
            json!({ "path": abs, "content": "overwrite" }),
        )
        .await;
        assert_cancel_no_leak(&decision, &fx.outside_secret);
    }

    #[tokio::test]
    async fn write_tilde_path_is_cancelled_not_expanded() {
        let fx = fixture();
        // `~/...` must be refused at the boundary, never expanded to $HOME.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "write",
            json!({ "path": "~/clobbered.txt", "content": "x" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn write_dotdot_traversal_is_cancelled() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "write",
            json!({ "path": "../secret.txt", "content": "x" }),
        )
        .await;
        assert_cancel_no_leak(&decision, &fx.outside_secret);
    }

    #[tokio::test]
    async fn write_inside_path_continues() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "write",
            json!({ "path": "new_inside.txt", "content": "ok" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    #[tokio::test]
    async fn edit_absolute_outside_file_path_is_cancelled() {
        let fx = fixture();
        // `edit` names its target `file_path`, not `path`.
        let abs = fx.outside_secret.to_string_lossy().into_owned();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": abs, "old_string": "a", "new_string": "b" }),
        )
        .await;
        assert_cancel_no_leak(&decision, &fx.outside_secret);
    }

    #[tokio::test]
    async fn edit_tilde_file_path_is_cancelled_not_expanded() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": "~/clobbered.txt", "old_string": "a", "new_string": "b" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Cancel(_)));
    }

    #[tokio::test]
    async fn edit_dotdot_traversal_file_path_is_cancelled() {
        let fx = fixture();
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": "../secret.txt", "old_string": "a", "new_string": "b" }),
        )
        .await;
        assert_cancel_no_leak(&decision, &fx.outside_secret);
    }

    #[tokio::test]
    async fn edit_inside_file_path_continues() {
        let fx = fixture();
        // In-bounds `edit` clears the sandbox; the approval gate is downstream.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": "inside.txt", "old_string": "a", "new_string": "b" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    /// The sandbox judges `edit` on `file_path`, so a stray `path` key neither
    /// smuggles an escape through nor masks an out-of-cwd `file_path`.
    #[tokio::test]
    async fn edit_judges_file_path_key_not_path_key() {
        let fx = fixture();
        let outside_abs = fx.outside_secret.to_string_lossy().into_owned();

        // In-bounds file_path + out-of-cwd `path` (the wrong key): judged on
        // file_path, so a regression that read `path` would Cancel here.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": "inside.txt", "path": outside_abs.clone() }),
        )
        .await;
        assert!(
            matches!(decision, HookDecision::Continue),
            "edit must judge `file_path`, ignoring a stray `path` key"
        );

        // Out-of-cwd file_path is caught regardless of a benign `path`.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": outside_abs, "path": "inside.txt" }),
        )
        .await;
        assert!(
            matches!(decision, HookDecision::Cancel(_)),
            "edit must Cancel an out-of-cwd `file_path` even with a benign `path` present"
        );
    }

    #[test]
    fn path_sandboxed_tools_cover_all_path_bearing_tools() {
        // Pin the exact set so dropping a tool from the containment table fails
        // loudly. `bash` is intentionally absent.
        assert_eq!(
            PATH_SANDBOXED_TOOLS,
            &["read", "ls", "grep", "find", "write", "edit"]
        );
        assert!(
            !PATH_SANDBOXED_TOOLS.contains(&"bash"),
            "bash must NOT be path-sandboxed — it is approval-gated, not path-confined"
        );
    }

    #[test]
    fn path_arg_keys_map_each_confined_tool_to_its_schema_key() {
        // Pin the mapping so a sandbox that only checked `"path"` — letting an
        // out-of-cwd `edit` through under `file_path` — fails loudly here.
        assert_eq!(path_arg_keys("edit"), &["file_path"]);
        for tool in ["read", "ls", "grep", "find", "write"] {
            assert_eq!(
                path_arg_keys(tool),
                &["path"],
                "{tool} names its path argument `path`"
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
        // The hook must Cancel BEFORE any of these side effects can take place.
        let cases = [
            ("write", json!({ "path": "out.txt", "content": "data" })),
            ("edit", json!({ "path": "out.txt", "old": "a", "new": "b" })),
            ("bash", json!({ "command": "rm -rf /" })),
        ];
        for (tool, args) in cases {
            let decision = deny_decision(&ext, tool, args).await;
            match decision {
                HookDecision::Cancel(reason) => {
                    assert!(
                        reason.contains("approval"),
                        "{tool} cancel reason must speak the approval semantics, got: {reason:?}"
                    );
                    assert!(
                        reason.contains("not yet available"),
                        "{tool} cancel reason must mark approval as unavailable, got: {reason:?}"
                    );
                    assert!(
                        reason.contains(tool),
                        "{tool} cancel reason should name the denied tool, got: {reason:?}"
                    );
                }
                other => {
                    panic!("{tool} must be Cancelled by the dangerous-deny ext, got {other:?}")
                }
            }
        }
    }

    #[tokio::test]
    async fn read_only_tools_pass_through_the_deny_stub() {
        let ext = DangerousDenyExtension::new();
        // The sandbox extension still judges these paths separately.
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

    /// A fake [`ApprovalEmitter`] recording every emitted request, so a test can
    /// read back the payload (and its `requestId`) without a live Tauri window.
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

    /// Spin until the emitter has captured an approval request, then return its
    /// `requestId`. Bounded so a wiring regression fails loudly instead of
    /// hanging.
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

    /// Drive a dangerous `write` through the real hook on a background task,
    /// resolve it via `respond_to_approval` once the request lands, and return
    /// the resolved decision — the frontend round-trip in miniature.
    async fn approve_via_respond(decision: ApprovalDecision) -> (HookDecision, serde_json::Value) {
        let (emitter, recorded) = recording_emitter();
        // The cx carries a deliberately different id: keying is off the ext's own
        // HandBox session id, not `cx.session_id`.
        let ext = Arc::new(PermissionExtension::new(
            "test-session".to_string(),
            Some(emitter),
        ));

        let hook_ext = Arc::clone(&ext);
        let task = tokio::spawn(async move {
            hook_ext
                .on_before_tool_call(
                    &cx_for_session("coding-agent-internal-id"),
                    &call_event("write", json!({ "path": "out.txt", "content": "data" })),
                )
                .await
                .expect("permission hook never returns Err")
        });

        let request_id = await_request_id(&recorded).await;
        respond_to_approval(&request_id, decision);

        let decision = task.await.expect("hook task joins");
        let request = recorded.lock().unwrap()[0].clone();
        (decision, request)
    }

    #[tokio::test]
    async fn dangerous_tool_emits_request_and_allow_resolves_to_continue() {
        let (decision, request) = approve_via_respond(ApprovalDecision::AllowOnce).await;

        // The `agent_approval_request` shape the frontend consumes.
        assert_eq!(request.get("toolName").unwrap(), "write");
        assert_eq!(request.get("callId").unwrap(), "call-1");
        // The emitted sessionId is the EXTENSION's HandBox id, not the
        // cx.session_id the hook was driven with — the frontend routes the modal
        // by this id, and abort keys off it.
        assert_eq!(request.get("sessionId").unwrap(), "test-session");
        assert_eq!(request.get("args").unwrap().get("path").unwrap(), "out.txt");
        assert!(
            request.get("requestId").and_then(|v| v.as_str()).is_some(),
            "request must carry a requestId"
        );

        assert!(
            matches!(decision, HookDecision::Continue),
            "an allowed approval must resolve to Continue"
        );
    }

    #[tokio::test]
    async fn deny_response_resolves_to_cancel_with_denied_reason() {
        let (decision, _request) = approve_via_respond(ApprovalDecision::Deny).await;

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

    // Nothing on the deny chain (Cancel → BeforeToolCallResult → ToolResult
    // ::error) rewrites or truncates `reason`, so pinning the text here pins what
    // the model actually receives — and it must read as a refusal, not a
    // malfunction.

    #[test]
    fn deny_reason_speaks_refusal_not_failure() {
        for tool in DANGEROUS_TOOLS {
            let reason = deny_reason(tool);

            // Both the Chinese refusal wording and the English "denied" marker,
            // plus the offending tool name.
            assert!(
                reason.contains("拒绝"),
                "{tool} deny reason must carry the Chinese refusal semantics, got: {reason:?}"
            );
            assert!(
                reason.contains("denied"),
                "{tool} deny reason must carry the English denied marker, got: {reason:?}"
            );
            assert!(
                reason.contains(tool),
                "{tool} deny reason must name the refused tool, got: {reason:?}"
            );

            // A refusal is not an error: wording like "failed to run {tool}"
            // would make the model mis-report a malfunction.
            for failure_word in ["failed", "error", "出错", "失败"] {
                assert!(
                    !reason.contains(failure_word),
                    "{tool} deny reason must read as a refusal, not a failure \
                     (contained {failure_word:?}): {reason:?}"
                );
            }
        }
    }

    /// The exact text is the verbatim contract carried to the model; pin it so a
    /// change to the user-facing wording is a deliberate, reviewed edit.
    #[test]
    fn deny_reason_exact_text_is_the_model_facing_contract() {
        assert_eq!(deny_reason("write"), "用户拒绝了 write（denied）");
        assert_eq!(deny_reason("bash"), "用户拒绝了 bash（denied）");
    }

    /// With no emitter wired the extension fails CLOSED: the dangerous tool is
    /// denied outright, never awaited, never run.
    #[tokio::test]
    async fn no_emitter_fails_closed_to_cancel() {
        let ext = PermissionExtension::new("fail-closed-session".to_string(), None);
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

    /// Non-dangerous tools are not approval-gated: they Continue WITHOUT emitting
    /// a request (the sandbox judged their paths earlier in the chain).
    #[tokio::test]
    async fn read_only_tool_continues_without_requesting_approval() {
        let (emitter, recorded) = recording_emitter();
        let ext = PermissionExtension::new("read-only-session".to_string(), Some(emitter));

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
    /// wins, and a duplicate or unknown id is a clean no-op.
    #[tokio::test]
    async fn respond_is_idempotent_for_duplicate_and_unknown_ids() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<ApprovalDecision>();
        pending_approvals().lock().unwrap().insert(
            request_id.clone(),
            PendingApproval {
                session_id: "idempotent-session".to_string(),
                tool_name: "write".to_string(),
                sender: tx,
            },
        );

        respond_to_approval(&request_id, ApprovalDecision::AllowOnce);
        assert_eq!(
            rx.await,
            Ok(ApprovalDecision::AllowOnce),
            "the first response is delivered"
        );

        // Duplicate for the same id: the entry is already gone — a clean no-op.
        respond_to_approval(&request_id, ApprovalDecision::Deny);

        // Unknown id: likewise a clean no-op.
        respond_to_approval("no-such-request-id", ApprovalDecision::AllowOnce);

        assert!(
            !pending_approvals()
                .lock()
                .unwrap()
                .contains_key(&request_id),
            "a resolved request leaves no lingering registry entry"
        );
    }

    /// Return the `requestId` of the `expected_count`-th (1-based) captured
    /// request. Needed wherever a test emits more than one: `await_request_id`
    /// only ever returns the FIRST, so a later call would resolve a stale
    /// (already-answered) id and hang.
    async fn await_nth_request_id(
        recorded: &Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
        expected_count: usize,
    ) -> String {
        for _ in 0..1000 {
            {
                let guard = recorded.lock().unwrap();
                if guard.len() >= expected_count {
                    return guard[expected_count - 1]
                        .get("requestId")
                        .and_then(|v| v.as_str())
                        .expect("approval request must carry a requestId")
                        .to_string();
                }
            }
            tokio::task::yield_now().await;
        }
        panic!("fewer than {expected_count} approval requests were emitted within the bound");
    }

    /// An `ExtensionContext` whose `session_id` is the coding agent's internal
    /// in-memory id — DELIBERATELY different from the HandBox id the extension is
    /// keyed off, so driving the hook with it proves keying is off
    /// `self.session_id`.
    fn cx_coding_agent_internal() -> ExtensionContext {
        cx_for_session("s_coding_agent_internal_id")
    }

    /// Drive a dangerous `write` through `ext`'s hook on a background task and
    /// resolve it with `decision`. `expected_count` is the running total the
    /// shared sink should hold once THIS call's request has landed, so a stale
    /// earlier request is not resolved instead.
    async fn drive_write_for_session(
        ext: &Arc<PermissionExtension>,
        decision: ApprovalDecision,
        recorded: &Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
        expected_count: usize,
    ) -> HookDecision {
        let hook_ext = Arc::clone(ext);
        let task = tokio::spawn(async move {
            hook_ext
                .on_before_tool_call(
                    &cx_coding_agent_internal(),
                    &call_event("write", json!({ "path": "out.txt", "content": "data" })),
                )
                .await
                .expect("permission hook never returns Err")
        });

        let request_id = await_nth_request_id(recorded, expected_count).await;
        respond_to_approval(&request_id, decision);
        task.await.expect("hook task joins")
    }

    /// Drive a dangerous `write` EXPECTING the always-allow short-circuit: it
    /// must resolve WITHOUT emitting a request (no prompt, no await).
    async fn drive_write_expecting_no_prompt(
        ext: &Arc<PermissionExtension>,
        recorded: &Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    ) -> HookDecision {
        let before = recorded.lock().unwrap().len();
        let decision = ext
            .on_before_tool_call(
                &cx_coding_agent_internal(),
                &call_event("write", json!({ "path": "out.txt", "content": "data" })),
            )
            .await
            .expect("permission hook never returns Err");
        assert_eq!(
            recorded.lock().unwrap().len(),
            before,
            "a session-always-allowed tool must NOT emit another approval request"
        );
        decision
    }

    #[tokio::test]
    async fn allow_always_skips_prompt_for_same_session_same_tool() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        // First call prompts; the user picks always-allow.
        let first =
            drive_write_for_session(&ext, ApprovalDecision::AllowAlways, &recorded, 1).await;
        assert!(
            matches!(first, HookDecision::Continue),
            "allow_always must resolve the first call to Continue"
        );
        assert_eq!(
            recorded.lock().unwrap().len(),
            1,
            "the first call emits exactly one approval request"
        );

        let second = drive_write_expecting_no_prompt(&ext, &recorded).await;
        assert!(
            matches!(second, HookDecision::Continue),
            "a remembered tool must Continue without prompting"
        );
    }

    /// Always-allow is keyed off the STABLE HandBox session id, so it survives
    /// the FRESH extension (and fresh coding-agent in-memory session) every
    /// `agent_run_stream` builds. Keying off `cx.session_id` would degrade it to
    /// per-turn.
    #[tokio::test]
    async fn allow_always_persists_across_turns_for_same_handbox_session() {
        let handbox_session_id = uuid::Uuid::new_v4().to_string();

        // Turn 1: a fresh extension; the user picks always-allow for write.
        let (emitter1, recorded1) = recording_emitter();
        let ext_turn1 = Arc::new(PermissionExtension::new(
            handbox_session_id.clone(),
            Some(emitter1),
        ));
        let first =
            drive_write_for_session(&ext_turn1, ApprovalDecision::AllowAlways, &recorded1, 1).await;
        assert!(
            matches!(first, HookDecision::Continue),
            "turn 1 allow_always resolves to Continue"
        );
        assert_eq!(recorded1.lock().unwrap().len(), 1, "turn 1 prompts once");

        // Turn 2: a brand-new extension for the SAME HandBox session, with its
        // own fresh sink; the remembered consent must short-circuit the prompt.
        let (emitter2, recorded2) = recording_emitter();
        let ext_turn2 = Arc::new(PermissionExtension::new(
            handbox_session_id.clone(),
            Some(emitter2),
        ));
        let second = drive_write_expecting_no_prompt(&ext_turn2, &recorded2).await;
        assert!(
            matches!(second, HookDecision::Continue),
            "a tool always-allowed in turn 1 must Continue without prompting in turn 2 — \
             keyed off the stable HandBox session id, not the per-turn cx.session_id"
        );
        assert!(
            recorded2.lock().unwrap().is_empty(),
            "turn 2 must NOT re-prompt: always-allow persists across turns of the same \
             HandBox session (not degraded to per-turn)"
        );
    }

    /// Always-allow does NOT cross sessions: allowing the tool in session A
    /// leaves session B prompting (and awaiting) for the same tool.
    #[tokio::test]
    async fn allow_always_does_not_cross_sessions() {
        let session_a = uuid::Uuid::new_v4().to_string();
        let session_b = uuid::Uuid::new_v4().to_string();
        // One shared sink, a distinct extension per session (as in production),
        // so both emitted requests can be read back.
        let (emitter, recorded) = recording_emitter();
        let ext_a = Arc::new(PermissionExtension::new(
            session_a.clone(),
            Some(emitter.clone()),
        ));
        let ext_b = Arc::new(PermissionExtension::new(session_b.clone(), Some(emitter)));

        // Session A: always-allow `write`.
        let a = drive_write_for_session(&ext_a, ApprovalDecision::AllowAlways, &recorded, 1).await;
        assert!(matches!(a, HookDecision::Continue));
        assert_eq!(recorded.lock().unwrap().len(), 1);

        // Session B still prompts — A's standing consent does not leak into B.
        let b = drive_write_for_session(&ext_b, ApprovalDecision::AllowOnce, &recorded, 2).await;
        assert!(matches!(b, HookDecision::Continue));
        assert_eq!(
            recorded.lock().unwrap().len(),
            2,
            "session B must emit its OWN approval request — always-allow is per-session"
        );
        let recorded_sessions: Vec<String> = recorded
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.get("sessionId").unwrap().as_str().unwrap().to_string())
            .collect();
        assert!(recorded_sessions.contains(&session_a));
        assert!(recorded_sessions.contains(&session_b));
    }

    #[tokio::test]
    async fn allow_once_does_not_remember_for_next_call() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        let first = drive_write_for_session(&ext, ApprovalDecision::AllowOnce, &recorded, 1).await;
        assert!(matches!(first, HookDecision::Continue));
        assert_eq!(recorded.lock().unwrap().len(), 1);

        let second = drive_write_for_session(&ext, ApprovalDecision::AllowOnce, &recorded, 2).await;
        assert!(matches!(second, HookDecision::Continue));
        assert_eq!(
            recorded.lock().unwrap().len(),
            2,
            "allow_once must NOT remember the tool — the next call prompts again"
        );
    }

    /// The gate's only source of truth is the in-memory map (no DB/file write),
    /// so a restart — an empty map — forgets every recorded consent.
    #[tokio::test]
    async fn allow_always_set_is_in_memory_and_session_scoped() {
        let recorded_session = uuid::Uuid::new_v4().to_string();
        let fresh_session = uuid::Uuid::new_v4().to_string();

        assert!(
            !is_session_allow_always(&fresh_session, "write"),
            "a session unknown to the in-memory set must not be always-allowed \
             (a restart starts from exactly this empty state)"
        );

        remember_session_allow_always(&recorded_session, "write");
        assert!(
            is_session_allow_always(&recorded_session, "write"),
            "a recorded session is always-allowed for that tool"
        );

        // Membership is keyed by session id AND tool name — no blanket allow
        // leaks across either axis.
        assert!(
            !is_session_allow_always(&recorded_session, "bash"),
            "always-allow is per-tool: an unrelated tool is not allowed"
        );
        assert!(
            !is_session_allow_always(&fresh_session, "write"),
            "always-allow is per-session: another session is not allowed"
        );

        // Cross-turn: keyed off the STABLE HandBox session id, so a brand-new
        // extension for the same session still sees the recorded consent.
        let (emitter, recorded_sink) = recording_emitter();
        let ext_next_turn = Arc::new(PermissionExtension::new(
            recorded_session.clone(),
            Some(emitter),
        ));
        let next_turn = drive_write_expecting_no_prompt(&ext_next_turn, &recorded_sink).await;
        assert!(
            matches!(next_turn, HookDecision::Continue),
            "a fresh extension for the same HandBox session must still honour the \
             recorded always-allow (no re-prompt) — proving the memory survives a \
             new build_agent_session, not just the same extension instance"
        );
    }

    #[test]
    fn approval_decision_serde_wire_values_match_the_frontend() {
        // The exact `decision` values the frontend sends to
        // `agent_approval_respond` — pin them so a rename is a deliberate IPC
        // break.
        assert_eq!(
            serde_json::to_value(ApprovalDecision::Deny).unwrap(),
            json!("deny")
        );
        assert_eq!(
            serde_json::to_value(ApprovalDecision::AllowOnce).unwrap(),
            json!("allow_once")
        );
        assert_eq!(
            serde_json::to_value(ApprovalDecision::AllowAlways).unwrap(),
            json!("allow_always")
        );
        assert_eq!(
            serde_json::from_value::<ApprovalDecision>(json!("allow_always")).unwrap(),
            ApprovalDecision::AllowAlways
        );
    }

    #[test]
    fn permission_manifest_declares_before_tool_call_capability_and_distinct_name() {
        let ext = PermissionExtension::new("manifest-session".to_string(), None);
        let m = ext.manifest();
        assert_eq!(m.name, PERMISSION_EXTENSION_NAME);
        assert_ne!(
            m.name, EXTENSION_NAME,
            "permission ext name must differ from the sandbox so both coexist"
        );
        assert_ne!(
            m.name, DANGEROUS_DENY_EXTENSION_NAME,
            "permission ext name must differ from the dangerous-deny ext"
        );
        assert!(
            m.capabilities.before_tool_call,
            "the permission extension must declare the before_tool_call capability"
        );
    }

    #[tokio::test]
    async fn read_only_tools_never_emit_an_approval_request() {
        let (emitter, recorded) = recording_emitter();
        let ext = PermissionExtension::new("read-only-edge-session".to_string(), Some(emitter));

        // Only the non-dangerous subset: iterating the whole table would park on
        // an approval await for write/edit.
        let read_only: Vec<&&str> = PATH_SANDBOXED_TOOLS
            .iter()
            .filter(|t| !DANGEROUS_TOOLS.contains(t))
            .collect();
        assert_eq!(
            read_only,
            vec![&"read", &"ls", &"grep", &"find"],
            "the read-only subset must be exactly read/ls/grep/find"
        );

        for tool in read_only {
            let decision = ext
                .on_before_tool_call(
                    &cx(Path::new("/tmp")),
                    &call_event(tool, json!({ "path": "inside.txt" })),
                )
                .await
                .expect("permission hook never returns Err");
            assert!(
                matches!(decision, HookDecision::Continue),
                "{tool} (read-only) must pass the approval gate untouched"
            );
        }
        assert!(
            recorded.lock().unwrap().is_empty(),
            "read-only tools must NEVER emit an approval request"
        );
    }

    /// Upstream `validate_tool_args` runs BEFORE `before_tool_call`, so a
    /// dangerous tool with invalid arguments never reaches the hook and emits no
    /// approval request. Modelled here by simply not invoking the hook.
    #[tokio::test]
    async fn illegal_args_never_reach_the_hook_so_no_request_is_emitted() {
        let (emitter, recorded) = recording_emitter();
        let _ext = PermissionExtension::new("illegal-args-session".to_string(), Some(emitter));

        assert!(
            recorded.lock().unwrap().is_empty(),
            "an illegal-arg call that never reaches the hook emits no approval request"
        );

        // Floor: a valid dangerous call that DOES reach the hook is what emits,
        // so the emit is gated on the hook firing, not on the tool name alone.
        // Fail-closed (no emitter) so the call resolves without an answer.
        let no_emitter = PermissionExtension::new("illegal-args-floor-session".to_string(), None);
        let _ = no_emitter
            .on_before_tool_call(
                &cx(Path::new("/tmp")),
                &call_event("write", json!({ "path": "out.txt", "content": "data" })),
            )
            .await
            .expect("permission hook never returns Err");
        assert!(
            recorded.lock().unwrap().is_empty(),
            "ext's recorded sink stays empty — only an invoked dangerous hook emits"
        );
    }

    /// Once a request is resolved, a later `Deny` for the same id is a clean
    /// no-op — a late deny can never flip an already-granted allow.
    #[tokio::test]
    async fn first_response_wins_allow_then_deny_only_allow_lands() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let session_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<ApprovalDecision>();
        pending_approvals().lock().unwrap().insert(
            request_id.clone(),
            PendingApproval {
                session_id,
                tool_name: "write".to_string(),
                sender: tx,
            },
        );

        respond_to_approval(&request_id, ApprovalDecision::AllowOnce);
        // A racing/late deny for the same id finds no entry — clean no-op.
        respond_to_approval(&request_id, ApprovalDecision::Deny);

        assert_eq!(
            rx.await,
            Ok(ApprovalDecision::AllowOnce),
            "only the FIRST decision (allow) is delivered; the late deny is dropped"
        );
    }

    /// A LOST response (frontend closed, IPC dropped) must not hang the turn:
    /// the dropped sender resolves `rx.await` to `Err` and the hook fail-closes
    /// to `Cancel`.
    #[tokio::test]
    async fn dropped_response_resolves_await_to_cancel_not_hang() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        // The hook is driven with a different coding-agent-internal cx, so the
        // drop can only match off the ext's HandBox id.
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        let hook_ext = Arc::clone(&ext);
        let task = tokio::spawn(async move {
            hook_ext
                .on_before_tool_call(
                    &cx_coding_agent_internal(),
                    &call_event("write", json!({ "path": "out.txt", "content": "data" })),
                )
                .await
                .expect("permission hook never returns Err")
        });

        // The request lands, then the response is LOST: drop the sender instead
        // of responding; the bare `rx.await` must resolve, not hang.
        let _request_id = await_request_id(&recorded).await;
        deny_pending_for_session(&session_id);

        let decision = task.await.expect("hook task joins (did not hang)");
        match decision {
            HookDecision::Cancel(reason) => assert!(
                reason.contains("denied"),
                "a lost response must fail-close to a denied Cancel, got: {reason:?}"
            ),
            other => panic!("a lost response must resolve to Cancel, got {other:?}"),
        }
    }

    /// Aborting a turn parked on an approval await fail-closes: the dropped
    /// sender resolves the await to `Cancel` and the dangerous tool never runs.
    /// `abort_run` relies on this, since the bare await does not race the cancel
    /// token.
    #[tokio::test]
    async fn deny_pending_for_session_unblocks_awaiting_hook_to_cancel() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        // The hook runs against a different coding-agent-internal cx (as in
        // production); the abort can only match because the pending registry
        // keys off the ext's HandBox id.
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        let hook_ext = Arc::clone(&ext);
        let task = tokio::spawn(async move {
            hook_ext
                .on_before_tool_call(
                    &cx_coding_agent_internal(),
                    &call_event("bash", json!({ "command": "rm -rf /" })),
                )
                .await
                .expect("permission hook never returns Err")
        });

        // The turn is parked on the approval await; abort denies it by HandBox
        // session id, which differs from the cx that drove the hook.
        await_request_id(&recorded).await;
        deny_pending_for_session(&session_id);

        let decision = task.await.expect("hook task joins after abort");
        assert!(
            matches!(decision, HookDecision::Cancel(_)),
            "an aborted pending approval must Cancel — the dangerous tool must not run"
        );

        assert!(
            !pending_approvals()
                .lock()
                .unwrap()
                .values()
                .any(|p| p.session_id == session_id),
            "deny_pending_for_session leaves no pending entry for the session"
        );
    }

    /// A late user "allow" arriving after the abort fail-closed the request finds
    /// no entry: a clean no-op that crucially records NO standing consent, which
    /// would otherwise let a future call skip the prompt.
    #[tokio::test]
    async fn late_allow_after_pending_denied_is_a_noop_and_records_no_consent() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, _rx) = oneshot::channel::<ApprovalDecision>();
        pending_approvals().lock().unwrap().insert(
            request_id.clone(),
            PendingApproval {
                session_id: session_id.clone(),
                tool_name: "write".to_string(),
                sender: tx,
            },
        );

        // Abort fail-closes the pending request (drops the sender, removes entry).
        deny_pending_for_session(&session_id);

        // A late "allow_always" for that id now finds nothing — no-op.
        respond_to_approval(&request_id, ApprovalDecision::AllowAlways);

        assert!(
            !is_session_allow_always(&session_id, "write"),
            "a late allow after abort must NOT record standing consent"
        );
    }

    /// [`deny_pending_for_session`] fail-closes ONLY the named session's pending
    /// approvals: another session's stays parked awaiting its own answer, and a
    /// session with none is a clean no-op.
    #[tokio::test]
    async fn deny_pending_for_session_is_session_scoped_and_noop_when_empty() {
        let session_a = uuid::Uuid::new_v4().to_string();
        let session_b = uuid::Uuid::new_v4().to_string();
        let unknown = uuid::Uuid::new_v4().to_string();

        let req_a = uuid::Uuid::new_v4().to_string();
        let req_b = uuid::Uuid::new_v4().to_string();
        let (tx_a, mut rx_a) = oneshot::channel::<ApprovalDecision>();
        let (tx_b, mut rx_b) = oneshot::channel::<ApprovalDecision>();
        {
            let mut pending = pending_approvals().lock().unwrap();
            pending.insert(
                req_a.clone(),
                PendingApproval {
                    session_id: session_a.clone(),
                    tool_name: "write".to_string(),
                    sender: tx_a,
                },
            );
            pending.insert(
                req_b.clone(),
                PendingApproval {
                    session_id: session_b.clone(),
                    tool_name: "bash".to_string(),
                    sender: tx_b,
                },
            );
        }

        // Unknown session: clean no-op — both pending entries survive.
        deny_pending_for_session(&unknown);
        assert!(
            rx_a.try_recv().is_err(),
            "A still pending after unknown deny"
        );
        assert!(
            rx_b.try_recv().is_err(),
            "B still pending after unknown deny"
        );

        // Deny session A only: A's sender is dropped; B is untouched.
        deny_pending_for_session(&session_a);
        assert_eq!(
            rx_a.try_recv(),
            Err(oneshot::error::TryRecvError::Closed),
            "session A's await is unblocked (sender dropped) → fail-closed"
        );
        assert_eq!(
            rx_b.try_recv(),
            Err(oneshot::error::TryRecvError::Empty),
            "session B's request is untouched — still awaiting its own answer"
        );

        // Clean up B.
        deny_pending_for_session(&session_b);
    }

    /// Walk a tool call through the extension chain in registration order
    /// (sandbox → permission), short-circuiting at the first `Cancel` exactly as
    /// the host dispatch does, so a test can assert WHICH extension stopped it.
    async fn decide_via_chain(
        sandbox: &SandboxExtension,
        permission: &PermissionExtension,
        cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> HookDecision {
        // Only a `Continue` falls through to the permission gate; any other
        // decision is the chain's verdict (first-decision-wins, as the host does).
        let sandbox_decision = sandbox
            .on_before_tool_call(cx, event)
            .await
            .expect("sandbox hook never returns Err");
        if !matches!(sandbox_decision, HookDecision::Continue) {
            return sandbox_decision;
        }
        permission
            .on_before_tool_call(cx, event)
            .await
            .expect("permission hook never returns Err")
    }

    /// A sandbox escape takes the SILENT Cancel path: the sandbox runs BEFORE the
    /// approval gate and the first Cancel wins, so no modal pops for it.
    #[tokio::test]
    async fn out_of_sandbox_read_is_silently_cancelled_without_emitting_approval() {
        let fx = fixture();
        let sandbox = ext(&fx.root);
        let (emitter, recorded) = recording_emitter();
        let permission = PermissionExtension::new("chain-read-session".to_string(), Some(emitter));

        let abs = fx.outside_secret.to_string_lossy().into_owned();
        let event = call_event("read", json!({ "path": abs }));

        let decision = decide_via_chain(&sandbox, &permission, &cx(&fx.root), &event).await;

        // The sandbox stops it (generic, leak-free reason) …
        assert_cancel_no_leak(&decision, &fx.outside_secret);
        // … and the approval emitter was NEVER invoked.
        assert!(
            recorded.lock().unwrap().is_empty(),
            "an out-of-sandbox read must NOT emit an approval request — the sandbox \
             Cancels it before the permission gate is reached (VAL-CAPERM-022)"
        );
    }

    #[tokio::test]
    async fn out_of_sandbox_ls_is_silently_cancelled_without_emitting_approval() {
        let fx = fixture();
        let sandbox = ext(&fx.root);
        let (emitter, recorded) = recording_emitter();
        let permission = PermissionExtension::new("chain-ls-session".to_string(), Some(emitter));

        let outside_dir = fx.outside_secret.parent().unwrap().to_path_buf();
        let abs = outside_dir.to_string_lossy().into_owned();
        let event = call_event("ls", json!({ "path": abs }));

        let decision = decide_via_chain(&sandbox, &permission, &cx(&fx.root), &event).await;

        assert_cancel_no_leak(&decision, &outside_dir);
        assert!(
            recorded.lock().unwrap().is_empty(),
            "an out-of-sandbox ls must NOT emit an approval request"
        );
    }

    /// With two sessions concurrently parked on a pending approval, responding to
    /// A resolves ONLY A's await; B stays parked until answered in its own right.
    #[tokio::test]
    async fn responding_to_one_session_does_not_resolve_another_pending() {
        let session_a = uuid::Uuid::new_v4().to_string();
        let session_b = uuid::Uuid::new_v4().to_string();
        // One shared sink, a distinct extension per session (as in production).
        let (emitter, recorded) = recording_emitter();
        let ext_a = Arc::new(PermissionExtension::new(
            session_a.clone(),
            Some(emitter.clone()),
        ));
        let ext_b = Arc::new(PermissionExtension::new(session_b.clone(), Some(emitter)));

        // Park BOTH sessions on a pending `write` approval (each on its own task).
        let task_a = tokio::spawn(async move {
            ext_a
                .on_before_tool_call(
                    &cx_coding_agent_internal(),
                    &call_event("write", json!({ "path": "a.txt", "content": "A" })),
                )
                .await
                .expect("permission hook never returns Err")
        });
        let task_b = tokio::spawn(async move {
            ext_b
                .on_before_tool_call(
                    &cx_coding_agent_internal(),
                    &call_event("write", json!({ "path": "b.txt", "content": "B" })),
                )
                .await
                .expect("permission hook never returns Err")
        });

        // Look A's requestId up by sessionId: arrival order across the two tasks
        // is non-deterministic, so index 0 is not necessarily A.
        for _ in 0..1000 {
            if recorded.lock().unwrap().len() >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let request_id_a = {
            let guard = recorded.lock().unwrap();
            assert_eq!(guard.len(), 2, "both sessions must each emit one request");
            guard
                .iter()
                .find(|r| r.get("sessionId").and_then(|v| v.as_str()) == Some(session_a.as_str()))
                .and_then(|r| r.get("requestId").and_then(|v| v.as_str()))
                .expect("session A's request must carry a requestId")
                .to_string()
        };

        // Respond to A ONLY. A's await must resolve; B's must stay parked.
        respond_to_approval(&request_id_a, ApprovalDecision::AllowOnce);

        let decision_a = task_a.await.expect("session A's hook task joins");
        assert!(
            matches!(decision_a, HookDecision::Continue),
            "responding allow to A resolves A's await to Continue"
        );

        // We can't `.await` task_b without answering it — it would hang — so
        // assert it is unresolved, then clean it up.
        assert!(
            !task_b.is_finished(),
            "session B's await must remain parked — responding to A must not resolve B"
        );
        assert!(
            pending_approvals()
                .lock()
                .unwrap()
                .values()
                .any(|p| p.session_id == session_b),
            "session B's pending entry must survive A's response"
        );

        // Clean up B's parked await so the test task does not leak.
        deny_pending_for_session(&session_b);
        let decision_b = task_b
            .await
            .expect("session B's hook task joins after cleanup");
        assert!(
            matches!(decision_b, HookDecision::Cancel(_)),
            "B was never answered; cleaning it up fail-closes to Cancel"
        );
    }

    /// `Deny` records nothing on the always-allow set, so a model re-sending the
    /// same tool in the same session prompts again.
    #[tokio::test]
    async fn deny_does_not_remember_so_resend_reprompts() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        let first = drive_write_for_session(&ext, ApprovalDecision::Deny, &recorded, 1).await;
        assert!(
            matches!(first, HookDecision::Cancel(_)),
            "a denied call Cancels"
        );
        assert_eq!(recorded.lock().unwrap().len(), 1, "first call emitted once");

        assert!(
            !is_session_allow_always(&session_id, "write"),
            "deny must NOT enter the always-allow set"
        );

        // The model re-sends the same tool: it prompts again.
        let second = drive_write_for_session(&ext, ApprovalDecision::Deny, &recorded, 2).await;
        assert!(matches!(second, HookDecision::Cancel(_)));
        assert_eq!(
            recorded.lock().unwrap().len(),
            2,
            "a denied tool re-prompts on re-send"
        );
    }
}
