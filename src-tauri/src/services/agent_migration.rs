//! agent_migration — one-shot SQLite→JSONL materialization of legacy agent
//! transcripts.
//!
//! M3 makes the coding-agent JSONL file the authoritative transcript store, but
//! every agent session created before M3 lives only in the SQLite
//! `agent_session_messages` table. This module replays each such transcript into
//! its `<id>.jsonl` once, so that the post-M3 read path (`agent_jsonl_store::
//! load_transcript` / `session_activity`) sees the full history — title,
//! activity time, project grouping, and per-message content all preserved.
//!
//! Why this is a pure replay with NO field remapping: the SQLite
//! `agent_session_messages.payload` column already holds a serialized
//! [`hand_ai_model::Message`] (the legacy runtime deserializes it verbatim at
//! `agent_runtime.rs:542`). The JSONL writer consumes the SAME
//! `hand_ai_model::Message` type (same `model` crate, same git tag, deduped by
//! Cargo). So migrating one message is exactly: deserialize the payload into a
//! `Message`, then write it back as a JSONL message entry. ToolCall / ToolResult
//! / thinking blocks are content blocks INSIDE the `Message`, so they ride
//! through the round-trip untouched (VAL-CASESS-014).
//!
//! Why NOT `SessionManager::append_message`: that helper stamps the entry-level
//! `timestamp` with `Utc::now()`, and `build_session_info` derives a session's
//! last-activity key (`SessionInfo.modified`) from the max entry timestamp. A
//! whole-library migration would therefore collapse every session's activity key
//! to the migration moment and DESTROY relative session ordering (VAL-CASESS-012).
//! So the migration writes through [`crate::services::agent_jsonl_store::
//! append_message_at`], which records the SQLite per-message `created_at` as the
//! entry timestamp — keeping each session's activity key equal to its
//! pre-migration `last_message_at`.
//!
//! Scope (this feature — happy path + fidelity only):
//!   - Per-session idempotent: a session whose `<id>.jsonl` already exists is
//!     skipped, so re-running the migration never doubles a transcript and a
//!     post-M3 native session (which has no SQLite messages) is a no-op.
//!   - An empty session (zero SQLite messages) builds NO JSONL file — it stays
//!     correctly anchored to its SQLite `created_at` via the sidebar's
//!     `lastMessageAt ?? createdAt` coalescing.
//!   - The session TITLE is never written as a JSONL label: the dual-source
//!     overlay keeps the SQLite `name` authoritative whenever no JSONL label is
//!     present, so writing one would only add an edge case (VAL-CASESS-012).
//!
//! Out of scope (left for `m3-migration-robustness`): corrupt-row skipping with
//! per-row counting, half-written / partial-transcript recovery, concurrency.
//! This module keeps a clean extension point (the per-session loop returns a
//! [`MigrationReport`] and a single bad payload aborts only its own session via
//! `tracing::warn` + skip, never panicking and never touching sibling sessions)
//! but does NOT implement those finer fault-tolerance assertions.

use std::path::Path;
use std::sync::Arc;

use hand_ai_model::Message;

use crate::models::AppError;
use crate::services::agent_jsonl_store::{append_message_at, session_cwd, session_path};
use crate::storage::types::AgentSession;
use crate::storage::{AgentSessionRepository, Database};

/// Page size for walking the full session list. `list_sessions` is paginated
/// (limit/offset); we drain every page so a large legacy library migrates in
/// full rather than only its first page.
const SESSION_PAGE_SIZE: i32 = 500;

/// Outcome of a migration pass. Counts let the caller log how much was
/// materialized vs. skipped without re-querying. (The robustness feature will
/// extend this with per-row corrupt-skip accounting.)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Sessions that had SQLite messages and were materialized into a fresh
    /// `<id>.jsonl` this pass.
    pub migrated_sessions: usize,
    /// Sessions skipped because their `<id>.jsonl` already existed (idempotent
    /// re-run, or a post-M3 native session).
    pub skipped_existing: usize,
    /// Sessions skipped because they had zero SQLite messages (no JSONL built —
    /// they stay anchored to their SQLite created_at).
    pub skipped_empty: usize,
    /// Sessions skipped because a payload could not be deserialized into a
    /// `Message` (logged via `tracing::warn`). The finer "skip the bad row, keep
    /// the rest" semantics belong to `m3-migration-robustness`; here a session
    /// with any bad payload is left wholly unmaterialized rather than partially
    /// written, so a re-run after a fix can still migrate it cleanly.
    pub skipped_undeserializable: usize,
    /// Total `Message` rows appended across all migrated sessions.
    pub messages_migrated: usize,
}

