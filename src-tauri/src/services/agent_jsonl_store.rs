//! Read/locate the coding-agent JSONL session backing a HandBox agent session.
//! JSONL is the authoritative transcript store, at
//! `<base>/sessions/<flattened-cwd>/<id>.jsonl` under the Tauri app-data dir.
//! Pre-seeding the header lets a HandBox session UUID double as the JSONL
//! session id, so multi-turn appends need no id map. Parsing goes through the
//! upstream `SessionManager`, but reads scan `<base>/sessions/` directly since
//! [`SessionManager::list_all`] only knows the home-based layout.

use std::path::{Path, PathBuf};

use hand_ai_model::Message;
use hand_coding_agent::core::session_manager::{
    build_session_info, SessionHeader, CURRENT_SESSION_VERSION,
};
use hand_coding_agent::SessionManager;

use crate::models::AppError;
use crate::storage::types::{AgentSessionMessage, Timestamp, UUID};

/// Activity metadata for a JSONL-backed session, lifted from its `SessionInfo`.
/// The SQLite row stays the source of truth for *config*; JSONL is the source
/// of truth for *activity* (message count, last activity, latest label).
#[derive(Debug, Clone, PartialEq)]
pub struct JsonlActivity {
    /// Number of `Message` entries in the JSONL (excludes header/labels/etc.).
    pub message_count: i32,
    /// Latest message timestamp (millis). `None` when there are no message
    /// entries yet — the sidebar's `lastMessageAt ?? createdAt` coalescing
    /// depends on a genuine `null` rather than 0 (see `agentGrouping.ts`).
    pub last_message_at: Option<Timestamp>,
    /// Latest non-empty session label, if the agent renamed the session.
    pub name: Option<String>,
}

/// Directory the JSONL session for `cwd` lives in under `base_dir`
/// (`<base>/sessions/<flattened-cwd>/`). Mirrors the writer side exactly so the
/// reader and writer never disagree about where a file is.
pub fn session_dir(base_dir: &Path, cwd: &Path) -> PathBuf {
    SessionManager::default_session_dir_with_base(Some(base_dir), cwd)
}

/// Resolve the `cwd` a session's JSONL is keyed by, given its (optional)
/// stored `working_dir` and the app data dir.
///
/// MUST match the cwd the writer used (`coding_agent_session::config_from_rows`);
/// diverging would silently look in the wrong `<flattened-cwd>` directory and
/// report every session as transcript-less.
pub fn session_cwd(working_dir: Option<&str>, app_data_dir: &Path) -> PathBuf {
    // Mirror the writer: pass working_dir through verbatim, falling back to
    // app_data_dir only when absent. `validate_working_dir` never stores an
    // empty string, so no empty-string special case is needed.
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

/// Ensure `<session_id>.jsonl` exists under `base_dir`/`cwd`, seeding a minimal
/// `SessionHeader` whose `id == session_id` when absent. Idempotent: an existing
/// file is left untouched, and the header write is atomic so a crash leaves no
/// header-less ghost.
///
/// Seeding is necessary because `SessionManager::create_in` mints its own id;
/// HandBox instead drives the session via `resume_session = <uuid>`.
///
/// `created_at` (millis) MUST be the session's SQLite `created_at`, not the wall
/// clock — the sidebar coalesces `lastMessageAt ?? createdAt`, so the header
/// must carry the real creation time, not the first-run time.
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

/// Serialize the one-line `{"type":"session","data":<SessionHeader>}` header,
/// built via the same serde path SessionManager uses.
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

/// Write `content` to `path` atomically via a sibling temp file + `rename`, so a
/// reader (or a crash) only ever sees the old or the new complete file; the temp
/// file is removed on any failure. The temp file must be a sibling, not in
/// `/tmp`, since a cross-device rename is not atomic.
fn write_file_atomic(path: &Path, content: &str) -> Result<(), AppError> {
    use std::io::Write;

    let dir = path.parent().ok_or_else(|| {
        AppError::internal_error(&format!(
            "session jsonl path has no parent directory: {}",
            path.display()
        ))
    })?;
    // Unique name so concurrent writers of different sessions never collide.
    let tmp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));

    // Scoped so the handle closes before the rename.
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

