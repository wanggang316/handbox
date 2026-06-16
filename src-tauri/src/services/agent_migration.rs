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
//! Robustness (this feature — `m3-migration-robustness`):
//!   - Per-row corrupt-skip (VAL-CASESS-015): a single transcript row whose
//!     payload is not a valid `Message` is dropped (logged + counted) while the
//!     session's OTHER rows still migrate. A session is only left wholly
//!     unmaterialized when it has NO migratable row at all.
//!   - Completeness-aware idempotency (VAL-CASESS-025): the "already migrated?"
//!     test is no longer "does `<id>.jsonl` exist" but "does a COMPLETE
//!     `<id>.jsonl` exist" — a file that is header-less (corrupt) or whose
//!     message count disagrees with the (good) SQLite rows is REWRITTEN, so a
//!     half-written / crashed-mid-migration file converges to one complete copy.
//!   - Atomic full-file write: each session's transcript is rebuilt and
//!     committed via [`write_transcript_atomic`] (temp file + rename), so a
//!     crash leaves either the prior complete file or no leftover — never a
//!     truncated official `<id>.jsonl` (VAL-CASESS-020 / VAL-CASESS-025).
//!   - Sibling isolation: a per-session fatal error (e.g. a transient IO error
//!     listing or writing one session) is logged + counted and the loop moves
//!     on, so one bad session never aborts the whole migration.
//!
//! Unchanged design choices carried from `m3-migration-core`:
//!   - An empty session (zero SQLite messages) builds NO JSONL file — it stays
//!     correctly anchored to its SQLite `created_at` via the sidebar's
//!     `lastMessageAt ?? createdAt` coalescing.
//!   - The session TITLE is never written as a JSONL label: the dual-source
//!     overlay keeps the SQLite `name` authoritative whenever no JSONL label is
//!     present, so writing one would only add an edge case (VAL-CASESS-012).
//!   - The per-message entry timestamp is the SQLite row's `created_at`, NOT the
//!     wall clock, so relative session ordering is preserved (VAL-CASESS-012).
//!   - The legacy SQLite tables are NOT dropped — migration is additive and
//!     re-runnable; dropping the legacy tables is a later milestone.

use std::path::Path;
use std::sync::Arc;

use hand_ai_model::Message;

use crate::models::AppError;
use crate::services::agent_jsonl_store::{
    session_activity, session_cwd, session_path, write_transcript_atomic,
};
use crate::storage::types::AgentSession;
use crate::storage::{AgentSessionRepository, Database};

/// Page size for walking the full session list. `list_sessions` is paginated
/// (limit/offset); we drain every page so a large legacy library migrates in
/// full rather than only its first page.
const SESSION_PAGE_SIZE: i32 = 500;