/// Replay every legacy SQLite agent transcript into its `<id>.jsonl` once.
///
/// `base_dir` is the Tauri app-data dir: it is simultaneously the JSONL base
/// directory AND the cwd fallback for a session with no `working_dir` — exactly
/// the role it plays for the writer (`coding_agent_session::config_from_rows` →
/// `session_cwd`). The cwd a session's JSONL is keyed by MUST be derived through
/// the same [`session_cwd`] the writer uses, or the post-M3 reader would look in
/// the wrong `<flattened-cwd>` subdir and report every migrated session as
/// transcript-less.
///
/// Per session:
///   1. List its SQLite messages (seq-ascending).
///   2. Zero messages → skip (no JSONL file; stays anchored to created_at).
///   3. `<id>.jsonl` already exists → skip (idempotent; also covers post-M3
///      native sessions, which have no SQLite messages anyway).
///   4. Otherwise seed the header (stamping the session's SQLite `created_at`)
///      and write each deserialized payload back as a JSONL message entry, in
///      seq order, stamping each entry timestamp from the row's `created_at`.
///
/// The session title is deliberately NOT written as a label (the SQLite `name`
/// stays the authoritative display name via the overlay). The legacy SQLite
/// tables are NOT dropped — migration is an additive materialization so it can
/// be re-run and rolled back; dropping the legacy tables is a later milestone.
pub async fn migrate_sqlite_sessions_to_jsonl(
    db: Arc<Database>,
    base_dir: &Path,
) -> Result<MigrationReport, AppError> {
    let repository = AgentSessionRepository::new(db);
    let mut report = MigrationReport::default();

    let mut offset = 0;
    loop {
        let page = repository.list_sessions(SESSION_PAGE_SIZE, offset).await?;
        let page_len = page.len();
        for session in &page {
            migrate_one_session(&repository, base_dir, session, &mut report).await?;
        }
        if page_len < SESSION_PAGE_SIZE as usize {
            break;
        }
        offset += SESSION_PAGE_SIZE;
    }

    Ok(report)
}