/// Write a session's complete transcript (header + every message) to its
/// `<id>.jsonl` in one atomic step, overwriting any existing file. Rebuilding
/// the whole file from a known-good message set makes the migration
/// crash-convergent: a half-written file becomes a complete one in one rename.
///
/// Each entry stamps the message's own historical `timestamp`, not the wall
/// clock, so cross-session activity ordering survives.
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
/// `SessionEntry::Session` on-disk shape. `SessionEntry` itself is not
/// re-exported, so we mirror only the header-line shape we need to write;
/// reading always goes through the upstream parser.
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

/// The `data` payload of a `{"type":"message","data":{..}}` JSONL line. Written
/// directly rather than via [`SessionManager::append_message`] on the migration
/// path only, because that helper stamps `Utc::now()`, which would collapse
/// every replayed message's activity key to migration time.
#[derive(serde::Serialize)]
struct MessageEntryData<'a> {
    id: String,
    message: &'a Message,
    timestamp: i64,
}

/// A `{"type":"message","data":<MessageEntryData>}` envelope, matching the
/// `SessionEntry::Message` on-disk shape. `parent_id` is omitted: a migrated
/// transcript is a flat list with no parentage.
#[derive(serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum MessageEntryLine<'a> {
    Message(MessageEntryData<'a>),
}

/// Append one `Message` with an EXPLICIT entry-level `timestamp`, ensuring the
/// file first. Migration-only writer (see [`MessageEntryData`]): the timestamp
/// must be the message's real send time so session ordering is preserved.
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

/// Load a session's transcript as frontend-shaped [`AgentSessionMessage`] rows,
/// or `None` when no JSONL file exists (the caller then falls back to the SQLite
/// transcript). Messages come from [`SessionManager::build_context`], the
/// post-compaction context the agent itself sees, so the UI renders what the
/// model was fed; each `Message` is serialized into `payload` verbatim.
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

/// Append a session label (display name); the most recent label wins on
/// read-back. A rename must come through here, not only through the SQLite
/// `name`, or a stale JSONL label would visually override it.
///
/// The file is ensured first so renaming a never-resumed session takes effect. A
/// `Label` is not a `Message`, so a rename never manufactures activity.
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

/// Remove the JSONL file backing `session_id`, so no orphan `<id>.jsonl` is left
/// behind. A session with no file is a clean no-op — absence is the desired
/// post-state — and callers treat failure as best-effort, since the SQLite row
/// delete is what actually removes the session.
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

    // `SessionInfo.modified` falls back to the file mtime; only a genuine
    // message timestamp counts as activity, so a zero-message session reports
    // `None` (the sidebar then coalesces to createdAt).
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

/// Convert a context `Vec<Message>` into frontend-shaped rows: `role` is the
/// serialized `Message` tag, `seq` the 0-based context index, `created_at` the
/// message's own timestamp.
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
            // The JSONL transcript has no per-message UUID HandBox owns; a
            // deterministic (session_id, seq) id keeps rows distinct without
            // churning keyed list diffs on every read.
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

/// The message's own `timestamp` (millis), used as the row's `created_at`. The
/// model crate stores `u64` while `Timestamp` is `i64`, so the astronomically
/// distant overflow saturates rather than panics.
fn message_timestamp(message: &Message) -> Timestamp {
    let ts: u64 = match message {
        Message::User(m) => m.timestamp,
        Message::Assistant(m) => m.timestamp,
        Message::ToolResult(m) => m.timestamp,
    };
    Timestamp::try_from(ts).unwrap_or(Timestamp::MAX)
}

/// Alias for the SQLite session UUID this module treats as the JSONL session id.
#[allow(dead_code)]
pub type JsonlSessionId = UUID;

#[cfg(test)]
mod tests {
    use super::*;
    use hand_ai_model::{
        Api, AssistantContentBlock, AssistantMessage, StopReason, TextContent, ToolCall,
        ToolResultMessage, Usage, UserMessage,
    };
    use tempfile::TempDir;

    /// A fixed, obviously-not-now `created_at` (millis), so header timestamps
    /// can be asserted without racing the wall clock.
    const TEST_CREATED_AT: i64 = 1_700_000_000_000;

    /// Seed a header then open it through the real `SessionManager` resume path,
    /// so tests exercise the actual writer rather than a hand-rolled file.
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

