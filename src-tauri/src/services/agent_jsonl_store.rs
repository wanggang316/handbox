//! agent_jsonl_store — read/locate the coding-agent JSONL session that backs a
//! HandBox agent session.
//!
//! M3 makes JSONL the authoritative transcript store. The coding-agent
//! [`SessionManager`] persists each session as `<base>/sessions/<flattened-cwd>/
//! <id>.jsonl`, where `<base>` is the Tauri app-data dir (the same `base_dir`
//! [`crate::services::coding_agent_session`] wires into the session). This module
//! is the thin, side-effect-honest seam HandBox uses to:
//!
//! 1. Locate / pre-create the JSONL file for a HandBox session so the
//!    coding-agent `resume_session` path always finds it — which is how a
//!    HandBox session UUID becomes the JSONL session id (the file is named
//!    `<uuid>.jsonl` and its header carries `id == <uuid>`), giving us "same
//!    HandBox session → same JSONL → multi-turn append" without an id map.
//! 2. Read a session's transcript back as the frontend-shaped
//!    [`AgentSessionMessage`] list (used by `agent_session_messages`).
//! 3. Read a session's activity metadata (message count / last-activity ts /
//!    label) from its `SessionInfo` (used to fill the sidebar list).
//!
//! Why we don't use [`SessionManager::list_all`]: that helper scans
//! `<root>/.hand/agent/sessions/`, the *home-based* layout. With a `base_dir`
//! override the writer lands files under `<base>/sessions/` instead, so a
//! direct scan of that directory is the only path that sees them.
//!
//! Reuse, not reinvention: every read goes through the upstream
//! `SessionManager` / `build_session_info` / `build_context`, so HandBox never
//! re-parses JSONL or re-implements the entry tree.

use std::path::{Path, PathBuf};

use hand_ai_model::Message;
use hand_coding_agent::core::session_manager::{
    build_session_info, SessionHeader, CURRENT_SESSION_VERSION,
};
use hand_coding_agent::SessionManager;

use crate::models::AppError;
use crate::storage::types::{AgentSessionMessage, Timestamp, UUID};

/// Activity metadata for a JSONL-backed session, lifted from its `SessionInfo`.
///
/// These are the fields the sidebar list needs that live in the transcript,
/// not in the SQLite config row: how many messages the session has, when it was
/// last active, and the latest session label (if the agent renamed it). The
/// SQLite row stays the source of truth for *config* (model/provider/tools/
/// project attachment); JSONL is the source of truth for *activity*.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonlActivity {
    /// Number of `Message` entries in the JSONL (excludes header/labels/etc.).
    pub message_count: i32,
    /// Latest message timestamp (millis). `None` for a session with no message
    /// entries yet — the caller maps this straight onto `last_message_at`, so a
    /// freshly-created (or only-just-resumed) session correctly reports `null`
    /// rather than 0 (the sidebar's `lastMessageAt ?? createdAt` coalescing
    /// depends on a genuine `null`, see `agentGrouping.ts`).
    pub last_message_at: Option<Timestamp>,
    /// Latest non-empty session label, if the agent renamed the session.
    pub name: Option<String>,
}

/// Directory the JSONL session for `cwd` lives in under `base_dir`
/// (`<base>/sessions/<flattened-cwd>/`). Mirrors the writer side exactly
/// ([`SessionManager::default_session_dir_with_base`] with `Some(base)`), so the
/// reader and writer never disagree about where a file is.
pub fn session_dir(base_dir: &Path, cwd: &Path) -> PathBuf {
    SessionManager::default_session_dir_with_base(Some(base_dir), cwd)
}

/// Resolve the `cwd` a session's JSONL is keyed by, given its (optional)
/// stored `working_dir` and the app data dir.
///
/// This MUST match the cwd the writer used (`coding_agent_session::
/// config_from_rows`): a session with no working dir roots its agent at the app
/// data dir, so its JSONL lands under the flattened-`app_data_dir` subdir — the
/// reader has to encode the same cwd to find the file. Diverging here would
/// silently look in the wrong `<flattened-cwd>` directory and report every
/// session as transcript-less.
pub fn session_cwd(working_dir: Option<&str>, app_data_dir: &Path) -> PathBuf {
    // Faithful to the writer (`config_from_rows`): map the stored working_dir
    // through verbatim, falling back to app_data_dir only when it is absent.
    // `validate_working_dir` never stores an empty string (empty → None), so a
    // plain `map`/`unwrap_or_else` matches the writer byte-for-byte without an
    // empty-string special case that could diverge from the cwd actually used.
    working_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data_dir.to_path_buf())
}

/// Absolute path to the JSONL file backing `session_id` under `base_dir`/`cwd`.
/// The file is named `<session_id>.jsonl`, so a HandBox UUID names the file
/// (and, once [`ensure_session_file`] writes the header, equals the header id).
pub fn session_path(base_dir: &Path, cwd: &Path, session_id: &str) -> PathBuf {
    session_dir(base_dir, cwd).join(format!("{session_id}.jsonl"))
}