/// Materialize a single session's transcript, updating `report` in place. Pulled
/// out of the page loop so the (future) robustness feature can wrap per-session
/// fault handling around one well-defined unit.
async fn migrate_one_session(
    repository: &AgentSessionRepository,
    base_dir: &Path,
    session: &AgentSession,
    report: &mut MigrationReport,
) -> Result<(), AppError> {
    let messages = repository.list_messages(&session.id).await?;

    // (2) Empty session → no JSONL file; the sidebar coalesces to created_at.
    if messages.is_empty() {
        report.skipped_empty += 1;
        return Ok(());
    }

    let cwd = session_cwd(session.working_dir.as_deref(), base_dir);

    // (3) Idempotent: an already-materialized session is left untouched so a
    // re-run never doubles its transcript.
    if session_path(base_dir, &cwd, &session.id).exists() {
        report.skipped_existing += 1;
        return Ok(());
    }

    // Deserialize the whole transcript BEFORE writing anything, so a single bad
    // payload leaves the session wholly unmaterialized (no half-written file)
    // rather than a truncated JSONL. The robustness feature will refine this to
    // per-row skipping with counting. Each message carries the SQLite row's
    // `created_at` — the entry-level timestamp the JSONL line must record so the
    // migrated session's last-activity key equals its pre-migration value
    // (VAL-CASESS-012); see `agent_jsonl_store::append_message_at`.
    let mut decoded: Vec<(Message, i64)> = Vec::with_capacity(messages.len());
    for row in &messages {
        match serde_json::from_value::<Message>(row.payload.clone()) {
            Ok(message) => decoded.push((message, row.created_at)),
            Err(e) => {
                tracing::warn!(
                    session_id = %session.id,
                    seq = row.seq,
                    error = %e,
                    "skipping migration of agent session: transcript payload is not a valid \
                     hand-agent Message"
                );
                report.skipped_undeserializable += 1;
                return Ok(());
            }
        }
    }

    // (4) Seed the header (stamping the SQLite created_at, NOT now) then append
    // each message as a JSONL line whose entry timestamp is the SQLite row's
    // `created_at`. The first append ensures the file; subsequent ones append in
    // seq order, so the on-disk transcript matches the source order exactly.
    for (message, message_created_at) in decoded {
        append_message_at(
            base_dir,
            &cwd,
            &session.id,
            session.created_at,
            &message,
            message_created_at,
        )?;
    }

    report.migrated_sessions += 1;
    report.messages_migrated += messages.len();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_jsonl_store::{load_transcript, session_activity};
    use crate::storage::types::{AgentSession, Timestamp};
    use hand_ai_model::{
        Api, AssistantContentBlock, AssistantMessage, StopReason, TextContent, ToolCall, Usage,
        UserMessage,
    };
    use sqlx::Row;
    use tempfile::TempDir;

    async fn test_db() -> (Arc<Database>, TempDir) {
        let temp = TempDir::new().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let db = Arc::new(Database::new(&db_path).await.expect("db"));
        (db, temp)
    }

    fn session_row(
        id: &str,
        name: &str,
        working_dir: Option<&str>,
        created_at: i64,
    ) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            name: name.to_string(),
            project_id: None,
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: None,
            thinking_level: None,
            temperature: None,
            max_tokens: None,
            working_dir: working_dir.map(str::to_string),
            enabled_tools: Vec::new(),
            tool_execution_mode: None,
            message_count: 0,
            last_message_at: None,
            created_at,
            updated_at: created_at,
        }
    }

    fn user_payload(text: &str) -> serde_json::Value {
        serde_json::to_value(Message::User(UserMessage::new_text(text.to_string()))).unwrap()
    }

    /// An assistant Message carrying a text block, a thinking block, AND a tool
    /// call — the content-fidelity fixture for VAL-CASESS-014.
    fn assistant_with_tool_and_thinking(text: &str, timestamp: u64) -> Message {
        Message::Assistant(AssistantMessage {
            role: "assistant".into(),
            content: vec![
                AssistantContentBlock::Thinking(hand_ai_model::ThinkingContent::new(
                    "let me think",
                )),
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
            timestamp,
            response_model: None,
            response_id: None,
            diagnostics: None,
        })
    }

    /// Count the SQLite transcript rows for a session — the migration parity
    /// baseline (VAL-CASESS-013).
    async fn sqlite_message_count(db: &Database, session_id: &str) -> i64 {
        sqlx::query("SELECT COUNT(*) AS c FROM agent_session_messages WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(db.pool())
            .await
            .unwrap()
            .try_get::<i64, _>("c")
            .unwrap()
    }

    /// VAL-CASESS-013: a migrated session's JSONL message count equals its
    /// original SQLite `count(*)` — every message replayed, none lost.
    #[tokio::test]
    async fn migration_preserves_message_count_per_session() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let session = session_row("sess-count", "Counting", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&session).await.unwrap();
        for i in 0..7 {
            repo.append_message(
                &session.id,
                "user",
                &user_payload(&format!("m{i}")),
                1_700_000_000_000 + i,
            )
            .await
            .unwrap();
        }

        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(report.migrated_sessions, 1);
        assert_eq!(report.messages_migrated, 7);

        let sqlite_count = sqlite_message_count(&db, &session.id).await;
        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .expect("a migrated session has a JSONL transcript");
        assert_eq!(
            rows.len() as i64,
            sqlite_count,
            "JSONL message rows must equal SQLite count(*)"
        );
    }

    /// VAL-CASESS-014: a session whose transcript carries tool calls and a
    /// thinking block migrates with those content blocks intact — `load_transcript`
    /// restores them (not a text-only flattening), and the per-message
    /// content-block count matches the original.
    #[tokio::test]
    async fn migration_preserves_tool_calls_and_thinking_blocks() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let session = session_row(
            "sess-fidelity",
            "Fidelity",
            Some(&cwd_str),
            1_700_000_000_000,
        );
        repo.create_session(&session).await.unwrap();
        repo.append_message(
            &session.id,
            "user",
            &user_payload("read x.txt"),
            1_700_000_000_001,
        )
        .await
        .unwrap();
        let assistant = assistant_with_tool_and_thinking("on it", 1_700_000_000_002);
        let assistant_payload = serde_json::to_value(&assistant).unwrap();
        let original_block_count = assistant_payload
            .get("content")
            .and_then(|c| c.as_array())
            .map(|a| a.len())
            .unwrap();
        assert_eq!(original_block_count, 3, "fixture has 3 content blocks");
        repo.append_message(
            &session.id,
            "assistant",
            &assistant_payload,
            1_700_000_000_002,
        )
        .await
        .unwrap();

        migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .unwrap();
        assert_eq!(rows.len(), 2);

        let blocks = rows[1].payload.get("content").unwrap().as_array().unwrap();
        assert_eq!(
            blocks.len(),
            original_block_count,
            "migrated assistant message must keep all content blocks"
        );
        let types: Vec<&str> = blocks
            .iter()
            .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
            .collect();
        assert!(
            types.contains(&"toolcall"),
            "tool call block survives: {types:?}"
        );
        assert!(
            types.contains(&"thinking"),
            "thinking block survives: {types:?}"
        );
        assert!(types.contains(&"text"), "text block survives: {types:?}");
    }

    /// VAL-CASESS-012 (ordering leg): after migration the relative activity
    /// ordering across sessions equals the pre-migration ordering. The
    /// pre-migration key is `coalesce(last_message_at, created_at)`; the
    /// post-migration key is the max JSONL entry timestamp, which the migration
    /// stamps from each message's SQLite `created_at`. Seed three sessions whose
    /// latest-message `created_at` strictly increase and assert the
    /// post-migration `session_activity` sort matches.
    #[tokio::test]
    async fn migration_preserves_relative_session_order() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        // Three sessions with distinct, strictly-increasing latest-message times.
        // Created in a deliberately scrambled order so the assertion can't pass
        // by accident of insertion order. The `created_at` passed to
        // append_message is both the SQLite last_message_at AND the JSONL entry
        // timestamp the migration replays — the cross-source activity key.
        let specs = [
            ("sess-mid", "Mid", 2_000_000_000_000_i64),
            ("sess-old", "Old", 1_000_000_000_000_i64),
            ("sess-new", "New", 3_000_000_000_000_i64),
        ];
        for (id, name, last_ts) in specs {
            let session = session_row(id, name, Some(&cwd_str), last_ts - 100);
            repo.create_session(&session).await.unwrap();
            repo.append_message(
                &id.to_string(),
                "user",
                &user_payload("only message"),
                last_ts,
            )
            .await
            .unwrap();
        }

        // Pre-migration expected order (descending activity): new, mid, old.
        let expected_desc = ["sess-new", "sess-mid", "sess-old"];

        migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let mut activities: Vec<(&str, Timestamp)> = expected_desc
            .iter()
            .map(|id| {
                let act = session_activity(base.path(), &jsonl_cwd, id)
                    .unwrap()
                    .expect("migrated session has activity");
                (
                    *id,
                    act.last_message_at
                        .expect("a migrated non-empty session reports a real activity time"),
                )
            })
            .collect();
        // Sort by activity descending — must reproduce expected_desc exactly.
        activities.sort_by(|a, b| b.1.cmp(&a.1));
        let sorted_ids: Vec<&str> = activities.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            sorted_ids,
            expected_desc.to_vec(),
            "post-migration activity ordering must match pre-migration ordering"
        );
    }

    /// VAL-CASESS-012 (title leg) + empty-session leg: a migrated session writes
    /// NO JSONL label (so the overlay keeps the SQLite name authoritative), and a
    /// session with zero SQLite messages builds no JSONL file at all.
    #[tokio::test]
    async fn migration_writes_no_label_and_skips_empty_sessions() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        // One session with a message, one empty.
        let with_msg = session_row("sess-titled", "My Title", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&with_msg).await.unwrap();
        repo.append_message(&with_msg.id, "user", &user_payload("hi"), 1_700_000_000_001)
            .await
            .unwrap();

        let empty = session_row("sess-empty", "Empty", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&empty).await.unwrap();

        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(report.migrated_sessions, 1);
        assert_eq!(report.skipped_empty, 1);

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());

        // The migrated session has no JSONL label → overlay keeps SQLite name.
        let act = session_activity(base.path(), &jsonl_cwd, &with_msg.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            act.name, None,
            "migration must not write a JSONL label; SQLite name stays authoritative"
        );

        // The empty session built no JSONL file.
        assert!(
            session_activity(base.path(), &jsonl_cwd, &empty.id)
                .unwrap()
                .is_none(),
            "an empty session must not be materialized into a JSONL file"
        );
    }

    /// Idempotency: running the migration twice over the same DB does not double
    /// the JSONL transcript and does not rebuild the file — the second pass skips
    /// every already-materialized session.
    #[tokio::test]
    async fn migration_is_idempotent_on_rerun() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let session = session_row("sess-idem", "Idem", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&session).await.unwrap();
        for i in 0..3 {
            repo.append_message(
                &session.id,
                "user",
                &user_payload(&format!("m{i}")),
                1_700_000_000_000 + i,
            )
            .await
            .unwrap();
        }

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let path = session_path(base.path(), &jsonl_cwd, &session.id);

        let first = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(first.migrated_sessions, 1);
        assert_eq!(first.messages_migrated, 3);
        let mtime_after_first = std::fs::metadata(&path).unwrap().modified().unwrap();
        let rows_after_first = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .unwrap()
            .len();
        assert_eq!(rows_after_first, 3);

        let second = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(
            second.migrated_sessions, 0,
            "second pass must materialize nothing"
        );
        assert_eq!(
            second.skipped_existing, 1,
            "second pass must skip the already-materialized session"
        );

        let rows_after_second = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .unwrap()
            .len();
        assert_eq!(
            rows_after_second, 3,
            "re-running migration must not double the transcript"
        );
        let mtime_after_second = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            mtime_after_first, mtime_after_second,
            "the JSONL file must not be rewritten on a second pass"
        );
    }

    /// A session with no `working_dir` (rooted at the app-data dir) migrates and
    /// reads back: the migration's cwd derivation matches the writer's
    /// `session_cwd(None, base_dir)` fallback, so the reader finds the file.
    #[tokio::test]
    async fn migration_handles_session_with_no_working_dir() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());

        let session = session_row("sess-nodir", "No Dir", None, 1_700_000_000_000);
        repo.create_session(&session).await.unwrap();
        repo.append_message(
            &session.id,
            "user",
            &user_payload("hello"),
            1_700_000_000_001,
        )
        .await
        .unwrap();

        migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();

        let jsonl_cwd = session_cwd(None, base.path());
        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .expect("a no-working-dir session migrates under the app-data-dir cwd");
        assert_eq!(rows.len(), 1);
    }

    /// VAL-CASESS-024: a JSONL session's project group name is derived from its
    /// (canonicalized) cwd via the SAME `default_project_name` algorithm the
    /// SQLite `agent_projects.name` uses — and a trailing-slash variant of the
    /// same directory canonicalizes to the identical basename. Root `/` (empty
    /// basename) falls back to the full path.
    #[test]
    fn project_basename_is_cross_source_consistent_after_canonicalize() {
        use crate::services::agent_project::default_project_name;

        let temp = TempDir::new().unwrap();
        let proj = temp.path().join("proj");
        std::fs::create_dir_all(&proj).unwrap();

        // canonicalize resolves symlinks (e.g. macOS /var → /private/var) and
        // normalizes the path; the basename must survive a trailing slash.
        let canon_plain = std::fs::canonicalize(&proj).unwrap();
        let with_trailing = format!("{}/", proj.to_string_lossy());
        let canon_trailing = std::fs::canonicalize(&with_trailing).unwrap();
        assert_eq!(
            canon_plain, canon_trailing,
            "trailing slash must canonicalize to the same path"
        );

        let name_plain = default_project_name(&canon_plain.to_string_lossy());
        let name_trailing = default_project_name(&canon_trailing.to_string_lossy());
        assert_eq!(
            name_plain, name_trailing,
            "both cwd spellings must derive the same project group name"
        );
        assert_eq!(name_plain, "proj", "basename of the canonical path");

        // Root path: empty basename falls back to the full path.
        assert_eq!(
            default_project_name("/"),
            "/",
            "root path (empty basename) falls back to the full path"
        );
    }
}