/// Outcome of a migration pass. Counts let the caller log how much was
/// materialized vs. skipped (and how much was repaired) without re-querying.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Sessions that had no complete `<id>.jsonl` yet and were materialized into
    /// a fresh one this pass.
    pub migrated_sessions: usize,
    /// Sessions whose `<id>.jsonl` already existed AND was complete (its message
    /// count matched the good SQLite rows) — skipped unchanged (idempotent
    /// re-run, or a post-M3 native session).
    pub skipped_existing: usize,
    /// Sessions whose `<id>.jsonl` existed but was INCOMPLETE — header-less
    /// (corrupt) or a message-count mismatch (a half-written / crashed-mid-
    /// migration file) — and so was REWRITTEN to a complete copy this pass
    /// (VAL-CASESS-025).
    pub rewritten_sessions: usize,
    /// Sessions skipped because they had zero SQLite messages (no JSONL built —
    /// they stay anchored to their SQLite created_at).
    pub skipped_empty: usize,
    /// Sessions left wholly unmaterialized because EVERY one of their SQLite
    /// payloads failed to deserialize into a `Message` (so there was nothing
    /// migratable). A session with a MIX of good and bad rows is migrated with
    /// its good rows; only an all-bad session lands here.
    pub skipped_undeserializable: usize,
    /// Individual transcript ROWS dropped because their payload could not be
    /// deserialized into a `Message` (VAL-CASESS-015). Counted across all
    /// sessions; the session's other rows still migrate.
    pub skipped_rows: usize,
    /// Sessions skipped because a per-session fatal error (e.g. a transient IO
    /// error reading or writing it) was logged and the migration moved on rather
    /// than aborting the whole pass — sibling sessions are unaffected.
    pub errored_sessions: usize,
    /// Total `Message` rows written across all migrated/rewritten sessions.
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
            // Sibling isolation (VAL-CASESS-015): a fatal error migrating ONE
            // session is logged + counted, never propagated — the loop moves on
            // so a single bad session can't abort the whole pass and strand its
            // siblings unmigrated.
            if let Err(e) = migrate_one_session(&repository, base_dir, session, &mut report).await {
                tracing::warn!(
                    session_id = %session.id,
                    error = %e,
                    "skipping migration of agent session: fatal per-session error; \
                     continuing with sibling sessions"
                );
                report.errored_sessions += 1;
            }
        }
        if page_len < SESSION_PAGE_SIZE as usize {
            break;
        }
        offset += SESSION_PAGE_SIZE;
    }

    Ok(report)
}

/// Materialize a single session's transcript, updating `report` in place. Pulled
/// out of the page loop so per-session fault handling wraps one well-defined
/// unit and a fatal error here is isolated to this session (the caller catches
/// it, counts `errored_sessions`, and continues with siblings).
async fn migrate_one_session(
    repository: &AgentSessionRepository,
    base_dir: &Path,
    session: &AgentSession,
    report: &mut MigrationReport,
) -> Result<(), AppError> {
    let messages = repository.list_messages(&session.id).await?;

    // (1) Empty session → no JSONL file; the sidebar coalesces to created_at.
    if messages.is_empty() {
        report.skipped_empty += 1;
        return Ok(());
    }

    // (2) Per-ROW corrupt-skip (VAL-CASESS-015): deserialize every payload, but a
    // single bad row is dropped (logged + counted) rather than aborting the whole
    // session. The session's GOOD rows still migrate; only an all-bad session is
    // left unmaterialized. Each good message carries the SQLite row's `created_at`
    // — the entry-level timestamp that keeps the migrated session's last-activity
    // key equal to its pre-migration value (VAL-CASESS-012).
    let mut decoded: Vec<(Message, i64)> = Vec::with_capacity(messages.len());
    for row in &messages {
        match serde_json::from_value::<Message>(row.payload.clone()) {
            Ok(message) => decoded.push((message, row.created_at)),
            Err(e) => {
                tracing::warn!(
                    session_id = %session.id,
                    seq = row.seq,
                    error = %e,
                    "skipping migration of one transcript row: payload is not a valid \
                     hand-agent Message; the session's other rows still migrate"
                );
                report.skipped_rows += 1;
            }
        }
    }

    // (3) Nothing migratable (every row was corrupt): leave the session wholly
    // unmaterialized so a re-run after a fix can still migrate it cleanly.
    if decoded.is_empty() {
        report.skipped_undeserializable += 1;
        return Ok(());
    }

    let cwd = session_cwd(session.working_dir.as_deref(), base_dir);
    let path = session_path(base_dir, &cwd, &session.id);

    // (4) Completeness-aware idempotency (VAL-CASESS-025). The "already migrated?"
    // test is not "does the file exist" but "does a COMPLETE file exist":
    //   - absent              → write (fresh materialization)
    //   - present + complete  → skip   (re-run no-op; never doubles a transcript)
    //   - present + incomplete→ rewrite (header-less corrupt file, or a message
    //                            count that disagrees with the good rows — i.e. a
    //                            half-written / crashed-mid-migration file)
    // "Complete" = `session_activity` reads a header (Some) AND its `message_count`
    // equals the number of good rows we are about to write. The rewrite is atomic
    // (temp file + rename), so even repairing a corrupt file never exposes a
    // truncated official file.
    if path.exists() {
        match completeness(base_dir, &cwd, &session.id, decoded.len()) {
            Completeness::Complete => {
                report.skipped_existing += 1;
                return Ok(());
            }
            Completeness::Incomplete => {
                write_transcript_atomic(base_dir, &cwd, &session.id, session.created_at, &decoded)?;
                report.rewritten_sessions += 1;
                report.messages_migrated += decoded.len();
                return Ok(());
            }
        }
    }

    // (5) Fresh materialization: write the whole transcript in one atomic step.
    // The header stamps the SQLite created_at (not now); each entry stamps its
    // SQLite per-row created_at, so on-disk order and activity key match the
    // source exactly.
    let migrated = decoded.len();
    write_transcript_atomic(base_dir, &cwd, &session.id, session.created_at, &decoded)?;
    report.migrated_sessions += 1;
    report.messages_migrated += migrated;
    Ok(())
}