/// Ensure a JSONL file named `<session_id>.jsonl` exists under `base_dir`/`cwd`,
/// creating it (with a minimal `SessionHeader` whose `id == session_id`) when it
/// is absent. Returns the file path.
///
/// This is the linchpin of id reuse. The coding-agent `create_in` path mints
/// its OWN `s_…` id and ignores any caller id, so HandBox cannot ask it to
/// create a file named after a HandBox UUID. Instead we pre-seed the header
/// ourselves and then drive the session via `resume_session = <uuid>`: the
/// resume path resolves `<dir>/<uuid>.jsonl`, opens it, and every subsequent
/// `append_message` writes into that same file. The header we stamp is
/// byte-compatible with what `SessionManager::create_in` would have written
/// (version `CURRENT_SESSION_VERSION`, the session id, a creation timestamp, the
/// cwd), so `SessionManager::open` accepts it as a valid session.
///
/// `created_at` (millis) is stamped as the header `timestamp` — it MUST be the
/// session's SQLite `created_at`, NOT the current wall clock. The sidebar's
/// activity key coalesces `lastMessageAt ?? createdAt`, where `createdAt` comes
/// from the SQLite row; pinning the header timestamp to that same value keeps
/// the JSONL header's reported creation time (`SessionInfo.timestamp`) equal to
/// the session's real creation time rather than its first-run time, which is the
/// invariant VAL-CASESS-007 relies on (and the correct comparison key for the
/// later migration's `coalesce(last_message_at, created_at)`).
///
/// Idempotent: an already-existing file is left untouched (so a second turn
/// appends to the first turn's transcript rather than clobbering it).
///
/// The header write is ATOMIC (temp-file + rename, see [`write_file_atomic`]):
/// a crash or write failure mid-seed never leaves a half-written `<id>.jsonl`
/// behind — the file either exists complete (with its one header line) or does
/// not exist at all, never as a truncated / header-less ghost
/// (VAL-CASESS-020 / VAL-CASESS-025).
pub fn ensure_session_file(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
    created_at: i64,
) -> Result<PathBuf, AppError> {
    let dir = session_dir(base_dir, cwd);
    let path = dir.join(format!("{session_id}.jsonl"));
    if path.exists() {
        return Ok(path);
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(&format!("failed to create session dir: {e}")))?;

    let line = header_line(session_id, cwd, created_at)?;
    write_file_atomic(&path, &format!("{line}\n"))?;
    Ok(path)
}

/// Serialize the one-line `{"type":"session","data":<SessionHeader>}` header for
/// a session. The on-disk shape is the `SessionEntry::Session` envelope; we
/// build it via the same serde path SessionManager uses.
fn header_line(session_id: &str, cwd: &Path, created_at: i64) -> Result<String, AppError> {
    let header = SessionHeader {
        version: CURRENT_SESSION_VERSION,
        id: session_id.to_string(),
        timestamp: created_at,
        cwd: cwd.to_string_lossy().to_string(),
        parent_session: None,
    };
    serde_json::to_string(&SessionEntryHeader::from(header))
        .map_err(|e| AppError::internal_error(&format!("failed to serialize session header: {e}")))
}

/// Write `content` to `path` ATOMICALLY: serialize into a sibling temp file in
/// the SAME directory, flush+sync it, then `rename` it onto the final path. The
/// rename is atomic within a filesystem, so a reader (and a crash / power loss)
/// only ever sees the OLD complete file or the NEW complete file — never a
/// half-written one.
///
/// Why this matters here (VAL-CASESS-020 / VAL-CASESS-025): every producer of a
/// `<id>.jsonl`'s contents (header seed, full-transcript migration rewrite) goes
/// through this, so a write that fails partway — a full disk, a read-only dir,
/// a kill mid-flush — leaves the disk in one of exactly two states: the prior
/// complete file (if any) or no temp leftover. On ANY failure the temp file is
/// removed before returning, so a crashed run never strands a `.tmp` ghost the
/// next pass would have to reason about.
///
/// The temp file lives in the same directory as the target (not `/tmp`) so the
/// `rename` stays within one filesystem (cross-device rename is not atomic and
/// would fall back to copy+delete).
fn write_file_atomic(path: &Path, content: &str) -> Result<(), AppError> {
    use std::io::Write;

    let dir = path.parent().ok_or_else(|| {
        AppError::internal_error(&format!(
            "session jsonl path has no parent directory: {}",
            path.display()
        ))
    })?;
    // A unique sibling temp name keeps concurrent writers of DIFFERENT sessions
    // from colliding on one temp path; the final `rename` is the only step that
    // touches the real file name.
    let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));

    // Scope the file handle so it is closed before the rename; on every error
    // path below we remove the temp file so no partial ghost is left behind.
    let write_result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::internal_error(&format!(
            "failed to write session jsonl atomically: {e}"
        )));
    }

    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::internal_error(&format!(
            "failed to commit session jsonl atomically: {e}"
        )));
    }
    Ok(())
}

/// Write a session's COMPLETE transcript (header + every message line) to its
/// `<id>.jsonl` in one ATOMIC step, overwriting any existing file.
///
/// This is the migration's full-file writer. Unlike [`append_message_at`] (which
/// appends one line at a time and so can only ever GROW a file), this rebuilds
/// the whole file from a known-good message set and commits it via
/// [`write_file_atomic`]. That is what makes the migration crash-convergent
/// (VAL-CASESS-025): a session whose `<id>.jsonl` is half-written, header-less,
/// or has the wrong message count can be REWRITTEN to a complete file in a
/// single rename, with no window in which a truncated official file is visible.
///
/// Each entry stamps the message's own historical `timestamp` (the SQLite
/// per-row `created_at`), NOT the wall clock — preserving the cross-session
/// activity ordering exactly as [`append_message_at`] does (VAL-CASESS-012).
/// The header stamps `created_at` (the session's SQLite creation time), keeping
/// `SessionInfo.timestamp == createdAt`.
pub fn write_transcript_atomic(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
    created_at: i64,
    messages: &[(Message, i64)],
) -> Result<(), AppError> {
    let dir = session_dir(base_dir, cwd);
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::internal_error(&format!("failed to create session dir: {e}")))?;
    let path = dir.join(format!("{session_id}.jsonl"));

    let mut content = String::new();
    content.push_str(&header_line(session_id, cwd, created_at)?);
    content.push('\n');
    for (message, timestamp) in messages {
        let entry = MessageEntryLine::Message(MessageEntryData {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            timestamp: *timestamp,
        });
        let line = serde_json::to_string(&entry).map_err(|e| {
            AppError::internal_error(&format!("failed to serialize migrated message entry: {e}"))
        })?;
        content.push_str(&line);
        content.push('\n');
    }

    write_file_atomic(&path, &content)
}