    /// An assistant message carrying a single tool call — the shape the model
    /// emits when it wants to run a dangerous tool such as `bash`.
    fn assistant_with_tool_call(
        call_id: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![AssistantContentBlock::ToolCall(ToolCall {
                content_type: "toolCall".into(),
                id: call_id.into(),
                name: tool.into(),
                arguments,
                thought_signature: None,
            })],
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

    /// A success tool result: what an approved dangerous execution appends.
    fn tool_result_success(call_id: &str, tool: &str, output: &str) -> Message {
        Message::ToolResult(ToolResultMessage::new(
            call_id,
            tool,
            vec![hand_ai_model::ToolResultContent::Text(TextContent::new(
                output.to_string(),
            ))],
        ))
    }

    /// An is_error tool result carrying the deny reason: what a denied or
    /// aborted dangerous call appends instead of a success result.
    fn tool_result_error(call_id: &str, tool: &str, reason: &str) -> Message {
        Message::ToolResult(ToolResultMessage::new_error(call_id, tool, reason))
    }

    /// The `isError` flag distinguishing a success result from a denied one.
    fn row_is_error(row: &AgentSessionMessage) -> Option<bool> {
        row.payload.get("isError").and_then(|v| v.as_bool())
    }

    /// A freshly-seeded session lands a real `<id>.jsonl` whose file name AND
    /// header id equal the HandBox session id — id reuse with no mapping.
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

    /// The header carries the caller's `created_at`, not the wall clock, so the
    /// sidebar's `lastMessageAt ?? createdAt` keeps an empty session anchored to
    /// its creation time.
    #[test]
    fn ensure_session_file_stamps_header_timestamp_from_created_at_not_now() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-created-at";

        let path = ensure_session_file(base.path(), cwd.path(), id, TEST_CREATED_AT)
            .expect("seeds header");

        // Read it back through the upstream parser — the same value the activity
        // overlay surfaces as createdAt.
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

    /// A round-trip through the real writer + reader restores the transcript as
    /// frontend-shaped rows, tool calls included, in order.
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

        // The tool call rides inside the assistant payload's content blocks.
        let blocks = rows[1].payload.get("content").unwrap().as_array().unwrap();
        assert!(
            blocks
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("toolcall")),
            "assistant transcript row must carry the tool call content block, got: {:?}",
            rows[1].payload
        );
    }

    // Approval audit trail: each outcome must restore in a distinguishable
    // shape — allow as a success ToolResult, deny as an is_error one carrying
    // the reason, abort as no success result at all.

    /// An approved execution restores with both its tool call and its success
    /// result after a fresh read off disk.
    #[test]
    fn load_transcript_restores_allowed_execution_as_success_result() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-allow-leg";

        // Exactly what a turn that ran an approved `bash` would write.
        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id);
            mgr.append_message(user_msg("run `echo hi`")).unwrap();
            mgr.append_message(assistant_with_tool_call(
                "call-bash-1",
                "bash",
                serde_json::json!({ "command": "echo hi" }),
            ))
            .unwrap();
            mgr.append_message(tool_result_success("call-bash-1", "bash", "hi\n"))
                .unwrap();
        }

        // A fresh read off the file the previous process left behind.
        let rows = load_transcript(base.path(), cwd.path(), id)
            .expect("read ok")
            .expect("an executed session has a transcript on disk");

        assert_eq!(rows.len(), 3, "user + assistant tool call + tool result");

        let assistant = &rows[1];
        assert_eq!(assistant.role, "assistant");
        let blocks = assistant
            .payload
            .get("content")
            .unwrap()
            .as_array()
            .unwrap();
        assert!(
            blocks.iter().any(|b| {
                b.get("type").and_then(|t| t.as_str()) == Some("toolcall")
                    && b.get("name").and_then(|n| n.as_str()) == Some("bash")
            }),
            "the restored assistant turn must show the executed bash tool call, got: {:?}",
            assistant.payload
        );

        let result = &rows[2];
        assert_eq!(result.role, "toolResult");
        assert_eq!(
            row_is_error(result),
            Some(false),
            "an approved execution's result must restore as a SUCCESS (isError=false), got: {:?}",
            result.payload
        );
    }

    /// A denied call restores as an is_error result whose deny reason survives,
    /// so an auditor sees what the model tried to run and that it was refused.
    #[test]
    fn load_transcript_restores_denied_call_as_auditable_error_not_success() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-deny-leg";

        let deny_reason = "用户拒绝了 bash（denied）";
        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id);
            mgr.append_message(user_msg("rm everything")).unwrap();
            mgr.append_message(assistant_with_tool_call(
                "call-bash-2",
                "bash",
                serde_json::json!({ "command": "rm -rf /" }),
            ))
            .unwrap();
            // A deny appends an is_error result carrying the reason, never a
            // success result.
            mgr.append_message(tool_result_error("call-bash-2", "bash", deny_reason))
                .unwrap();
        }

        let rows = load_transcript(base.path(), cwd.path(), id)
            .expect("read ok")
            .expect("a denied session still has a transcript on disk");

        assert_eq!(
            rows.len(),
            3,
            "the attempt is on the record: user + call + denied result"
        );

        let blocks = rows[1].payload.get("content").unwrap().as_array().unwrap();
        assert!(
            blocks.iter().any(|b| {
                b.get("type").and_then(|t| t.as_str()) == Some("toolcall")
                    && b.get("name").and_then(|n| n.as_str()) == Some("bash")
            }),
            "a denied call's attempt must remain visible (the model tried to run bash)"
        );

        let result = &rows[2];
        assert_eq!(result.role, "toolResult");
        assert_eq!(
            row_is_error(result),
            Some(true),
            "a denied call's result must restore as an ERROR (isError=true), not a success, got: {:?}",
            result.payload
        );

        let payload_str = serde_json::to_string(&result.payload).unwrap();
        assert!(
            payload_str.contains(deny_reason),
            "the deny reason must survive into the restored result, got: {payload_str}"
        );
    }

    /// An aborted pending approval leaves no success ToolResult — at most a tool
    /// call plus an is_error refusal — so it never reads back as executed.
    #[test]
    fn load_transcript_aborted_pending_leaves_no_success_result() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();

        // Shape A: torn down before any ToolResult was appended.
        let id_no_result = "sess-abort-no-result";
        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id_no_result);
            mgr.append_message(user_msg("write a file")).unwrap();
            mgr.append_message(assistant_with_tool_call(
                "call-write-1",
                "write",
                serde_json::json!({ "path": "x.txt", "content": "data" }),
            ))
            .unwrap();
            // No ToolResult: aborted while parked on the approval await.
        }

        let rows = load_transcript(base.path(), cwd.path(), id_no_result)
            .expect("read ok")
            .expect("the session has a transcript");
        assert_eq!(
            rows.len(),
            2,
            "only user + the un-executed assistant tool call"
        );
        assert!(
            !rows.iter().any(|r| r.role == "toolResult"),
            "an aborted-before-result turn must leave NO tool result at all, got: {rows:?}"
        );
        assert!(
            rows.iter().all(|r| row_is_error(r) != Some(false)),
            "an aborted turn must produce no SUCCESS tool result (isError=false)"
        );

        // Shape B: the fail-closed deny landed before teardown.
        let id_failclosed = "sess-abort-failclosed";
        {
            let mut mgr = open_resumed(base.path(), cwd.path(), id_failclosed);
            mgr.append_message(user_msg("write a file")).unwrap();
            mgr.append_message(assistant_with_tool_call(
                "call-write-2",
                "write",
                serde_json::json!({ "path": "y.txt", "content": "data" }),
            ))
            .unwrap();
            mgr.append_message(tool_result_error(
                "call-write-2",
                "write",
                "用户拒绝了 write（denied）",
            ))
            .unwrap();
        }

        let rows = load_transcript(base.path(), cwd.path(), id_failclosed)
            .expect("read ok")
            .expect("the session has a transcript");
        let success_results = rows
            .iter()
            .filter(|r| r.role == "toolResult" && row_is_error(r) == Some(false))
            .count();
        assert_eq!(
            success_results, 0,
            "a fail-closed aborted call must leave NO success tool result — only an is_error \
             refusal, which never reads back as 已执行成功, got: {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|r| r.role == "toolResult" && row_is_error(r) == Some(true)),
            "the fail-closed refusal result must restore as an is_error record"
        );
    }

    /// A session with no JSONL file reads back as `None` on both seams, so the
    /// caller cleanly falls back to SQLite.
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

    /// `session_activity` reports the message count, a real last-activity
    /// timestamp and the latest label; a messageless session reports `None` so
    /// the sidebar coalesces to createdAt rather than showing the epoch.
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

    /// `append_message_at` stamps the timestamp it is given, and that becomes
    /// the activity key the reader surfaces — otherwise every replayed message
    /// would collapse to the same "now" and lose relative order.
    #[test]
    fn append_message_at_stamps_given_timestamp_as_activity_key() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-append-at";
        // Clearly historical, unmistakably not a "now" value.
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

        let activity = session_activity(base.path(), cwd.path(), id)
            .unwrap()
            .expect("file exists");
        assert_eq!(activity.message_count, 1);
        assert_eq!(
            activity.last_message_at,
            Some(entry_ts),
            "the entry timestamp must equal the value passed, not the wall clock"
        );

        // The hand-written line is a valid entry to the upstream reader.
        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .expect("a transcript");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].role, "user");

        // A second append lands in the same file and advances the activity key.
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

    /// A rename's label becomes the display name `session_activity` reads back,
    /// overriding an older agent-assigned one: the newest label wins.
    #[test]
    fn append_label_makes_new_name_authoritative_over_an_older_label() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-rename";

        // The agent auto-titled the session earlier.
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

    /// Renaming a never-resumed session seeds the file and sets the name while
    /// leaving activity untouched (a label is not a message), and the seeded
    /// header carries `created_at` rather than the rename moment.
    #[test]
    fn append_label_on_empty_session_sets_name_without_creating_activity() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-empty-rename";

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

    /// Deleting a session removes its JSONL file so no orphan transcript is
    /// left on disk.
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

    /// Deleting a session with no JSONL file is a clean no-op, not an error.
    #[test]
    fn delete_session_file_absent_is_noop() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        delete_session_file(base.path(), cwd.path(), "never-created")
            .expect("deleting a session with no JSONL file must be a clean no-op");
    }

    /// A garbage line between valid messages is silently skipped by the upstream
    /// parser: activity reports the surviving count and the max surviving
    /// timestamp, never counting the bad line as 0 or as newest.
    #[test]
    fn malformed_jsonl_line_is_skipped_on_read_without_polluting_activity() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-garbage-line";

        // Known timestamps, so the activity key can be asserted to equal exactly
        // the max of the two valid ones.
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

        // Pure garbage directly after the first message.
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

        let rows = load_transcript(base.path(), cwd.path(), id)
            .unwrap()
            .expect("a seeded session has a transcript");
        assert_eq!(
            rows.len(),
            2,
            "malformed line must be skipped, leaving exactly the two valid messages"
        );
        assert!(rows.iter().all(|r| r.role == "user"));

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

    /// A file whose first line is not a valid header reports no activity, so the
    /// overlay keeps the SQLite values instead of rendering a blank row.
    #[test]
    fn bad_header_jsonl_reports_no_activity_not_a_blank_row() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-bad-header";

        // First line junk, followed by an otherwise valid-looking line.
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

    /// `write_transcript_atomic` writes one complete file that reads back as
    /// exactly the messages written, in order, with the activity key equal to
    /// the max stamped timestamp.
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

        let dir = session_dir(base.path(), cwd.path());
        let stray: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "no temp ghost must remain: {stray:?}");
    }

    /// It OVERWRITES an existing half-written file rather than appending, so a
    /// re-run converges to exactly one complete transcript.
    #[test]
    fn write_transcript_atomic_overwrites_a_half_written_file() {
        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-rewrite";

        // A valid header but only one message, as if a prior run was killed
        // after the first append.
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

    /// A read-only session directory yields a structured `AppError` and leaves
    /// nothing behind — no half-written `<id>.jsonl`, no `.tmp` ghost. Unix-only
    /// and self-skipping, since removing the write bit has no effect for root.
    #[cfg(unix)]
    #[test]
    fn write_transcript_atomic_on_readonly_dir_errors_and_leaves_no_file() {
        use std::os::unix::fs::PermissionsExt;

        let base = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let id = "sess-readonly";

        // Pre-create the parent, then strip its write permission so creating the
        // temp file inside it fails.
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

        let err = result.expect_err("a write into a read-only dir must fail");
        assert_eq!(err.code, "INTERNAL_ERROR");
        assert!(err.hint.is_some(), "AppError carries a hint");

        // Restore write permission so the directory listing can be read.
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
