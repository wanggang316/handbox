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
//! This feature enforces the boundary for the READ-ONLY path tools
//! (`read`, `ls`, `grep`, `find`) AND the write-side path tools
//! (`write`, `edit`). All six are path-confined to the working directory: a
//! mutating `write`/`edit` aimed outside the cwd is just as dangerous as an
//! out-of-cwd read, so the sandbox is extended symmetrically to them (M2).
//! `bash` is intentionally NOT path-sandboxed: an arbitrary shell command has
//! no single path argument to confine (it can name any path inside the command
//! string, pipe, redirect, or `cd` away), so containment is meaningless there —
//! it is gated by the approval flow instead. The deny/approval surface
//! (m1-dangerous-deny-stub, M2 approval) layers ON TOP of this extension —
//! see [`SandboxExtension`] for the extension points designed for that reuse.
//!
//! PATH-ARGUMENT KEYS
//! ------------------
//! The confined tools do NOT all name their path argument the same way:
//! `read`/`ls`/`grep`/`find`/`write` use `path`, but `edit` uses `file_path`
//! (verified against the coding-agent tool schemas). The sandbox therefore
//! probes a tool-specific set of candidate keys (see [`path_arg_keys`]) rather
//! than a single hard-coded `"path"`.
//!
//! HOW THE BOUNDARY IS JUDGED
//! --------------------------
//! Path containment reuses [`agent_tools::resolve_in_sandbox`], HandBox's
//! existing strict resolver: it rejects `~`, collapses `..`, canonicalizes
//! (resolving symlinks), and verifies component-wise containment under the
//! canonical root. The `Cancel` reason is GENERIC — it never echoes the
//! out-of-sandbox absolute path — matching the resolver's leak-free contract.

use std::collections::{HashMap, HashSet};
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

/// The tools whose path argument this feature confines to the working
/// directory: the read-only set (`read`/`ls`/`grep`/`find`) plus the write-side
/// mutating tools (`write`/`edit`). `bash` is never path-sandboxed (it is
/// approval-gated instead — an arbitrary shell command has no single path
/// argument to confine; see the module-level SCOPE note).
///
/// The path-argument KEY is not uniform across these tools: most use `path`,
/// but `edit` uses `file_path` (verified against the coding-agent tool schemas;
/// see [`path_arg_keys`]). The containment rule is otherwise identical for all
/// of them — resolve the path through [`resolve_in_sandbox`] and `Cancel` an
/// escape. A confined tool that legitimately omits its path (`ls`/`grep`/`find`
/// over the cwd) is treated as in-bounds rather than a violation; a `write`
/// without `path` or an `edit` without `file_path` is a parameter error the
/// tool reports itself. `grep`/`find` are confined because the upstream tools
/// apply NO containment of their own: an absolute path is used as-is and a
/// leading `~` would expand to `$HOME`, letting an unsandboxed call read file
/// contents / list filenames anywhere on disk; `write`/`edit` are confined for
/// the symmetric reason on the WRITE side — an out-of-cwd `write`/`edit` would
/// mutate files anywhere on disk.
const PATH_SANDBOXED_TOOLS: &[&str] = &["read", "ls", "grep", "find", "write", "edit"];