/// A `{"type":"session","data":<SessionHeader>}` envelope, matching the
/// `SessionEntry::Session` on-disk shape (`#[serde(tag="type", content="data",
/// rename_all="snake_case")]`). `SessionEntry` itself is not re-exported, so we
/// mirror only the single header-line shape we need to write. Reading is always
/// done through the upstream parser (`build_session_info` / `SessionManager`),
/// so this never has to deserialize.
#[derive(serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum SessionEntryHeader {
    Session(SessionHeader),
}

impl From<SessionHeader> for SessionEntryHeader {
    fn from(h: SessionHeader) -> Self {
        SessionEntryHeader::Session(h)
    }
}

/// The `data` payload of a `{"type":"message","data":{..}}` JSONL line, mirroring
/// the `SessionEntry::Message` variant's serde shape. We write this directly (not
/// via [`SessionManager::append_message`]) ONLY on the migration path, because we
/// must control the entry-level `timestamp`: `SessionManager::append_message`
/// stamps `Utc::now()`, which would make every replayed legacy message look like
/// it was sent at migration time and so destroy the cross-session activity
/// ordering (`build_session_info` derives `SessionInfo.modified` from the max
/// entry timestamp — VAL-CASESS-012). Stamping the SQLite per-message
/// `created_at` instead keeps each session's last-activity key equal to its
/// pre-migration `last_message_at`.
#[derive(serde::Serialize)]
struct MessageEntryData<'a> {
    id: String,
    message: &'a Message,
    timestamp: i64,
}

/// A `{"type":"message","data":<MessageEntryData>}` envelope, matching the
/// `SessionEntry::Message` on-disk shape (same tag/content/snake_case as the
/// header). `parent_id` is omitted (the variant skips it when `None`), matching a
/// flat-list session with no parentage — which is exactly what a migrated legacy
/// transcript is.
#[derive(serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum MessageEntryLine<'a> {
    Message(MessageEntryData<'a>),
}