/// Whether an existing `<id>.jsonl` is a complete materialization of the
/// `expected_messages` good SQLite rows.
enum Completeness {
    /// Header reads AND message count matches — leave it untouched.
    Complete,
    /// Header-less (corrupt) OR message-count mismatch — rewrite it.
    Incomplete,
}

/// Classify an existing `<id>.jsonl` as complete vs. incomplete for the
/// migration's completeness-aware idempotency (VAL-CASESS-025).
///
/// A read error (a file so corrupt the upstream parser cannot even open it, or a
/// header-less file → `Ok(None)`) is treated as INCOMPLETE: the safe action is
/// to rewrite from the authoritative SQLite rows rather than trust an unreadable
/// file. Only a file that both reads a header AND reports exactly
/// `expected_messages` is considered complete.
fn completeness(
    base_dir: &Path,
    cwd: &Path,
    session_id: &str,
    expected_messages: usize,
) -> Completeness {
    match session_activity(base_dir, cwd, session_id) {
        Ok(Some(activity)) if activity.message_count as usize == expected_messages => {
            Completeness::Complete
        }
        // Header-less / corrupt (None), a count mismatch, or an unreadable file:
        // rewrite from SQLite to converge on a complete transcript.
        _ => Completeness::Incomplete,
    }
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

    /// A payload that is valid JSON but NOT a valid hand-agent `Message` — used
    /// to seed a corrupt transcript row that `serde_json::from_value::<Message>`
    /// will reject. Appending it through the repository assigns it a real seq and
    /// stores it like any other row.
    fn corrupt_payload() -> serde_json::Value {
        serde_json::json!({ "not": "a message", "shape": [1, 2, 3] })
    }

    /// VAL-CASESS-015: a session whose transcript interleaves CORRUPT rows
    /// (payloads that are not valid `Message`s) among valid ones migrates its
    /// GOOD rows while dropping (and counting) only the bad rows — it is no
    /// longer left wholly unmaterialized. A separate sibling session migrates
    /// cleanly and is unaffected by its neighbor's corruption.
    #[tokio::test]
    async fn migration_skips_corrupt_rows_and_keeps_the_rest_and_siblings() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        // Session A: good, BAD, good, BAD, good → 3 good rows, 2 corrupt.
        let a = session_row("sess-mixed", "Mixed", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&a).await.unwrap();
        repo.append_message(&a.id, "user", &user_payload("g0"), 1_700_000_000_001)
            .await
            .unwrap();
        repo.append_message(&a.id, "assistant", &corrupt_payload(), 1_700_000_000_002)
            .await
            .unwrap();
        repo.append_message(&a.id, "user", &user_payload("g1"), 1_700_000_000_003)
            .await
            .unwrap();
        repo.append_message(&a.id, "assistant", &corrupt_payload(), 1_700_000_000_004)
            .await
            .unwrap();
        repo.append_message(&a.id, "user", &user_payload("g2"), 1_700_000_000_005)
            .await
            .unwrap();

        // Sibling session B: all good — must migrate untouched by A's corruption.
        let b = session_row("sess-clean", "Clean", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&b).await.unwrap();
        for i in 0..2 {
            repo.append_message(
                &b.id,
                "user",
                &user_payload(&format!("b{i}")),
                1_700_000_000_010 + i,
            )
            .await
            .unwrap();
        }

        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();

        // Both sessions materialized; A kept its 3 good rows, dropped 2 bad rows.
        assert_eq!(report.migrated_sessions, 2, "both sessions materialized");
        assert_eq!(
            report.skipped_rows, 2,
            "exactly the two corrupt rows dropped"
        );
        assert_eq!(
            report.skipped_undeserializable, 0,
            "no session was all-bad, so none is wholly skipped"
        );
        assert_eq!(report.messages_migrated, 5, "3 good (A) + 2 good (B)");

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let a_rows = load_transcript(base.path(), &jsonl_cwd, &a.id)
            .unwrap()
            .expect("A migrated its good rows");
        assert_eq!(a_rows.len(), 3, "only A's 3 good rows survive");

        let b_rows = load_transcript(base.path(), &jsonl_cwd, &b.id)
            .unwrap()
            .expect("sibling B migrated cleanly");
        assert_eq!(b_rows.len(), 2, "sibling B is unaffected by A's corruption");
    }

    /// VAL-CASESS-015 (all-bad leg): a session whose EVERY payload is corrupt is
    /// left wholly unmaterialized (no JSONL file) and counted as
    /// `skipped_undeserializable`, while its good sibling still migrates. A
    /// re-run after the data is fixed could then migrate it cleanly.
    #[tokio::test]
    async fn migration_leaves_all_corrupt_session_unmaterialized_but_not_siblings() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let bad = session_row("sess-all-bad", "AllBad", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&bad).await.unwrap();
        for i in 0..3 {
            repo.append_message(&bad.id, "user", &corrupt_payload(), 1_700_000_000_000 + i)
                .await
                .unwrap();
        }

        let good = session_row("sess-good", "Good", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&good).await.unwrap();
        repo.append_message(&good.id, "user", &user_payload("ok"), 1_700_000_000_100)
            .await
            .unwrap();

        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();

        assert_eq!(
            report.skipped_undeserializable, 1,
            "the all-bad session is skipped"
        );
        assert_eq!(report.skipped_rows, 3, "all three bad rows counted");
        assert_eq!(
            report.migrated_sessions, 1,
            "only the good session migrates"
        );

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        assert!(
            session_activity(base.path(), &jsonl_cwd, &bad.id)
                .unwrap()
                .is_none(),
            "an all-corrupt session must build no JSONL file (re-runnable after a fix)"
        );
        assert_eq!(
            load_transcript(base.path(), &jsonl_cwd, &good.id)
                .unwrap()
                .unwrap()
                .len(),
            1,
            "sibling good session migrated"
        );
    }

    /// VAL-CASESS-025 (half-migration crash → converge): a session left with a
    /// HALF-WRITTEN `<id>.jsonl` (header + fewer messages than SQLite, as if the
    /// migration was killed mid-write) is REWRITTEN on the next pass to a single
    /// COMPLETE transcript whose message count equals the SQLite count — never a
    /// doubled or still-truncated file.
    #[tokio::test]
    async fn migration_rewrites_a_half_written_file_to_a_complete_one() {
        use crate::services::agent_jsonl_store::write_transcript_atomic;

        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let session = session_row("sess-half", "Half", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&session).await.unwrap();
        for i in 0..4 {
            repo.append_message(
                &session.id,
                "user",
                &user_payload(&format!("m{i}")),
                1_700_000_000_000 + i,
            )
            .await
            .unwrap();
        }
        let sqlite_count = sqlite_message_count(&db, &session.id).await;
        assert_eq!(sqlite_count, 4);

        // Simulate a crash mid-migration: an existing file with only the first
        // TWO of the four messages.
        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let partial = vec![
            (
                Message::User(UserMessage::new_text("m0")),
                1_700_000_000_000_i64,
            ),
            (
                Message::User(UserMessage::new_text("m1")),
                1_700_000_000_001_i64,
            ),
        ];
        write_transcript_atomic(
            base.path(),
            &jsonl_cwd,
            &session.id,
            session.created_at,
            &partial,
        )
        .unwrap();
        assert_eq!(
            load_transcript(base.path(), &jsonl_cwd, &session.id)
                .unwrap()
                .unwrap()
                .len(),
            2,
            "precondition: a half-written file with 2 of 4 messages"
        );

        // Re-run: the incomplete file is rewritten to the full transcript.
        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(
            report.rewritten_sessions, 1,
            "the incomplete file was rewritten"
        );
        assert_eq!(
            report.migrated_sessions, 0,
            "not counted as a fresh materialization"
        );

        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            rows.len() as i64,
            sqlite_count,
            "after rewrite the JSONL count equals the SQLite count exactly"
        );

        // Exactly one official .jsonl file and no stray temp leftover.
        let dir = crate::services::agent_jsonl_store::session_dir(base.path(), &jsonl_cwd);
        let entries: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let jsonl: Vec<_> = entries.iter().filter(|n| n.ends_with(".jsonl")).collect();
        let tmp: Vec<_> = entries.iter().filter(|n| n.ends_with(".tmp")).collect();
        assert_eq!(jsonl.len(), 1, "exactly one complete transcript: {jsonl:?}");
        assert!(tmp.is_empty(), "no temp ghost remains: {tmp:?}");
    }

    /// VAL-CASESS-025 (corrupt-header leg): an existing `<id>.jsonl` whose header
    /// is corrupt (so `session_activity` reads `None`) is REWRITTEN on the next
    /// pass to a complete, readable transcript — a header-less ghost converges to
    /// one good file.
    #[tokio::test]
    async fn migration_rewrites_a_corrupt_header_file() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let session = session_row("sess-badhdr", "BadHdr", Some(&cwd_str), 1_700_000_000_000);
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

        // Plant a header-less corrupt file at the session's JSONL path.
        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let dir = crate::services::agent_jsonl_store::session_dir(base.path(), &jsonl_cwd);
        std::fs::create_dir_all(&dir).unwrap();
        let path =
            crate::services::agent_jsonl_store::session_path(base.path(), &jsonl_cwd, &session.id);
        std::fs::write(&path, "garbage first line, not a header\n").unwrap();
        assert!(
            session_activity(base.path(), &jsonl_cwd, &session.id)
                .unwrap()
                .is_none(),
            "precondition: the planted file has no valid header"
        );

        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(
            report.rewritten_sessions, 1,
            "the corrupt-header file was rewritten"
        );

        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .expect("the rewritten file is a valid transcript");
        assert_eq!(rows.len(), 3, "the rewrite produced the full transcript");
    }

    /// VAL-CASESS-017: running the migration TWICE neither doubles a transcript
    /// nor rebuilds an already-complete file, and a get-or-create on the same
    /// project path between passes does not reset the existing project's
    /// `created_at` / `name`. (The migration itself never touches projects;
    /// project grouping is derived at read time. This guards the two pieces 017
    /// rests on: idempotent transcript materialization + get-or-create that
    /// preserves an existing project.)
    #[tokio::test]
    async fn migration_rerun_is_idempotent_and_does_not_reset_an_existing_project() {
        use crate::services::agent_project::AgentProjectService;

        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        let session = session_row("sess-017", "Idem017", Some(&cwd_str), 1_700_000_000_000);
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

        // First migration pass.
        let first = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(first.migrated_sessions, 1);
        assert_eq!(first.messages_migrated, 3);

        // get-or-create a project on this cwd between passes.
        let project_service = AgentProjectService::new(db.clone());
        let canonical = std::fs::canonicalize(&cwd).unwrap();
        let canonical_str = canonical.to_string_lossy().into_owned();
        let created = project_service
            .create_project(canonical_str.clone())
            .await
            .unwrap();

        // Second migration pass: nothing doubles, the complete file is skipped.
        let second = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .unwrap();
        assert_eq!(
            second.migrated_sessions, 0,
            "second pass materializes nothing"
        );
        assert_eq!(
            second.rewritten_sessions, 0,
            "a complete file is not rewritten"
        );
        assert_eq!(
            second.skipped_existing, 1,
            "the complete session is skipped"
        );

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .unwrap();
        assert_eq!(rows.len(), 3, "the transcript was not doubled");

        // get-or-create on the SAME path returns the same row unchanged.
        let again = project_service.create_project(canonical_str).await.unwrap();
        assert_eq!(again.id, created.id, "same path → same project");
        assert_eq!(again.created_at, created.created_at, "created_at not reset");
        assert_eq!(again.name, created.name, "name not reset");
    }

    /// VAL-CASESS-020 (migration leg): when the JSONL base directory is read-only
    /// so a session's transcript genuinely cannot be written, the migration does
    /// NOT abort or leave a half-written file — it logs + counts the session as
    /// `errored_sessions` and moves on. No `<id>.jsonl` is materialized, the
    /// SQLite source is untouched (no ghost rows), and the overall pass still
    /// returns `Ok` so other (writable) work is not blocked.
    ///
    /// `#[cfg(unix)]` + a non-root self-skip, since stripping the write bit is a
    /// no-op for root.
    #[cfg(unix)]
    #[tokio::test]
    async fn migration_into_readonly_dir_errors_session_without_leaving_a_file() {
        use std::os::unix::fs::PermissionsExt;

        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());

        // A session with no working_dir → its JSONL would land under
        // `<base>/sessions/<flattened-base>/`. We make `base` itself read-only so
        // the `sessions/` subtree cannot be created/written.
        let session = session_row("sess-ro", "ReadOnly", None, 1_700_000_000_000);
        repo.create_session(&session).await.unwrap();
        for i in 0..2 {
            repo.append_message(
                &session.id,
                "user",
                &user_payload(&format!("m{i}")),
                1_700_000_000_000 + i,
            )
            .await
            .unwrap();
        }

        let mut perms = std::fs::metadata(base.path()).unwrap().permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(base.path(), perms).unwrap();

        // Self-skip when not actually read-only (running as root).
        if std::fs::create_dir(base.path().join(".writable-probe")).is_ok() {
            let _ = std::fs::remove_dir(base.path().join(".writable-probe"));
            let mut restore = std::fs::metadata(base.path()).unwrap().permissions();
            restore.set_mode(0o755);
            std::fs::set_permissions(base.path(), restore).unwrap();
            eprintln!("skipping read-only migration test: base dir still writable (root?)");
            return;
        }

        let report = migrate_sqlite_sessions_to_jsonl(db.clone(), base.path())
            .await
            .expect("the overall migration still returns Ok despite one errored session");
        assert_eq!(
            report.errored_sessions, 1,
            "the unwritable session is counted as errored, not silently lost"
        );
        assert_eq!(report.migrated_sessions, 0, "nothing was materialized");

        // Restore write permission so the TempDir cleans up and so we can inspect.
        let mut restore = std::fs::metadata(base.path()).unwrap().permissions();
        restore.set_mode(0o755);
        std::fs::set_permissions(base.path(), restore).unwrap();

        // No JSONL was written (the session reads back as transcript-less), and
        // the SQLite source rows are intact (no ghosting).
        let jsonl_cwd = session_cwd(None, base.path());
        assert!(
            load_transcript(base.path(), &jsonl_cwd, &session.id)
                .unwrap()
                .is_none(),
            "a failed write must leave no JSONL transcript behind"
        );
        assert_eq!(
            sqlite_message_count(&db, &session.id).await,
            2,
            "the SQLite source transcript is untouched — no ghost rows, re-runnable"
        );
    }
}