/// The candidate path-argument key(s) to inspect for a confined `tool_name`.
///
/// Most confined tools name their target `path`; `edit` names it `file_path`.
/// Returned as a slice so the resolver can try each key in turn (the first key
/// that is present as a string is the one judged). Centralising the
/// tool→key(s) mapping here keeps [`SandboxExtension::decide`] uniform: it never
/// hard-codes a key name, so adding a tool with a differently-named path arg is
/// a one-line change here.
fn path_arg_keys(tool_name: &str) -> &'static [&'static str] {
    match tool_name {
        // `edit`'s schema names the target `file_path` (not `path`).
        "edit" => &["file_path"],
        // read/ls/grep/find/write all name it `path`.
        _ => &["path"],
    }
}

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
/// coding agent's path-bearing file tools (read/ls/grep/find/write/edit) via
/// the `before_tool_call` hook.
///
/// EXTENSIBILITY (the M1 deny-stub / M2 approval hand-off point):
/// - The extension captures the session's `working_dir` at construction, so
///   every hook invocation judges against a stable, known root rather than
///   trusting per-event context.
/// - The "which tools are path-confined" decision is a single table
///   ([`PATH_SANDBOXED_TOOLS`]); the per-tool path-argument key(s) live in
///   [`path_arg_keys`], so adding a tool (or one with a differently-named path
///   argument, as `edit`'s `file_path`) is a localized change.
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
            },
            working_dir,
        }
    }

    /// Decide whether a tool call may proceed under the sandbox boundary.
    ///
    /// Pure over `(tool_name, arguments)` + the captured `working_dir`, so it
    /// is unit-testable without a live session. Tools not in
    /// [`PATH_SANDBOXED_TOOLS`] always [`HookDecision::Continue`]. For a
    /// confined tool, its path argument — looked up under the tool-specific
    /// key(s) from [`path_arg_keys`] (`file_path` for `edit`, `path` for the
    /// rest), first present string wins — is run through [`resolve_in_sandbox`]:
    /// an `Err` (escape / invalid / unresolved) becomes a generic
    /// [`HookDecision::Cancel`]; an in-bounds path or an absent/non-string path
    /// (e.g. `ls`/`grep`/`find` over the cwd) is allowed.
    fn decide(&self, tool_name: &str, arguments: &serde_json::Value) -> HookDecision {
        if !PATH_SANDBOXED_TOOLS.contains(&tool_name) {
            return HookDecision::Continue;
        }

        // Extract the path argument under the tool's candidate key(s). A
        // missing / non-string path is not a boundary violation: `ls`
        // legitimately omits it (lists the cwd), and a `read`/`write` without
        // `path` (or an `edit` without `file_path`) is a parameter error the
        // tool reports itself.
        let path = match path_arg_keys(tool_name)
            .iter()
            .find_map(|key| arguments.get(*key).and_then(|v| v.as_str()))
        {
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

/// The user's decision for one dangerous-tool approval request, with its SCOPE.
///
/// Replaces the earlier `allow: bool` so the wire carries the scope explicitly
/// rather than encoding it ambiguously:
/// - [`ApprovalDecision::Deny`] — refuse this call (the tool is `Cancel`led).
/// - [`ApprovalDecision::AllowOnce`] — allow THIS call only; the next call to the
///   same tool prompts again (no memory).
/// - [`ApprovalDecision::AllowAlways`] — allow this call AND remember the tool
///   for the REST OF THIS SESSION: subsequent calls to the same tool in the same
///   session run without prompting. The memory is session-scoped and in-memory
///   only (see [`session_allow_always`]).
///
/// `#[serde(rename_all = "snake_case")]` so the wire values are
/// `"deny" | "allow_once" | "allow_always"` — the exact strings the frontend
/// sends as the `decision` argument of `agent_approval_respond`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Deny,
    AllowOnce,
    AllowAlways,
}

/// One pending approval awaiting a user answer: the wake channel plus the
/// `(session_id, tool_name)` it is for. The scope target is stored ALONGSIDE the
/// sender so [`respond_to_approval`] — which only receives a `request_id` from
/// the stateless IPC command — can record an `AllowAlways` against the right
/// session+tool without the command knowing either.
struct PendingApproval {
    session_id: String,
    tool_name: String,
    sender: oneshot::Sender<ApprovalDecision>,
}

/// Process-level `request_id → PendingApproval` registry of approval decisions
/// still awaiting a user answer.
///
/// WHY PROCESS-LEVEL (mirrors `coding_agent_runtime::run_controls`):
/// the `PermissionExtension` is owned by the `AgentSession`, which the driver
/// task owns for the turn — there is no instance-level place to hang per-request
/// state the stateless `agent_approval_respond` command can reach. A
/// `OnceLock<Mutex<HashMap<..>>>` gives both the extension (insert + await) and
/// the command (remove + send) a shared rendezvous keyed by `request_id`.
///
/// LIFECYCLE: `on_before_tool_call` inserts an entry then awaits the matching
/// receiver; [`respond_to_approval`] removes the entry and `send`s the decision,
/// waking the await. A `request_id` is a fresh uuid, so entries never collide.
/// If the receiver is dropped before a response arrives (e.g. the run is
/// aborted), `respond` finds no entry — a clean no-op.
fn pending_approvals() -> &'static Mutex<HashMap<String, PendingApproval>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingApproval>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Process-level `session_id → Set<tool_name>` registry of tools the user chose
/// to ALWAYS allow for that session ("始终允许该工具（本会话）").
///
/// SCOPE — deliberately three properties, each a security/UX decision:
/// - PER-SESSION: keyed by `session_id`, so allowing a tool in session A does
///   NOT allow it in session B (session B still prompts) — VAL-CAPERM-009.
/// - IN-MEMORY ONLY: a `OnceLock<Mutex<HashMap<..>>>`, NEVER written to the DB or
///   any file. A process restart starts empty, so "always allow" does NOT
///   survive a restart — the user re-consents in the new process. This is a
///   safety decision: a persisted blanket allow is exactly what we must not have
///   (VAL-CAPERM-010).
/// - PROCESS-WIDE STRUCTURE (mirrors `run_controls`): the `PermissionExtension`
///   instance is owned by the driver task, but `respond_to_approval` runs in the
///   stateless IPC command; both reach the same set through this registry.
fn session_allow_always() -> &'static Mutex<HashMap<String, HashSet<String>>> {
    static ALLOW: OnceLock<Mutex<HashMap<String, HashSet<String>>>> = OnceLock::new();
    ALLOW.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether `tool_name` is on the session's always-allow set (the user previously
/// chose "始终允许" for it in THIS session). Read by `on_before_tool_call` to
/// skip the prompt+await entirely for a remembered tool.
fn is_session_allow_always(session_id: &str, tool_name: &str) -> bool {
    session_allow_always()
        .lock()
        .unwrap()
        .get(session_id)
        .is_some_and(|tools| tools.contains(tool_name))
}

/// Record that `tool_name` is ALWAYS allowed for `session_id` for the rest of
/// this session (in-memory only). Called by [`respond_to_approval`] on an
/// `AllowAlways` decision.
fn remember_session_allow_always(session_id: &str, tool_name: &str) {
    session_allow_always()
        .lock()
        .unwrap()
        .entry(session_id.to_string())
        .or_default()
        .insert(tool_name.to_string());
}

/// Deny EVERY pending approval for `session_id`, fail-closed: each matching
/// entry is REMOVED from the registry and its sender DROPPED, so the awaiting
/// `on_before_tool_call`'s `rx.await` resolves `Err` → [`HookDecision::Cancel`]
/// and the dangerous tool never executes.
///
/// WHY THIS EXISTS (the abort承重 point — VAL-CAPERM-016): the permission hook
/// awaits the decision on a BARE `rx.await` that does NOT race the run's cancel
/// token. So flipping the cancel token alone ([`crate::services::coding_agent_runtime::abort_run`])
/// cannot unblock a turn parked on an approval await — without this, aborting a
/// run while a write/edit/bash sits waiting for consent would leave the await
/// hanging (and, worse, a late user "allow" could still run the tool AFTER the
/// abort). Dropping the sender unblocks the await deterministically and resolves
/// it to the fail-closed `Cancel`, so an aborted run's pending dangerous tool is
/// guaranteed NOT to execute.
///
/// Dropping (rather than `send(Deny)`) is the right primitive here: the
/// `request_approval` `Err` arm is ALREADY the fail-closed deny path (it returns
/// the same `Cancel(deny_reason)`), and dropping deliberately does NOT touch the
/// always-allow set — an aborted approval must never be mistaken for standing
/// consent. A session with no pending approvals is a clean no-op.
pub fn deny_pending_for_session(session_id: &str) {
    let mut pending = pending_approvals().lock().unwrap();
    // Collect the request ids for this session first, then remove them — we
    // can't remove while iterating the borrowed map. Dropping the removed
    // `PendingApproval` (and with it its `oneshot::Sender`) closes the channel,
    // waking the awaiting `rx.await` with `Err` (fail-closed Cancel).
    let request_ids: Vec<String> = pending
        .iter()
        .filter(|(_, p)| p.session_id == session_id)
        .map(|(id, _)| id.clone())
        .collect();
    for request_id in request_ids {
        // The removed PendingApproval (incl. its sender) is dropped at end of
        // scope, closing the oneshot and unblocking the awaiter.
        pending.remove(&request_id);
    }
}

/// Resolve a pending approval: wake the awaiting `on_before_tool_call` with the
/// user's `decision`, and — for [`ApprovalDecision::AllowAlways`] — first record
/// the tool on the request's session always-allow set so future calls to the
/// same tool in the same session skip the prompt.
///
/// IDEMPOTENT / leak-free: the entry is REMOVED from the registry before being
/// used, so the FIRST response for a `request_id` wins and a duplicate or
/// unknown `request_id` is a clean no-op (nothing to remove, nothing to send).
/// A send failure (the receiver was already dropped — the awaiting tool call
/// was abandoned) is likewise ignored: there is no live awaiter to wake — but
/// the `AllowAlways` memory is still recorded, since the user's standing consent
/// for the session is independent of whether this particular await is alive.
///
/// This is the body the `agent_approval_respond` IPC command delegates to; it is
/// `pub` so the command (and unit tests) can drive the rendezvous without a live
/// session.
pub fn respond_to_approval(request_id: &str, decision: ApprovalDecision) {
    let pending = pending_approvals().lock().unwrap().remove(request_id);
    if let Some(pending) = pending {
        // Record the session-scoped standing consent BEFORE waking the awaiter,
        // so a racing second call to the same tool sees the memory immediately.
        if decision == ApprovalDecision::AllowAlways {
            remember_session_allow_always(&pending.session_id, &pending.tool_name);
        }
        // The receiver may already be gone (run aborted); a failed send means
        // there is no awaiter to wake, which is fine.
        let _ = pending.sender.send(decision);
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
    /// The HandBox DB session id (UUID) this extension keys all approval state
    /// off of: the pending-approval registry, the per-session always-allow set,
    /// and the emitted `agent_approval_request` payload's `sessionId`.
    ///
    /// WHY NOT `cx.session_id` (the production-hang fix): the
    /// [`ExtensionContext::session_id`] the host passes to the hook is the
    /// VENDORED coding-agent's INTERNAL session id (an `s_<ts>_<…>` minted by
    /// `SessionManager::in_memory()` because HandBox builds every session with
    /// `no_session: true`), which is unrelated to HandBox's DB session UUID.
    /// `coding_agent_runtime::abort_run` / `deny_pending_for_session` are called
    /// with the HandBox UUID, so keying the pending registry off `cx.session_id`
    /// meant an abort could NEVER match the entry it had to drop — a run parked
    /// on an approval await would hang forever (the bare `rx.await` does not race
    /// the cancel token). Likewise the always-allow set keyed off `cx.session_id`
    /// degraded to per-turn: each `agent_run_stream` mints a FRESH in-memory
    /// session (a new `s_…` id), so "始终允许" was never found again. Keying off
    /// THIS field — the stable HandBox UUID threaded in at construction
    /// ([`crate::services::coding_agent_session::build_agent_session`]) — makes
    /// abort hit and always-allow persist across the session's turns.
    session_id: String,
    /// Emitter pushing `agent_approval_request` to the frontend. `None` →
    /// fail-closed (deny every dangerous tool); see the type-level doc.
    emitter: Option<ApprovalEmitter>,
}

impl PermissionExtension {
    /// Construct a permission extension bound to the HandBox DB `session_id`
    /// (UUID).
    ///
    /// `session_id` is the stable HandBox session UUID this extension keys all
    /// approval state off of (pending registry / always-allow / emit payload) —
    /// NOT the coding-agent's internal `cx.session_id`. It MUST be the same id
    /// `coding_agent_runtime::abort_run` is called with, or an abort cannot
    /// unblock a parked approval await (see the `session_id` field doc).
    ///
    /// `emitter` is the approval-request channel: `Some` wires the frontend
    /// prompt + await; `None` makes the extension fail closed (deny dangerous
    /// tools), the safe default when there is no UI to consult.
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
            },
            session_id,
            emitter,
        }
    }

    /// Request approval for one dangerous tool call and await the decision.
    ///
    /// SCOPE SHORT-CIRCUIT: if the user previously chose "始终允许" for this
    /// `(session_id, tool_name)` (it is on the session always-allow set), the
    /// call is allowed WITHOUT emitting a request or awaiting — it
    /// [`HookDecision::Continue`]s straight away (VAL-CAPERM-008). Only a tool
    /// NOT remembered for the session reaches the prompt+await below.
    ///
    /// Otherwise returns the resolved [`HookDecision`]: `Continue` on allow
    /// (once or always), `Cancel` on deny / fail-closed. Factored out of the
    /// trait method so the rendezvous is directly unit-testable. A non-dangerous
    /// tool never reaches here — the caller short-circuits it to `Continue`.
    ///
    /// All approval state is keyed off [`self.session_id`](PermissionExtension::session_id)
    /// — the stable HandBox UUID, NOT the per-event `cx.session_id` — so an
    /// `abort_run` (called with the HandBox UUID) can match and drop the pending
    /// entry, and always-allow persists across the session's turns.
    async fn request_approval(
        &self,
        call_id: &str,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> HookDecision {
        let session_id = self.session_id.as_str();

        // SCOPE: a tool the user chose to always allow for THIS session runs
        // without prompting — no event emitted, no await. Checked first so a
        // remembered tool never re-surfaces an approval request.
        if is_session_allow_always(session_id, tool_name) {
            return HookDecision::Continue;
        }

        // No approval surface → fail closed (M1 safety posture). Never await,
        // never run the tool.
        let Some(emitter) = &self.emitter else {
            return HookDecision::Cancel(deny_reason(tool_name));
        };

        // Mint a fresh request id and register the wake channel BEFORE emitting,
        // so a response that races back the instant the event is delivered
        // always finds a live entry to resolve. The session+tool ride along so
        // `respond_to_approval` can record an `AllowAlways` against them.
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

        // Await the user's decision. `AllowOnce`/`AllowAlways` → allow (the
        // always-allow memory was already recorded by `respond_to_approval`);
        // `Deny` → deny; `Err` (sender dropped without sending) → deny. On the
        // `Err` path the registry entry is already gone (removed by
        // `respond_to_approval`), or we remove it here to avoid leaking an
        // orphaned entry if the receiver resolved via `Err`.
        match rx.await {
            Ok(ApprovalDecision::AllowOnce) | Ok(ApprovalDecision::AllowAlways) => {
                HookDecision::Continue
            }
            Ok(ApprovalDecision::Deny) => HookDecision::Cancel(deny_reason(tool_name)),
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
        _cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> Result<HookDecision, ExtensionError> {
        // Only the dangerous, side-effecting tools are approval-gated; read-only
        // / non-dangerous tools pass straight through (the sandbox judges their
        // paths separately, earlier in the chain).
        if !DANGEROUS_TOOLS.contains(&event.tool_name.as_str()) {
            return Ok(HookDecision::Continue);
        }
        // Key the approval rendezvous off `self.session_id` (the HandBox UUID),
        // NOT `cx.session_id` (the coding-agent's internal in-memory id). Only
        // the HandBox UUID matches the id `abort_run` / `deny_pending_for_session`
        // use, so an aborted turn parked here can actually be unblocked, and
        // always-allow keys off the same stable id across turns. The call_id /
        // tool_name / args still come from the live event.
        Ok(self
            .request_approval(&event.call_id, &event.tool_name, &event.arguments)
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

    /// An [`ExtensionContext`] bound to a SPECIFIC `session_id`, for the scope
    /// tests: the always-allow set is process-global and keyed by session id, so
    /// each scope test mints a fresh uuid session to stay isolated from the
    /// others (and from the default `"test-session"`).
    fn cx_for_session(session_id: &str) -> ExtensionContext {
        let root = Path::new("/tmp");
        ExtensionContext {
            cwd: root.to_path_buf(),
            session_id: session_id.to_string(),
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
        // `bash` is NOT path-sandboxed even with a path-shaped arg (an arbitrary
        // shell command has no single path to confine — it is approval-gated
        // instead); an unrelated tool likewise passes through untouched.
        // `write`/`edit` are deliberately ABSENT here — they ARE path-confined
        // now (M2), see the write/edit boundary tests below.
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

    // -----------------------------------------------------------------------
    // VAL-CATOOLS-025 — the write-side path tools (`write`/`edit`) are
    // path-confined to the working directory, symmetric to the read-only set.
    // An out-of-cwd target (absolute outside, `~`, or `..` traversal) is
    // Cancelled so the tool never runs (no bytes mutated outside the sandbox);
    // an in-bounds target Continues (it still has to clear the approval gate
    // separately, downstream). `edit` names its target `file_path`, `write`
    // names it `path` — both are exercised. `bash` is NOT path-sandboxed (it
    // is approval-gated; covered by `non_sandboxed_tool_is_always_continued`).
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn write_absolute_outside_path_is_cancelled() {
        let fx = fixture();
        // An out-of-cwd absolute write target must be refused so the tool never
        // mutates a file outside the sandbox.
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
        // `~/...` must be REFUSED at the boundary, never expanded to $HOME —
        // upstream `write` would expand it; the sandbox stops it first.
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
        // `edit` names its target `file_path` (not `path`). An out-of-cwd
        // absolute target must be refused so the tool never edits a file
        // outside the sandbox.
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
        // In-bounds `edit` clears the sandbox (it still faces the approval gate
        // downstream). Uses `file_path`, the edit-specific key.
        let decision = decide_via_hook(
            &ext(&fx.root),
            &fx.root,
            "edit",
            json!({ "file_path": "inside.txt", "old_string": "a", "new_string": "b" }),
        )
        .await;
        assert!(matches!(decision, HookDecision::Continue));
    }

    /// The sandbox keys `edit` off `file_path`, NOT `path`. A would-be escape
    /// that puts the out-of-cwd target under a `path` key (the wrong key for
    /// `edit`) while leaving `file_path` in-bounds must NOT smuggle the escape
    /// through — the resolver judges the `file_path` value, so this Continues;
    /// and conversely an out-of-cwd `file_path` is caught even if a benign
    /// `path` is also present. This pins that `edit` reads the right key.
    #[tokio::test]
    async fn edit_judges_file_path_key_not_path_key() {
        let fx = fixture();
        let outside_abs = fx.outside_secret.to_string_lossy().into_owned();

        // In-bounds file_path + out-of-cwd `path` (wrong key): judged on
        // file_path → Continue. A regression that read `path` would Cancel here.
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
        // The six path-bearing tools share one containment rule and ride one
        // table — pin the exact set so a regression (e.g. dropping grep/find,
        // the original Critical escape, or dropping the M2 write/edit boundary)
        // fails loudly here. `bash` is intentionally NOT in the set.
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
        // `edit`'s schema names the target `file_path`; the others use `path`.
        // Pin the mapping so a sandbox that only checked `"path"` (and so let an
        // out-of-cwd `edit` through under `file_path`) fails loudly here.
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
    /// (it awaits the decision), resolve it via `respond_to_approval(.., decision)`
    /// once the request lands, and return the resolved decision. Mirrors the
    /// frontend round-trip: emit request → user answers → IPC responds → await
    /// resolves.
    async fn approve_via_respond(decision: ApprovalDecision) -> (HookDecision, serde_json::Value) {
        let (emitter, recorded) = recording_emitter();
        // The extension is keyed off its OWN HandBox session id now (not
        // cx.session_id); the cx the hook receives is the coding-agent's internal
        // id and no longer affects keying — use a distinct value to make that
        // explicit (the emitted sessionId must be the ext's, "test-session").
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

    /// A dangerous tool emits an `agent_approval_request` carrying the tool name,
    /// call id, session id, args, and a request id; an allow response resolves
    /// the awaited hook to `Continue` (the tool runs).
    #[tokio::test]
    async fn dangerous_tool_emits_request_and_allow_resolves_to_continue() {
        let (decision, request) = approve_via_respond(ApprovalDecision::AllowOnce).await;

        // The emitted request is the `agent_approval_request` shape the frontend
        // consumes: { sessionId, callId, toolName, args, requestId }.
        assert_eq!(request.get("toolName").unwrap(), "write");
        assert_eq!(request.get("callId").unwrap(), "call-1");
        // The emitted sessionId is the EXTENSION's HandBox session id
        // ("test-session"), NOT the cx.session_id the hook was driven with
        // ("coding-agent-internal-id") — the frontend routes the modal by this
        // HandBox id, and abort keys off it.
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

    // -----------------------------------------------------------------------
    // VAL-CAPERM-006 — the denial reason fed back to the model must explicitly
    // read as "被拒/denied" (the USER refused), distinct from "execution
    // failed / errored". This is a CONTENT contract on the exact string that
    // rides the deny chain all the way to the model:
    //   deny_reason(tool)
    //     -> HookDecision::Cancel(reason)                (request_approval)
    //     -> dispatch_before_tool_call propagates the first Cancel verbatim
    //     -> BeforeToolCallResult { block: true, reason: Some(reason) }
    //                                                   (make_before_tool_call_hook)
    //     -> ToolResult::error(reason)                  (agent_loop prepare)
    // Nothing on that chain rewrites or truncates `reason`, so pinning the
    // text here pins what the model actually receives. The deny vocabulary is
    // deliberately NOT generic-failure wording ("failed"/"error"/"出错") so the
    // model words its reply as a refusal, not a malfunction.
    // -----------------------------------------------------------------------

    #[test]
    fn deny_reason_speaks_refusal_not_failure() {
        for tool in DANGEROUS_TOOLS {
            let reason = deny_reason(tool);

            // Carries BOTH the Chinese "被拒" semantics ("拒绝") and the English
            // "denied" marker, and names the offending tool — the contract the
            // model leans on to say "the action was refused".
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

            // Distinct from generic execution-failure wording: a refusal is not
            // an error/crash. If this ever regresses to "failed to run {tool}"
            // the model would mis-report a malfunction instead of a refusal.
            for failure_word in ["failed", "error", "出错", "失败"] {
                assert!(
                    !reason.contains(failure_word),
                    "{tool} deny reason must read as a refusal, not a failure \
                     (contained {failure_word:?}): {reason:?}"
                );
            }
        }
    }

    /// The exact `deny_reason` text is the verbatim contract the deny chain
    /// carries to the model (see VAL-CAPERM-006 block above). Pin it so a change
    /// to the user-facing wording is a deliberate, reviewed edit — and document
    /// the precise string the GUI validator should expect in the model's
    /// fed-back tool result.
    #[test]
    fn deny_reason_exact_text_is_the_model_facing_contract() {
        assert_eq!(deny_reason("write"), "用户拒绝了 write（denied）");
        assert_eq!(deny_reason("bash"), "用户拒绝了 bash（denied）");
    }

    /// With NO emitter wired (no approval UI — a unit test / headless build) a
    /// dangerous tool is denied outright: the extension fails CLOSED, never
    /// awaits, never runs the tool (the M1 safety posture preserved). No request
    /// is emitted (there is no emitter to emit through).
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

    /// Read-only / non-dangerous tools are NOT approval-gated: the permission
    /// extension Continues them WITHOUT emitting an approval request (the
    /// sandbox judges their paths separately, earlier in the chain).
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
    /// wins and a duplicate (or an unknown id) is a clean no-op. We register a
    /// real oneshot, respond twice, and assert only the first decision is
    /// delivered; the second respond — and a respond for an unknown id — do
    /// nothing and do not panic.
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

        // First response wins (delivers `AllowOnce`).
        respond_to_approval(&request_id, ApprovalDecision::AllowOnce);
        assert_eq!(
            rx.await,
            Ok(ApprovalDecision::AllowOnce),
            "the first response is delivered"
        );

        // Duplicate response for the same id: the entry is already gone — a
        // clean no-op (no panic, nothing to deliver).
        respond_to_approval(&request_id, ApprovalDecision::Deny);

        // Unknown id: likewise a clean no-op.
        respond_to_approval("no-such-request-id", ApprovalDecision::AllowOnce);

        // The registry holds no entry for this id afterwards.
        assert!(
            !pending_approvals()
                .lock()
                .unwrap()
                .contains_key(&request_id),
            "a resolved request leaves no lingering registry entry"
        );
    }

    // -----------------------------------------------------------------------
    // VAL-CAPERM-008..011 — approval SCOPE ("本次允许 / 始终允许（本会话）").
    //
    // "始终允许 <tool>" for a session records the tool on a process-global,
    // session-keyed, in-memory always-allow set. Subsequent calls to the same
    // tool in the SAME session skip the prompt entirely (no emit, no await);
    // a DIFFERENT session still prompts (the set is session-isolated); and the
    // set is never persisted, so a process restart starts empty. "本次允许"
    // (`AllowOnce`) records nothing, so the next call prompts again.
    //
    // The always-allow set is process-global, so each test mints a fresh uuid
    // session to stay isolated from the others.
    // -----------------------------------------------------------------------

    /// Spin until the recording emitter has captured AT LEAST `expected_count`
    /// requests, then return the `requestId` of the `expected_count`-th one (1-
    /// based). Used by the scope tests that emit MORE than one request across
    /// successive calls — `await_request_id` only ever returns the FIRST, which
    /// would make a later call resolve a stale (already-answered) id and hang.
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

    /// A coding-agent-internal `ExtensionContext` whose `session_id` is the
    /// vendored in-memory id (`s_…`-style) the host actually passes to the hook —
    /// DELIBERATELY DIFFERENT from the HandBox session id the extension is keyed
    /// off. Mirrors production, where `no_session: true` mints a fresh internal
    /// id per turn that has nothing to do with HandBox's DB UUID. Driving the
    /// hook with this proves the keying is off `self.session_id`, not `cx`.
    fn cx_coding_agent_internal() -> ExtensionContext {
        cx_for_session("s_coding_agent_internal_id")
    }

    /// Drive a dangerous `write` call through the real permission hook of `ext`
    /// (which is keyed off its OWN HandBox session id) on a background task,
    /// resolve the request with `decision`, and return the resolved
    /// `HookDecision`. The hook is driven with a coding-agent-internal cx that
    /// does NOT match the ext's HandBox id — proving keying is off the ext, not
    /// the cx. `expected_count` is the running total of requests the shared
    /// `recorded` sink should hold once THIS call's request has landed (1 for the
    /// first call, 2 for the second, …), so the helper resolves THIS call's
    /// request rather than a stale earlier one. Mirrors the frontend round-trip.
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

    /// Drive a dangerous `write` call through `ext`'s hook EXPECTING the
    /// always-allow short-circuit: the call must resolve WITHOUT emitting a
    /// request (no prompt, no await). Returns the resolved decision. Driven with
    /// the coding-agent-internal cx so the short-circuit is provably off the
    /// ext's HandBox session id.
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

    /// VAL-CAPERM-008 — after "始终允许 <tool>" in a session, the SAME tool in
    /// the SAME session runs without prompting: the second call emits NO request
    /// and resolves straight to `Continue`.
    #[tokio::test]
    async fn allow_always_skips_prompt_for_same_session_same_tool() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        // First call prompts and the user picks "始终允许" → allowed, remembered
        // against the ext's HandBox session id.
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

        // Second call to the same tool in the same session: NO new request, and
        // it Continues directly off the remembered consent.
        let second = drive_write_expecting_no_prompt(&ext, &recorded).await;
        assert!(
            matches!(second, HookDecision::Continue),
            "a remembered tool must Continue without prompting"
        );
    }

    /// VAL-CAPERM-008 (cross-turn, production wiring) — always-allow is keyed off
    /// the STABLE HandBox session id, so it survives a FRESH `PermissionExtension`
    /// built for the SAME HandBox session on the NEXT turn. This models the real
    /// flow: every `agent_run_stream` mints a NEW coding-agent in-memory session
    /// (a new `s_…` cx.session_id) and a NEW extension, yet the user's "始终允许"
    /// from turn 1 must still be remembered in turn 2. The OLD code keyed off
    /// `cx.session_id`, which changed every turn, degrading always-allow to
    /// per-turn — this test would fail under that regression.
    #[tokio::test]
    async fn allow_always_persists_across_turns_for_same_handbox_session() {
        let handbox_session_id = uuid::Uuid::new_v4().to_string();

        // --- Turn 1: a fresh extension; user picks "始终允许 write". ---
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

        // --- Turn 2: a BRAND-NEW extension for the SAME HandBox session (as a
        // second agent_run_stream would build), with its OWN fresh sink. The
        // remembered consent must short-circuit the prompt — NO new request. ---
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

    /// VAL-CAPERM-009 — "始终允许" does NOT cross sessions: allowing the tool in
    /// session A leaves session B prompting (and awaiting) for the same tool.
    #[tokio::test]
    async fn allow_always_does_not_cross_sessions() {
        let session_a = uuid::Uuid::new_v4().to_string();
        let session_b = uuid::Uuid::new_v4().to_string();
        // One shared sink; a distinct extension per HandBox session (each session
        // gets its own extension in production). The extensions share the sink so
        // we can read both emitted requests back.
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

        // Session B: the SAME tool still prompts (a second request is emitted)
        // and awaits — A's standing consent does not leak into B.
        let b = drive_write_for_session(&ext_b, ApprovalDecision::AllowOnce, &recorded, 2).await;
        assert!(matches!(b, HookDecision::Continue));
        assert_eq!(
            recorded.lock().unwrap().len(),
            2,
            "session B must emit its OWN approval request — always-allow is per-session"
        );
        // The two requests carry the two distinct session ids.
        let recorded_sessions: Vec<String> = recorded
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.get("sessionId").unwrap().as_str().unwrap().to_string())
            .collect();
        assert!(recorded_sessions.contains(&session_a));
        assert!(recorded_sessions.contains(&session_b));
    }

    /// VAL-CAPERM-011 — "本次允许" (`AllowOnce`) records nothing: the next call
    /// to the same tool in the same session prompts (and awaits) again.
    #[tokio::test]
    async fn allow_once_does_not_remember_for_next_call() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        // First call: allow once.
        let first = drive_write_for_session(&ext, ApprovalDecision::AllowOnce, &recorded, 1).await;
        assert!(matches!(first, HookDecision::Continue));
        assert_eq!(recorded.lock().unwrap().len(), 1);

        // Second call to the same tool: it prompts AGAIN (a second request is
        // emitted) — allow_once left no memory.
        let second = drive_write_for_session(&ext, ApprovalDecision::AllowOnce, &recorded, 2).await;
        assert!(matches!(second, HookDecision::Continue));
        assert_eq!(
            recorded.lock().unwrap().len(),
            2,
            "allow_once must NOT remember the tool — the next call prompts again"
        );
    }

    /// VAL-CAPERM-010 — the always-allow set is in-memory only: recording a tool
    /// mutates the process-global `session_allow_always` map (no DB/file write),
    /// so a fresh process (an empty map — what a restart yields) does not know
    /// it. We assert the SCOPE PRIMITIVE directly: a session unknown to the set
    /// is not always-allowed, and a recorded session is — proving the gate's
    /// only source of truth is this in-memory map, which a restart resets.
    #[tokio::test]
    async fn allow_always_set_is_in_memory_and_session_scoped() {
        let recorded_session = uuid::Uuid::new_v4().to_string();
        let fresh_session = uuid::Uuid::new_v4().to_string();

        // A never-recorded session (the state a process restart yields) is NOT
        // always-allowed for the tool.
        assert!(
            !is_session_allow_always(&fresh_session, "write"),
            "a session unknown to the in-memory set must not be always-allowed \
             (a restart starts from exactly this empty state)"
        );

        // Recording is a pure in-memory mutation of the process-global map.
        remember_session_allow_always(&recorded_session, "write");
        assert!(
            is_session_allow_always(&recorded_session, "write"),
            "a recorded session is always-allowed for that tool"
        );

        // The membership is keyed by session id AND tool name: a different tool
        // in the recorded session, and the recorded tool in a different session,
        // are both absent — no blanket allow leaks across either axis.
        assert!(
            !is_session_allow_always(&recorded_session, "bash"),
            "always-allow is per-tool: an unrelated tool is not allowed"
        );
        assert!(
            !is_session_allow_always(&fresh_session, "write"),
            "always-allow is per-session: another session is not allowed"
        );

        // CROSS-TURN (the keying fix): the memory is keyed off the STABLE HandBox
        // session id, so a BRAND-NEW PermissionExtension built for the SAME
        // session — as a second `build_agent_session` / `agent_run_stream` would
        // do — still sees the recorded consent and short-circuits the prompt. This
        // is the property that broke when keying off the per-turn cx.session_id.
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
        // The wire strings are the exact `decision` values the frontend sends as
        // the `agent_approval_respond` argument — pin them so a rename here is a
        // deliberate, reviewed break of the IPC contract.
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
            "permission ext name must differ from the M1 deny stub"
        );
        assert!(
            m.capabilities.before_tool_call,
            "the permission extension must declare the before_tool_call capability"
        );
    }

    // =======================================================================
    // VAL-CAPERM-012..020 — approval EDGE CASES (m2-approval-edge-cases).
    //
    // These pin the behaviours the boundary depends on:
    //   012 read-only tools never prompt;
    //   016 aborting a turn parked on an approval await fail-closes (the tool
    //       never runs) — the承重 implementation point;
    //   017 a lost / dropped response does not hang — the await resolves to a
    //       fail-closed Cancel;
    //   018 idempotent / first-wins (allow-then-deny → only the allow lands);
    //   019 illegal args never reach the hook (validate先于hook upstream), so the
    //       permission extension only emits when the hook is actually invoked;
    //   020 a denied tool is NOT remembered, so a model re-send re-prompts.
    //
    // The pending / always-allow registries are process-global, so every test
    // mints a fresh uuid session to stay isolated from the rest of the binary.
    // =======================================================================

    /// VAL-CAPERM-012 — the read-only path tools (read/ls/grep/find) are NOT
    /// approval-gated: the permission extension Continues each WITHOUT emitting an
    /// approval request. (Restates `read_only_tool_continues_without_requesting_approval`
    /// under the edge-case contract, pinning that only DANGEROUS_TOOLS emit.)
    #[tokio::test]
    async fn read_only_tools_never_emit_an_approval_request() {
        let (emitter, recorded) = recording_emitter();
        let ext = PermissionExtension::new("read-only-edge-session".to_string(), Some(emitter));

        // The read-only subset of the path-sandboxed tools — i.e. the ones that
        // are NOT in DANGEROUS_TOOLS. (Since M2 widened PATH_SANDBOXED_TOOLS to
        // include write/edit, iterating the whole table here would hit the
        // dangerous tools and park on an approval await; the approval contract
        // for THOSE is covered by the dangerous-tool tests, not this one.)
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
            "read-only tools must NEVER emit an approval request (VAL-CAPERM-012)"
        );
    }

    /// VAL-CAPERM-019 — the permission extension emits ONLY when its hook is
    /// actually invoked, and ONLY for dangerous tools. Upstream `validate_tool_args`
    /// runs BEFORE `before_tool_call` (agent_loop `prepare_tool_call`), so a
    /// dangerous tool whose arguments fail schema validation is short-circuited to
    /// an Immediate error and the hook is NEVER called — no approval request is
    /// emitted. We model that upstream guarantee here: when the hook is NOT
    /// invoked (the invalid-arg case), nothing is recorded; only a hook invocation
    /// for a dangerous tool emits.
    #[tokio::test]
    async fn illegal_args_never_reach_the_hook_so_no_request_is_emitted() {
        let (emitter, recorded) = recording_emitter();
        let ext = PermissionExtension::new("illegal-args-session".to_string(), Some(emitter));

        // Upstream: invalid args → Immediate error in `prepare_tool_call`, BEFORE
        // `before_tool_call`. So for an illegal-arg dangerous call the hook is
        // never reached — we simply DO NOT invoke it, mirroring that ordering.
        // No request may have been emitted by anyone.
        assert!(
            recorded.lock().unwrap().is_empty(),
            "an illegal-arg call that never reaches the hook emits no approval request"
        );

        // Sanity floor: a VALID dangerous call that DOES reach the hook is what
        // emits — proving the emit is gated on the hook firing, not on the tool
        // name alone. (Fail-closed deny so the spawned await resolves.)
        let no_emitter = PermissionExtension::new("illegal-args-floor-session".to_string(), None);
        let _ = no_emitter
            .on_before_tool_call(
                &cx(Path::new("/tmp")),
                &call_event("write", json!({ "path": "out.txt", "content": "data" })),
            )
            .await
            .expect("permission hook never returns Err");
        // (no_emitter records nothing either, but it is the fail-closed path; the
        // recorded sink belongs to `ext` and stays empty since `ext`'s hook was
        // never invoked.)
        assert!(
            recorded.lock().unwrap().is_empty(),
            "ext's recorded sink stays empty — only an invoked dangerous hook emits"
        );
    }

    /// VAL-CAPERM-018 — first-wins idempotency across SCOPES: once a request is
    /// resolved (here `AllowOnce`), a SUBSEQUENT `Deny` for the same id is a clean
    /// no-op — only the first decision is ever delivered. Pins that a late
    /// "deny" can never flip an already-granted allow (and vice versa).
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

        // First decision wins.
        respond_to_approval(&request_id, ApprovalDecision::AllowOnce);
        // A racing/late deny for the same id finds no entry — clean no-op.
        respond_to_approval(&request_id, ApprovalDecision::Deny);

        assert_eq!(
            rx.await,
            Ok(ApprovalDecision::AllowOnce),
            "only the FIRST decision (allow) is delivered; the late deny is dropped"
        );
    }

    /// VAL-CAPERM-017 — a LOST response does not hang the turn: if the sender is
    /// dropped (the response never arrives — e.g. the frontend window closed
    /// without answering, or the IPC was lost) the `rx.await` resolves `Err` and
    /// the hook fail-closes to `Cancel`. We drive the real hook and drop the
    /// pending sender via [`deny_pending_for_session`] (the same primitive a lost
    /// response / abort uses) instead of ever responding.
    #[tokio::test]
    async fn dropped_response_resolves_await_to_cancel_not_hang() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        // The ext is keyed off the HandBox session id; the hook is driven with a
        // DIFFERENT coding-agent-internal cx, so the drop must match off the
        // ext's id, not cx's.
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

        // The request lands, then the response is LOST: drop the sender rather
        // than responding. The bare `rx.await` must resolve (Err → Cancel), not
        // hang.
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

    /// VAL-CAPERM-016 — aborting a turn parked on an approval await fail-closes:
    /// [`deny_pending_for_session`] drops the pending sender so the bare `rx.await`
    /// resolves to `Cancel` and the dangerous tool NEVER runs. This is the承重
    /// abort path (`abort_run` calls this after flipping the cancel token, since
    /// the bare await does not race the token). Drives the REAL hook end-to-end.
    #[tokio::test]
    async fn deny_pending_for_session_unblocks_awaiting_hook_to_cancel() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        // The ext keys its pending registry off the HandBox session id; the hook
        // runs against a DIFFERENT coding-agent-internal cx (as in production).
        // The abort uses the HandBox id — it can only match because the registry
        // is keyed off the ext's id, not the cx's.
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

        // Turn is parked on the approval await. Abort denies the pending request
        // keyed by the HandBox session id (different from the cx that drove it).
        await_request_id(&recorded).await;
        deny_pending_for_session(&session_id);

        let decision = task.await.expect("hook task joins after abort");
        assert!(
            matches!(decision, HookDecision::Cancel(_)),
            "an aborted pending approval must Cancel — the dangerous tool must not run"
        );

        // The registry holds no residue for this session afterwards.
        assert!(
            !pending_approvals()
                .lock()
                .unwrap()
                .values()
                .any(|p| p.session_id == session_id),
            "deny_pending_for_session leaves no pending entry for the session"
        );
    }

    /// VAL-CAPERM-016 (post-abort safety) — a late user "allow" arriving AFTER the
    /// abort fail-closed the request finds NO entry, so it cannot run the tool:
    /// `respond_to_approval(allow)` for an already-denied id is a clean no-op, and
    /// crucially does NOT record the tool on the always-allow set (which would let
    /// a future call skip the prompt).
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

        // The tool was NOT recorded as always-allowed: a post-abort allow can't
        // grant standing consent the abort never honoured.
        assert!(
            !is_session_allow_always(&session_id, "write"),
            "a late allow after abort must NOT record standing consent"
        );
    }

    /// [`deny_pending_for_session`] is session-scoped: it fail-closes ONLY the
    /// named session's pending approvals, leaving another session's pending
    /// request untouched (it still awaits its own answer). And a session with no
    /// pending approval is a clean no-op.
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

        // Deny session A only: A's sender is dropped (Err on the receiver), B is
        // untouched and still awaits.
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

    // -----------------------------------------------------------------------
    // VAL-CAPERM-022 — an out-of-sandbox read takes the SILENT Cancel path: the
    // sandbox (registered BEFORE the permission gate) Cancels it, and the
    // permission extension's emitter is NEVER invoked — no `agent_approval_request`
    // is emitted, so no modal pops for a sandbox escape. The chain ordering is
    // what guarantees this (the first Cancel wins), so the test drives the SAME
    // ordering the host dispatch uses: sandbox first, then permission, stopping
    // at the first Cancel.
    // -----------------------------------------------------------------------

    /// Walk a tool call through the HandBox extension chain in registration order
    /// (sandbox → permission), short-circuiting at the first `Cancel` exactly as
    /// the host dispatch does. Returns the deciding `HookDecision` so a test can
    /// assert WHICH extension stopped the call — and, paired with a recording
    /// emitter, that a `Cancel` upstream of the permission gate left it un-emitted.
    async fn decide_via_chain(
        sandbox: &SandboxExtension,
        permission: &PermissionExtension,
        cx: &ExtensionContext,
        event: &ToolCallEvent,
    ) -> HookDecision {
        // Sandbox runs first; only a `Continue` lets the call fall through to the
        // permission gate. Any other decision (a `Cancel` for a sandbox escape, or
        // a `Replace`) is the chain's verdict and short-circuits — the permission
        // gate is never consulted (mirrors the host's first-decision-wins dispatch).
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

    /// VAL-CAPERM-022 — an out-of-sandbox `read` is silently Cancelled by the
    /// sandbox and NEVER reaches the approval gate: the permission extension's
    /// emitter is not invoked, so no `agent_approval_request` is emitted (no modal
    /// for a sandbox escape). Pins the chain ordering — sandbox BEFORE permission,
    /// first Cancel wins — that makes the escape silent rather than prompted.
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
        // … and crucially the approval emitter was NEVER invoked: a sandbox escape
        // takes the silent Cancel path, it does NOT surface an approval modal.
        assert!(
            recorded.lock().unwrap().is_empty(),
            "an out-of-sandbox read must NOT emit an approval request — the sandbox \
             Cancels it before the permission gate is reached (VAL-CAPERM-022)"
        );
    }

    /// VAL-CAPERM-022 (ls coverage) — same silent-Cancel guarantee for an
    /// out-of-sandbox `ls`: the directory listing escape is Cancelled by the
    /// sandbox first; no approval request is emitted.
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
            "an out-of-sandbox ls must NOT emit an approval request (VAL-CAPERM-022)"
        );
    }

    // -----------------------------------------------------------------------
    // VAL-CAPERM-023 — concurrent runs hold ISOLATED pending approvals: with
    // session A and session B both parked on a pending approval, responding to A
    // resolves ONLY A's await; B stays pending until B is answered in its own
    // right. This is the positive isolation contract (distinct from the abort
    // isolation pinned by `deny_pending_for_session_is_session_scoped_*`): a real
    // user decision for one session must never resolve another's.
    // -----------------------------------------------------------------------

    /// VAL-CAPERM-023 — two sessions each have a pending approval; `respond_to_approval`
    /// for session A resolves ONLY A's await (to its decision), and session B's
    /// request remains pending and awaiting until answered separately. Drives the
    /// REAL hook for both sessions concurrently, resolving A's request by its own
    /// `requestId` (the per-call nth-resolve avoids waking — or deadlocking on —
    /// the wrong session's await).
    #[tokio::test]
    async fn responding_to_one_session_does_not_resolve_another_pending() {
        let session_a = uuid::Uuid::new_v4().to_string();
        let session_b = uuid::Uuid::new_v4().to_string();
        // One shared sink; a distinct extension per HandBox session (as in
        // production — each session has its own extension keyed off its UUID).
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

        // Wait until BOTH requests have landed, then look up A's requestId by its
        // sessionId (order of arrival across the two tasks is non-deterministic, so
        // we cannot rely on index 0 being A).
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

        // B is still pending: its task has NOT finished, and a registry entry for
        // B's session still exists. (We can't `.await` task_b without answering it
        // — it would hang — so we assert it is unresolved, then clean it up.)
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
            "session B's pending entry must survive A's response (VAL-CAPERM-023)"
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

    /// VAL-CAPERM-020 — a DENIED tool is not remembered: `Deny` records nothing on
    /// the always-allow set, so a model re-sending the SAME tool in the SAME
    /// session prompts AGAIN (a second request is emitted and awaited). Contrast
    /// `allow_always` which DOES skip the second prompt.
    #[tokio::test]
    async fn deny_does_not_remember_so_resend_reprompts() {
        let session_id = uuid::Uuid::new_v4().to_string();
        let (emitter, recorded) = recording_emitter();
        let ext = Arc::new(PermissionExtension::new(session_id.clone(), Some(emitter)));

        // First call: the user denies.
        let first = drive_write_for_session(&ext, ApprovalDecision::Deny, &recorded, 1).await;
        assert!(
            matches!(first, HookDecision::Cancel(_)),
            "a denied call Cancels"
        );
        assert_eq!(recorded.lock().unwrap().len(), 1, "first call emitted once");

        // Deny left no standing consent.
        assert!(
            !is_session_allow_always(&session_id, "write"),
            "deny must NOT enter the always-allow set"
        );

        // The model re-sends the SAME tool: it prompts AGAIN (a second request is
        // emitted and awaited) — deny is never remembered.
        let second = drive_write_for_session(&ext, ApprovalDecision::Deny, &recorded, 2).await;
        assert!(matches!(second, HookDecision::Cancel(_)));
        assert_eq!(
            recorded.lock().unwrap().len(),
            2,
            "a denied tool re-prompts on re-send (VAL-CAPERM-020)"
        );
    }
}