/// Append a single `Message` to a session's JSONL with an EXPLICIT entry-level
/// `timestamp`, ensuring the file (header seeded with `created_at`) first.
///
/// This is the migration-only writer (see [`MessageEntryData`] for why it cannot
/// go through [`SessionManager::append_message`]): the entry timestamp must be
/// the message's real send time (the SQLite `created_at`) so the migrated
/// session's last-activity key matches its pre-migration value and relative
/// session ordering is preserved (VAL-CASESS-012). The entry id is a fresh UUID —
/// the format is opaque (the upstream reader only matches ids for compaction,
/// which migrated transcripts never carry), so uniqueness is all that matters.
///
/// The append is plain line-appending (the same `\n`-terminated, one-entry-per-
/// line shape `ensure_session_file` writes the header in), reusing the upstream
/// `SessionEntry` envelope shape via [`MessageEntryLine`] rather than re-parsing.
pub fn append_message_at(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
    created_at: i64,
    message: &Message,
    timestamp: i64,
) -> Result<(), AppError> {
    let path = ensure_session_file(base_dir, cwd, session_id, created_at)?;
    let entry = MessageEntryLine::Message(MessageEntryData {
        id: uuid::Uuid::new_v4().to_string(),
        message,
        timestamp,
    });
    let line = serde_json::to_string(&entry).map_err(|e| {
        AppError::internal_error(&format!("failed to serialize migrated message entry: {e}"))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .map_err(|e| {
            AppError::internal_error(&format!("failed to open session jsonl for append: {e}"))
        })?;
    use std::io::Write;
    writeln!(file, "{line}").map_err(|e| {
        AppError::internal_error(&format!("failed to append migrated message entry: {e}"))
    })?;
    Ok(())
}

/// Load a session's transcript from JSONL as the frontend-shaped
/// [`AgentSessionMessage`] list, or `None` when no JSONL file exists yet (the
/// caller then falls back to the legacy SQLite transcript for pre-M3 sessions).
///
/// The messages come from [`SessionManager::build_context`], which yields the
/// post-compaction `Vec<Message>` — the same context the agent itself would
/// see, so what the UI renders matches what the model was actually fed. Each
/// `Message` is serialized into `AgentSessionMessage.payload` verbatim (so tool
/// calls and thinking blocks ride through inside the assistant message's content
/// blocks), with `role` taken from the serialized `role` tag and `seq` assigned
/// 0-based in context order.
pub fn load_transcript(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
) -> Result<Option<Vec<AgentSessionMessage>>, AppError> {
    let path = session_path(base_dir, cwd, session_id);
    if !path.exists() {
        return Ok(None);
    }
    let manager = SessionManager::open(&path)
        .map_err(|e| AppError::internal_error(&format!("failed to open session jsonl: {e}")))?;
    let messages = manager.build_context();
    Ok(Some(messages_to_rows(session_id, &messages)?))
}

/// Append a session label (display name) to a session's JSONL, making the new
/// name the authoritative source the activity overlay reads back.
///
/// Why this exists: M3 makes the JSONL session label the authoritative display
/// name whenever present ([`session_activity`]'s `name` → the list/get overlay's
/// `session.name`). A rename that only writes the SQLite `name` would be visually
/// overwritten by a stale JSONL label (the title the agent auto-assigned). So a
/// rename must also append a fresh label here; the most-recent label wins on
/// read-back ([`SessionManager::label`] scans newest-first).
///
/// The file is ensured first so renaming a session that has never been resumed
/// (no `<id>.jsonl` yet) still takes effect — the header is seeded (stamping
/// `created_at` as its timestamp, see [`ensure_session_file`]), then the label
/// appended. Appending a `Label` entry does NOT add a `Message`, so this leaves
/// `session_activity`'s `message_count` and `last_message_at` untouched (a
/// rename must never manufacture "activity").
///
/// `created_at` is threaded through to [`ensure_session_file`] so that renaming
/// a never-resumed session seeds its header with the session's real creation
/// time, keeping `SessionInfo.timestamp == createdAt` even for a first-ever
/// rename (VAL-CASESS-007 / VAL-CASESS-008). For an already-existing file the
/// value is unused (the seed is idempotent).
pub fn append_label(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
    label: &str,
    created_at: i64,
) -> Result<(), AppError> {
    let path = ensure_session_file(base_dir, cwd, session_id, created_at)?;
    let mut manager = SessionManager::open(&path)
        .map_err(|e| AppError::internal_error(&format!("failed to open session jsonl: {e}")))?;
    manager
        .append_label(label)
        .map_err(|e| AppError::internal_error(&format!("failed to append session label: {e}")))?;
    Ok(())
}

/// Remove the JSONL file backing `session_id` under `base_dir`/`cwd`, if it
/// exists. A session with no JSONL file (pre-M3 / never-resumed) is a clean
/// no-op — the absence is the desired post-state, so it is not an error.
///
/// Used on delete to keep the on-disk transcript store in step with the SQLite
/// row: without this, deleting a session would leave an orphan `<id>.jsonl`
/// behind. The caller treats failure as best-effort (the authoritative SQLite
/// row delete is what removes the session from the list).
pub fn delete_session_file(base_dir: &Path, cwd: &Path, session_id: &str) -> Result<(), AppError> {
    let path = session_path(base_dir, cwd, session_id);
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path)
        .map_err(|e| AppError::internal_error(&format!("failed to delete session jsonl: {e}")))
}

/// Read activity metadata (message count / last activity / label) for a
/// JSONL-backed session, or `None` when no JSONL file exists yet.
pub fn session_activity(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
) -> Result<Option<JsonlActivity>, AppError> {
    let path = session_path(base_dir, cwd, session_id);
    if !path.exists() {
        return Ok(None);
    }
    let info = build_session_info(&path)
        .map_err(|e| AppError::internal_error(&format!("failed to read session info: {e}")))?;
    let Some(info) = info else {
        // Header-less / corrupt file: treat as "no JSONL activity".
        return Ok(None);
    };

    // `SessionInfo.modified` prefers the latest message timestamp and falls
    // back to the file mtime; we only want a genuine last-MESSAGE timestamp for
    // the activity key, so a session with zero messages reports `None` (→ the
    // sidebar coalesces to createdAt) rather than the file mtime.
    let last_message_at = if info.message_count == 0 {
        None
    } else {
        Some(info.modified)
    };

    Ok(Some(JsonlActivity {
        message_count: i32::try_from(info.message_count).unwrap_or(i32::MAX),
        last_message_at,
        name: info.name,
    }))
}

/// Convert a context `Vec<Message>` into the frontend-shaped
/// [`AgentSessionMessage`] rows. `role` is the serialized `Message` tag
/// (`user` / `assistant` / `toolResult`); `seq` is the 0-based context index;
/// `created_at` is the message's own timestamp.
fn messages_to_rows(
    session_id: &str,
    messages: &[Message],
) -> Result<Vec<AgentSessionMessage>, AppError> {
    let mut rows = Vec::with_capacity(messages.len());
    for (seq, message) in messages.iter().enumerate() {
        let payload = serde_json::to_value(message).map_err(|e| {
            AppError::internal_error(&format!("failed to serialize session message: {e}"))
        })?;
        let role = payload
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant")
            .to_string();
        let created_at = message_timestamp(message);
        rows.push(AgentSessionMessage {
            // The JSONL transcript has no per-message UUID HandBox owns; the
            // (session_id, seq) pair is the stable identity the UI keys off, so
            // a synthetic deterministic id keeps rows distinct without inventing
            // a fresh uuid on every read (which would churn keyed list diffs).
            id: format!("{session_id}:{seq}"),
            session_id: session_id.to_string(),
            seq: seq as i64,
            role,
            payload,
            created_at,
        });
    }
    Ok(rows)
}

/// The message's own `timestamp` field (millis since epoch), regardless of
/// variant — used as the transcript row's `created_at`. The model crate stores
/// timestamps as `u64`; `Timestamp` (the HandBox wire type) is `i64`, so we
/// saturate on the astronomically-distant overflow rather than panic.
fn message_timestamp(message: &Message) -> Timestamp {
    let ts: u64 = match message {
        Message::User(m) => m.timestamp,
        Message::Assistant(m) => m.timestamp,
        Message::ToolResult(m) => m.timestamp,
    };
    Timestamp::try_from(ts).unwrap_or(Timestamp::MAX)
}

/// Alias for the SQLite session UUID this module treats as the JSONL session id.
/// Kept as a doc anchor so call sites reading `UUID` see the intent.
#[allow(dead_code)]
pub type JsonlSessionId = UUID;

#[cfg(test)]
mod tests {
    use super::*;
    use hand_ai_model::{
        Api, AssistantContentBlock, AssistantMessage, StopReason, TextContent, ToolCall, Usage,
        UserMessage,
    };
    use tempfile::TempDir;

    /// A fixed, recognizable `created_at` (millis) the tests stamp into the
    /// header so they can assert `SessionInfo.timestamp == created_at` without
    /// racing the wall clock. Picked to be obviously NOT a "now" value.
    const TEST_CREATED_AT: i64 = 1_700_000_000_000;

    /// Build a session under `base`/`cwd` via the REAL coding-agent
    /// `SessionManager` resume path (the same path production drives), seeded
    /// with our header, then append messages — so the test exercises the actual
    /// JSONL writer, not a hand-rolled file.
    fn open_resumed(base: &Path, cwd: &Path, session_id: &str) -> SessionManager {
        ensure_session_file(base, cwd, session_id, TEST_CREATED_AT).expect("header seeded");
        let path = session_path(base, cwd, session_id);
        SessionManager::open(&path).expect("resume opens the seeded file")
    }

    fn user_msg(text: &str) -> Message {
        Message::User(UserMessage::new_text(text.to_string()))
    }

    fn assistant_with_tool_and_thinking(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![
                AssistantContentBlock::Text(TextContent::new(text.to_string())),
                AssistantContentBlock::ToolCall(ToolCall {
                    content_type: "toolCall".into(),
                    id: "tc-1".into(),
                    name: "read".into(),
                    arguments: serde_json::json!({ "path": "x.txt" }),
                    thought_signature: None,
                }),
            ],
            api: Api::OpenAICompletions,
            provider: hand_ai_model::types::Provider::OpenAI,
            model: "gpt-4o".into(),
            usage: Usage::default(),
            stop_reason: StopReason::ToolUse,
            error_message: None,
            timestamp: 1234,
            response_model: None,
            response_id: None,
            diagnostics: None,
        })
    }

    /// VAL-CASESS-001 (write leg): a freshly-seeded session lands a real
    /// `<id>.jsonl` whose file name AND header id equal the HandBox session id,
    /// proving id reuse — the HandBox UUID becomes the JSONL session id with no
    /// mapping.
    #[test]
    fn ensure_session_file_names_file_and_header_after_handbox_id() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "11111111-2222-3333-4444-555555555555";

        let path = ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT)
            .expect("seeds header");
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            format!("{id}.jsonl"),
            "the JSONL file must be named after the HandBox session id"
        );

        // The upstream reader accepts it and reports our id as the header id.
        let manager = SessionManager::open(&path).expect("opens as a valid session");
        assert_eq!(manager.id(), id, "header id must equal the HandBox id");
    }

    /// VAL-CASESS-007 (header-stamp leg): the header `timestamp` `ensure_session_file`
    /// writes is the `created_at` the caller passed — the session's real SQLite
    /// creation time — NOT the current wall clock. `SessionInfo.timestamp` (the
    /// header timestamp the activity overlay would surface as the session's
    /// createdAt) therefore equals `created_at`, so the sidebar's
    /// `lastMessageAt ?? createdAt` coalescing keeps an empty session anchored to
    /// its creation time rather than its first-run time.
    #[test]
    fn ensure_session_file_stamps_header_timestamp_from_created_at_not_now() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-created-at";

        let path = ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT)
            .expect("seeds header");

        // Read the header timestamp back through the upstream parser (the same
        // value the activity overlay surfaces as the session's createdAt).
        let info = build_session_info(&path)
            .expect("info reads")
            .expect("a seeded session has a header");
        assert_eq!(
            info.timestamp, TEST_CREATED_AT,
            "header timestamp must equal the created_at we stamped, not now()"
        );
    }

    /// Re-seeding an existing session is idempotent: the second call neither
    /// errors nor truncates, so a multi-turn session keeps its prior transcript.
    #[test]
    fn ensure_session_file_is_idempotent_and_preserves_content() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-idem";

        let mut mgr = open_resumed(base.path(), cwd.path(), id);
        mgr.append_message(user_msg("hello")).unwrap();

        // Second "ensure" (e.g. the start of turn 2) must not clobber turn 1.
        let path =
            ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT).expect("idempotent");
        let reopened = SessionManager::open(&path).unwrap();
        assert_eq!(
            reopened.message_count(),
            1,
            "re-ensuring an existing file must preserve its messages"
        );
    }

    /// VAL-CASESS-003: a round-trip through the real writer + reader restores
    /// the full transcript — user message AND an assistant message carrying a
    /// tool call — as frontend-shaped rows with the right roles and order.
    #[test]
    fn load_transcript_restores_messages_with_tool_calls_in_order() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-transcript";

        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id);
            mgr.append_message(user_msg("read x.txt please")).unwrap();
            mgr.append_message(assistant_with_tool_and_thinking("on it"))
                .unwrap();
        }

        let rows = load_transcript(base.path(), cwd.path(), id)
            .expect("read ok")
            .expect("a seeded session has a transcript");

        assert_eq!(rows.len(), 2, "both messages restored");
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[0].seq, 0);
        assert_eq!(rows[1].role, "assistant");
        assert_eq!(rows[1].seq, 1);

        // The tool call rides through inside the assistant payload's content
        // blocks (serde tag "toolcall"), proving tool calls survive the trip.
        let blocks = rows[1].payload.get("content").unwrap().as_array().unwrap();
        assert!(
            blocks
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("toolcall")),
            "assistant transcript row must carry the tool call content block, got: {:?}",
            rows[1].payload
        );
    }

    /// A session with no JSONL file yet reads back as `None` on both seams, so
    /// the caller cleanly falls back to SQLite for pre-M3 sessions.
    #[test]
    fn absent_jsonl_reads_as_none() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        assert!(
            load_transcript(base.path(), cwd.path(), "never-created")
                .unwrap()
                .is_none(),
            "no jsonl file → transcript reads as None (SQLite fallback)"
        );
        assert!(
            session_activity(base.path(), cwd.path(), "never-created")
                .unwrap()
                .is_none(),
            "no jsonl file → activity reads as None"
        );
    }

    /// VAL-CASESS-002 (activity leg): after appending, `session_activity`
    /// reports the message count and a real last-activity timestamp; the latest
    /// label surfaces as `name`. A messageless session reports `last_message_at:
    /// None` so the sidebar coalesces to createdAt (never 0 / "56 years ago").
    #[test]
    fn session_activity_reports_count_label_and_null_for_empty() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-activity";

        // Empty session: count 0, last_message_at None.
        ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT).unwrap();
        let empty = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("file exists");
        assert_eq!(empty.message_count, 0);
        assert_eq!(
            empty.last_message_at, None,
            "a messageless session must report null last activity, never 0"
        );

        // After two messages + a label, count and last activity are real.
        {
            let mut mgr = SessionManager::open(&session_path(base.path(), cwd.path(), id)).unwrap();
            mgr.append_message(user_msg("first")).unwrap();
            mgr.append_message(user_msg("second")).unwrap();
            mgr.append_label("My Renamed Session").unwrap();
        }
        let active = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("file exists");
        assert_eq!(active.message_count, 2);
        assert!(
            active.last_message_at.is_some(),
            "a session with messages must report a real last-activity timestamp"
        );
        assert_eq!(active.name.as_deref(), Some("My Renamed Session"));
    }

    /// `append_message_at` (the migration-only writer) stamps the entry-level
    /// timestamp it is given — NOT the wall clock — and that timestamp is the
    /// activity key the reader surfaces. This is the linchpin of VAL-CASESS-012's
    /// ordering: a replayed legacy message must report its real send time, not
    /// the migration moment, or every migrated session would collapse to the same
    /// "now" and lose relative order. The message round-trips back through the
    /// upstream reader, proving the hand-written line is a valid Message entry.
    #[test]
    fn append_message_at_stamps_given_timestamp_as_activity_key() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-append-at";
        // A clearly-historical timestamp, unmistakably not a "now" value.
        let entry_ts: i64 = 1_600_000_000_000;

        append_message_at(
            base.path(),
            cwd.path(),
            id,
            TEST_CREATED_AT,
            &user_msg("replayed from sqlite"),
            entry_ts,
        )
        .expect("append a migrated message");

        // The activity key (max entry timestamp) is exactly what we stamped.
        let activity = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("file exists");
        assert_eq!(activity.message_count, 1);
        assert_eq!(
            activity.last_message_at,
            Some(entry_ts),
            "the entry timestamp must equal the value passed, not the wall clock"
        );

        // The message itself round-trips through the upstream reader intact.
        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .expect("a transcript");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].role, "user");

        // A second append lands in the same file and the activity key advances to
        // the newer entry timestamp.
        append_message_at(
            base.path(),
            cwd.path(),
            id,
            TEST_CREATED_AT,
            &user_msg("second"),
            entry_ts + 5_000,
        )
        .expect("append a second migrated message");
        let activity2 = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .unwrap();
        assert_eq!(activity2.message_count, 2);
        assert_eq!(activity2.last_message_at, Some(entry_ts + 5_000));
    }

    /// Two turns against the SAME HandBox id append to the SAME file rather than
    /// minting a new one — the core "multi-turn append, not re-create" contract.
    #[test]
    fn two_turns_same_id_append_to_one_file() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-two-turns";

        // Turn 1.
        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id);
            mgr.append_message(user_msg("turn one")).unwrap();
        }
        // Turn 2: ensure (idempotent) then resume + append again.
        {
            ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT).unwrap();
            let mut mgr = SessionManager::open(&session_path(base.path(), cwd.path(), id)).unwrap();
            mgr.append_message(user_msg("turn two")).unwrap();
        }

        // Exactly one file under the session dir, holding both turns.
        let dir = session_dir(base.path(), cwd.path());
        let jsonl_files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
            .collect();
        assert_eq!(
            jsonl_files.len(),
            1,
            "two turns on the same id must reuse one jsonl, got: {jsonl_files:?}"
        );

        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .unwrap();
        assert_eq!(rows.len(), 2, "both turns' messages persisted in one file");
    }

    /// VAL-CASESS-004 (write leg): renaming a session appends a label that
    /// becomes the authoritative display name `session_activity` reads back —
    /// even when the session already carried an (older) agent-assigned label.
    /// The newest label wins, so the overlay reflects exactly the user's input.
    #[test]
    fn append_label_makes_new_name_authoritative_over_an_older_label() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-rename";

        // The agent auto-titled the session at some earlier point.
        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id);
            mgr.append_message(user_msg("hi")).unwrap();
            mgr.append_label("Old Agent Title").unwrap();
        }
        assert_eq!(
            session_activity(base.path(), cwd.path(), id)
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("Old Agent Title"),
        );

        // User renames → the new label must take over.
        append_label(
            base.path(),
            cwd.path(),
            id,
            "User Chosen Name",
            TEST_CREATED_AT,
        )
        .expect("rename label ok");
        assert_eq!(
            session_activity(base.path(), cwd.path(), id)
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("User Chosen Name"),
            "the most recent label must win on read-back",
        );
    }

    /// VAL-CASESS-004 (empty-session leg) + the "rename never manufactures
    /// activity" invariant (the geological base for VAL-CASESS-008): renaming a
    /// session that has never been resumed (no file yet) ensures the file,
    /// appends the label, and the name reflects the input — while
    /// `last_message_at` stays `None` and `message_count` stays 0 (a label is
    /// not a message).
    ///
    /// Also covers VAL-CASESS-007 (rename-seed leg): the header `ensure_session_file`
    /// seeds on a first-ever rename carries the `created_at` we passed, NOT the
    /// rename moment — so an empty session's createdAt (`SessionInfo.timestamp`)
    /// stays its real creation time even when its first on-disk write is a rename.
    #[test]
    fn append_label_on_empty_session_sets_name_without_creating_activity() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-empty-rename";

        // No file exists yet: append_label must seed it then label it, stamping
        // the seeded header with the created_at we pass (not "now").
        append_label(
            base.path(),
            cwd.path(),
            id,
            "Named Before Any Message",
            TEST_CREATED_AT,
        )
        .expect("rename of an empty session ensures the file and labels it");

        let info = build_session_info(&session_path(base.path(), cwd.path(), id))
            .unwrap()
            .expect("ensure-on-rename created the file with a header");
        assert_eq!(
            info.timestamp, TEST_CREATED_AT,
            "a first-ever rename must seed the header with created_at, not the rename moment",
        );

        let activity = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("ensure-on-rename created the file");
        assert_eq!(
            activity.name.as_deref(),
            Some("Named Before Any Message"),
            "rename takes effect even on a never-resumed session",
        );
        assert_eq!(
            activity.last_message_at, None,
            "a rename must not manufacture a last-activity timestamp",
        );
        assert_eq!(
            activity.message_count, 0,
            "a label is not a message — message_count must stay 0",
        );
    }

    /// VAL-CASESS-005 (file-cleanup leg): deleting a session removes its JSONL
    /// file so no orphan transcript is left on disk.
    #[test]
    fn delete_session_file_removes_existing_jsonl() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-delete";

        let path = ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT).unwrap();
        assert!(path.exists(), "precondition: the file was created");

        delete_session_file(base.path(), cwd.path(), id).expect("delete ok");
        assert!(
            !path.exists(),
            "deleting a session must remove its JSONL file (no orphan)",
        );
    }

    /// VAL-CASESS-005 (no-op leg): deleting a session that has no JSONL file
    /// (pre-M3 / never resumed) is a clean no-op, not an error — the absence is
    /// already the desired post-state.
    #[test]
    fn delete_session_file_absent_is_noop() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        delete_session_file(base.path(), cwd.path(), "never-created")
            .expect("deleting a session with no JSONL file must be a clean no-op");
    }

    /// VAL-CASESS-016: a `<id>.jsonl` whose body interleaves a line of garbage
    /// between valid message lines reads back as the valid messages ONLY — the
    /// malformed line is silently skipped by the upstream `parse_session_entries`
    /// (it is not parsed as a message and not surfaced). `load_transcript` returns
    /// the surviving messages in order; `session_activity` reports the surviving
    /// count and the MAX of the surviving message timestamps as the activity key —
    /// the bad line counts neither as 0 nor as "newest".
    #[test]
    fn malformed_jsonl_line_is_skipped_on_read_without_polluting_activity() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-garbage-line";

        // Seed a real header, then build the body by hand so we can inject a
        // garbage line between two valid messages with KNOWN timestamps. The
        // valid messages carry historical timestamps; the bad line, if it leaked
        // into the activity key, would either zero it out or (as junk) be ignored
        // — we assert the key equals exactly the MAX of the two valid timestamps.
        let early_ts: i64 = 1_600_000_000_000;
        let late_ts: i64 = 1_650_000_000_000;
        append_message_at(
            base.path(),
            cwd.path(),
            id,
            TEST_CREATED_AT,
            &user_msg("first valid"),
            early_ts,
        )
        .unwrap();

        // Inject a line of pure garbage directly after the first message.
        {
            use std::io::Write;
            let path = session_path(base.path(), cwd.path(), id);
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "this is not json at all {{").unwrap();
        }

        append_message_at(
            base.path(),
            cwd.path(),
            id,
            TEST_CREATED_AT,
            &user_msg("second valid"),
            late_ts,
        )
        .unwrap();

        // The transcript reads back as the two VALID messages only — the garbage
        // line never materializes as a row.
        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .expect("a seeded session has a transcript");
        assert_eq!(
            rows.len(),
            2,
            "malformed line must be skipped, leaving exactly the two valid messages"
        );
        assert!(rows.iter().all(|r| r.role == "user"));

        // The activity key is the MAX of the surviving message timestamps — the
        // bad line is neither treated as 0 (would zero the key) nor as latest.
        let activity = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("file exists");
        assert_eq!(
            activity.message_count, 2,
            "count reflects only the valid messages"
        );
        assert_eq!(
            activity.last_message_at,
            Some(late_ts),
            "activity key must equal the max VALID message timestamp, not the bad line"
        );
    }

    /// VAL-CASESS-018: a `<id>.jsonl` whose FIRST line is not a valid session
    /// header (a corrupt / header-less file) reports `session_activity == None` —
    /// the upstream `build_session_info` returns `Ok(None)` for a header-less
    /// file, which this maps to "no JSONL activity". The overlay then keeps the
    /// SQLite values rather than rendering a blank/empty activity row.
    #[test]
    fn bad_header_jsonl_reports_no_activity_not_a_blank_row() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-bad-header";

        // Hand-write a file whose first line is junk (not a session header),
        // followed by what would otherwise be a valid-looking line.
        let dir = session_dir(base.path(), cwd.path());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::write(&path, "not a header line\n{\"type\":\"label\"}\n").unwrap();
        assert!(path.exists(), "precondition: the bad-header file exists");

        assert!(
            session_activity(base.path(), cwd.path(), id)
                .unwrap()
                .is_none(),
            "a header-less / corrupt JSONL must report no activity (not a blank row)"
        );
    }

    /// VAL-CASESS-020 / VAL-CASESS-025 (atomic-write leg): `write_transcript_atomic`
    /// writes a session's full transcript as ONE complete file, and reading it
    /// back through the upstream parser restores exactly the messages written, in
    /// order, with the activity key equal to the max stamped timestamp. This is
    /// the all-or-nothing rewrite the migration relies on — there is no
    /// observable half-written state.
    #[test]
    fn write_transcript_atomic_writes_a_complete_readable_file() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-atomic";

        let messages = vec![
            (user_msg("one"), 1_600_000_000_000_i64),
            (user_msg("two"), 1_600_000_001_000_i64),
            (user_msg("three"), 1_600_000_002_000_i64),
        ];
        write_transcript_atomic(base.path(), cwd.path(), id, TEST_CREATED_AT, &messages)
            .expect("atomic transcript write");

        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .expect("the atomically-written file is a valid transcript");
        assert_eq!(rows.len(), 3, "all three messages present");

        let activity = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("file exists");
        assert_eq!(activity.message_count, 3);
        assert_eq!(
            activity.last_message_at,
            Some(1_600_000_002_000),
            "activity key is the max stamped timestamp"
        );

        // The header carries created_at, not the wall clock.
        let info = build_session_info(&session_path(base.path(), cwd.path(), id))
            .unwrap()
            .unwrap();
        assert_eq!(info.timestamp, TEST_CREATED_AT);

        // Exactly one official file on disk, no stray .tmp leftover.
        let dir = session_dir(base.path(), cwd.path());
        let stray: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "no temp ghost must remain: {stray:?}");
    }

    /// VAL-CASESS-025 (rewrite leg): `write_transcript_atomic` OVERWRITES an
    /// existing (here: half-written / wrong-count) file with the complete one,
    /// rather than appending to it — so a re-run that detects an incomplete file
    /// converges to exactly one complete transcript.
    #[test]
    fn write_transcript_atomic_overwrites_a_half_written_file() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-rewrite";

        // Plant a half-written file: a valid header but only ONE message, as if a
        // prior migration was killed after the first append.
        let dir = session_dir(base.path(), cwd.path());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        let half = format!(
            "{}\n{}\n",
            header_line(id, cwd.path(), TEST_CREATED_AT).unwrap(),
            serde_json::to_string(&MessageEntryLine::Message(MessageEntryData {
                id: "x".into(),
                message: &user_msg("only the first"),
                timestamp: 1_600_000_000_000,
            }))
            .unwrap(),
        );
        std::fs::write(&path, half).unwrap();
        assert_eq!(
            load_transcript(base.path(), cwd.path(), id)
                .unwrap()
                .unwrap()
                .len(),
            1,
            "precondition: the planted file has one message"
        );

        // Rewrite with the COMPLETE three-message transcript.
        let messages = vec![
            (user_msg("a"), 1_600_000_000_000_i64),
            (user_msg("b"), 1_600_000_001_000_i64),
            (user_msg("c"), 1_600_000_002_000_i64),
        ];
        write_transcript_atomic(base.path(), cwd.path(), id, TEST_CREATED_AT, &messages)
            .expect("rewrite ok");

        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .unwrap();
        assert_eq!(
            rows.len(),
            3,
            "the half-written file must be replaced, not appended to"
        );
    }

    /// VAL-CASESS-020: when the session directory is read-only so the JSONL write
    /// genuinely cannot complete, `write_transcript_atomic` returns a structured
    /// `AppError` AND leaves NO file behind — neither a half-written official
    /// `<id>.jsonl` nor a stray `.tmp` ghost. The atomic temp-file + rename design
    /// means the failure happens on the temp file (which is then removed) before
    /// any rename onto the real path could occur.
    ///
    /// `#[cfg(unix)]` + a non-root guard: removing the write bit has no effect for
    /// root, so the test self-skips when it cannot actually make the dir
    /// read-only (CI may run as root).
    #[cfg(unix)]
    #[test]
    fn write_transcript_atomic_on_readonly_dir_errors_and_leaves_no_file() {
        use std::os::unix::fs::PermissionsExt;

        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-readonly";

        // Pre-create the session dir so the write target's PARENT exists, then
        // strip its write permission so creating the temp file inside it fails.
        let dir = session_dir(base.path(), cwd.path());
        std::fs::create_dir_all(&dir).unwrap();
        let mut perms = std::fs::metadata(&dir).unwrap().permissions();
        perms.set_mode(0o555); // r-xr-xr-x: readable + traversable, NOT writable
        std::fs::set_permissions(&dir, perms).unwrap();

        // Self-skip when not actually read-only (running as root).
        if std::fs::File::create(dir.join(".writable-probe")).is_ok() {
            let _ = std::fs::remove_file(dir.join(".writable-probe"));
            // Restore perms so the TempDir can be cleaned up, then skip.
            let mut restore = std::fs::metadata(&dir).unwrap().permissions();
            restore.set_mode(0o755);
            std::fs::set_permissions(&dir, restore).unwrap();
            eprintln!("skipping read-only test: directory is still writable (running as root?)");
            return;
        }

        let messages = vec![(user_msg("never lands"), 1_600_000_000_000_i64)];
        let result =
            write_transcript_atomic(base.path(), cwd.path(), id, TEST_CREATED_AT, &messages);

        // ① A structured AppError is returned (not a panic, not a silent Ok).
        let err = result.expect_err("a write into a read-only dir must fail");
        assert_eq!(err.code, "INTERNAL_ERROR");
        assert!(err.hint.is_some(), "AppError carries a hint");

        // ② No official file and no temp ghost were left behind. Restore write
        // permission first so we can read the directory listing.
        let mut restore = std::fs::metadata(&dir).unwrap().permissions();
        restore.set_mode(0o755);
        std::fs::set_permissions(&dir, restore).unwrap();

        let leftovers: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            !leftovers.iter().any(|n| n.ends_with(".jsonl")),
            "no half-written official .jsonl must remain: {leftovers:?}"
        );
        assert!(
            !leftovers.iter().any(|n| n.ends_with(".tmp")),
            "no .tmp ghost must remain: {leftovers:?}"
        );
    }
}
