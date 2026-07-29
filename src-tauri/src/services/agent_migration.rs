//! One-shot SQLite→JSONL materialization of legacy agent transcripts: rows in
//! `agent_session_messages` already hold serialized [`hand_ai_model::Message`]s,
//! replayed once into each session's authoritative `<id>.jsonl` with entry
//! timestamps taken from the row's `created_at` so session ordering survives.
//! That table's existence is the "migration pending" flag and it is dropped
//! after a successful pass; `agent_sessions` / `agent_projects` stay live.

use std::path::Path;
use std::sync::Arc;

use hand_ai_model::Message;
use sqlx::Row;

use crate::models::AppError;
use crate::services::agent_jsonl_store::{
    session_activity, session_cwd, session_path, write_transcript_atomic,
};
use crate::storage::types::AgentSession;
use crate::storage::{AgentSessionRepository, Database};

/// Page size for draining the paginated `list_sessions`, so a large legacy
/// library migrates in full rather than only its first page.
const SESSION_PAGE_SIZE: i32 = 500;

/// Outcome of a migration pass. Counts let the caller log how much was
/// materialized vs. skipped (and how much was repaired) without re-querying.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Sessions materialized into a fresh `<id>.jsonl` this pass.
    pub migrated_sessions: usize,
    /// Sessions whose `<id>.jsonl` already existed AND was complete — skipped
    /// unchanged (idempotent re-run, or a natively JSONL-backed session).
    pub skipped_existing: usize,
    /// Sessions whose `<id>.jsonl` existed but was INCOMPLETE (header-less or a
    /// message-count mismatch) and so was REWRITTEN to a complete copy.
    pub rewritten_sessions: usize,
    /// Sessions with zero SQLite messages: no JSONL is built, they stay anchored
    /// to their SQLite created_at.
    pub skipped_empty: usize,
    /// Sessions left wholly unmaterialized because EVERY payload failed to
    /// deserialize into a `Message`; a session with a MIX of good and bad rows
    /// migrates its good rows instead.
    pub skipped_undeserializable: usize,
    /// Individual transcript ROWS dropped as undeserializable; the session's
    /// other rows still migrate.
    pub skipped_rows: usize,
    /// Sessions skipped after a per-session fatal error (e.g. transient IO): it
    /// is logged and counted rather than aborting the pass, so siblings survive.
    pub errored_sessions: usize,
    /// Total `Message` rows written across all migrated/rewritten sessions.
    pub messages_migrated: usize,
}

/// Replay every legacy SQLite agent transcript into its `<id>.jsonl` once.
///
/// The cwd a session's JSONL is keyed by MUST come from the same
/// [`session_cwd`] the writer uses (`base_dir` is both the JSONL base and the
/// no-`working_dir` fallback), or the reader looks in the wrong
/// `<flattened-cwd>` subdir. No JSONL label is written, so the SQLite `name`
/// stays the authoritative display name.
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
            // Sibling isolation: a fatal error migrating ONE session is logged
            // + counted, never propagated, so it cannot abort the whole pass.
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

/// Outcome of the gated migrate-then-drop entry point.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrateAndDropReport {
    /// `true` when the legacy table existed on entry, so this pass migrated and
    /// (on success) dropped it; `false` when it was already absent.
    pub ran: bool,
    /// The migration pass result, present only when `ran` is true.
    pub migration: Option<MigrationReport>,
    /// `true` when the legacy table was dropped this pass (migration succeeded).
    pub dropped: bool,
}

/// Gated one-time migration + drop of the legacy transcript table.
///
/// The presence of `agent_session_messages` IS the "migration not yet complete"
/// flag: present → migrate then drop, absent → skip both. Ordering is
/// load-bearing — a migration error leaves the table (and the transcript) in
/// place for a retry on the next startup.
pub async fn migrate_and_drop_legacy_if_present(
    db: Arc<Database>,
    base_dir: &Path,
) -> Result<MigrateAndDropReport, AppError> {
    if !legacy_transcript_table_exists(&db).await? {
        // Already migrated + dropped on an earlier startup: skip entirely.
        return Ok(MigrateAndDropReport::default());
    }

    // Migrate first; only drop if the whole pass succeeded.
    let migration = migrate_sqlite_sessions_to_jsonl(Arc::clone(&db), base_dir).await?;
    drop_legacy_transcript_table(&db).await?;

    Ok(MigrateAndDropReport {
        ran: true,
        migration: Some(migration),
        dropped: true,
    })
}

/// Whether the legacy `agent_session_messages` transcript table still exists —
/// a single boolean answering "has the one-time migration + drop already run?".
async fn legacy_transcript_table_exists(db: &Database) -> Result<bool, AppError> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS c FROM sqlite_master \
         WHERE type = 'table' AND name = 'agent_session_messages'",
    )
    .fetch_one(db.pool())
    .await
    .map_err(|e| {
        AppError::internal_error(&format!(
            "Failed to probe for legacy agent_session_messages table: {}",
            e
        ))
    })?;
    let count: i64 = row.try_get("c")?;
    Ok(count > 0)
}

/// IRREVERSIBLE — only call after a SUCCESSFUL migration pass (the transcript
/// already lives in JSONL). `agent_sessions` / `agent_projects` are deliberately
/// untouched: they remain the live config + grouping source.
async fn drop_legacy_transcript_table(db: &Database) -> Result<(), AppError> {
    sqlx::query("DROP TABLE IF EXISTS agent_session_messages")
        .execute(db.pool())
        .await
        .map_err(|e| {
            AppError::internal_error(&format!(
                "Failed to drop legacy agent_session_messages table: {}",
                e
            ))
        })?;
    Ok(())
}

/// Materialize a single session's transcript, updating `report` in place. A
/// fatal error here is isolated to this session: the caller counts it as
/// `errored_sessions` and continues with siblings.
async fn migrate_one_session(
    repository: &AgentSessionRepository,
    base_dir: &Path,
    session: &AgentSession,
    report: &mut MigrationReport,
) -> Result<(), AppError> {
    let messages = repository.list_messages(&session.id).await?;

    // Empty session → no JSONL file; the sidebar coalesces to created_at.
    if messages.is_empty() {
        report.skipped_empty += 1;
        return Ok(());
    }

    // A single bad row is dropped (logged + counted) rather than aborting the
    // session. Each good message carries the SQLite row's `created_at` so the
    // migrated session keeps its pre-migration last-activity key.
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

    // Nothing migratable: leave the session unmaterialized so a re-run after a
    // fix can still migrate it cleanly.
    if decoded.is_empty() {
        report.skipped_undeserializable += 1;
        return Ok(());
    }

    let cwd = session_cwd(session.working_dir.as_deref(), base_dir);
    let path = session_path(base_dir, &cwd, &session.id);

    // Completeness-aware idempotency: the "already migrated?" test is not "does
    // the file exist" but "does a COMPLETE file exist", so a header-less or
    // count-mismatched half-written file is atomically rewritten, not skipped.
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

    // The header stamps the SQLite created_at (not now) and each entry its own
    // row's created_at, so on-disk order and activity key match the source.
    let migrated = decoded.len();
    write_transcript_atomic(base_dir, &cwd, &session.id, session.created_at, &decoded)?;
    report.migrated_sessions += 1;
    report.messages_migrated += migrated;
    Ok(())
}

/// Whether an existing `<id>.jsonl` fully materializes the good SQLite rows.
enum Completeness {
    /// Header reads AND message count matches — leave it untouched.
    Complete,
    /// Header-less (corrupt) OR message-count mismatch — rewrite it.
    Incomplete,
}

/// A read error or a header-less file counts as INCOMPLETE: rewriting from the
/// authoritative SQLite rows is safer than trusting an unreadable file. Only a
/// file that reads a header AND reports exactly `expected_messages` is complete.
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
        // Header-less, count mismatch, or unreadable: rewrite from SQLite.
        _ => Completeness::Incomplete,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_jsonl_store::{load_transcript, session_activity};
    use crate::storage::types::{AgentSession, AgentSessionMessage, Timestamp};
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
            agent_definition_id: None,
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: None,
            thinking_level: None,
            temperature: None,
            max_tokens: None,
            working_dir: working_dir.map(str::to_string),
            enabled_tools: Vec::new(),
            mcp_servers: Vec::new(),
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

    /// Content-fidelity fixture: text, thinking, and tool-call blocks together.
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

    async fn sqlite_message_count(db: &Database, session_id: &str) -> i64 {
        sqlx::query("SELECT COUNT(*) AS c FROM agent_session_messages WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(db.pool())
            .await
            .unwrap()
            .try_get::<i64, _>("c")
            .unwrap()
    }

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

    /// `load_transcript` restores the content blocks rather than flattening to
    /// text, so the per-message block count matches the original.
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

    /// The SQLite key is `coalesce(last_message_at, created_at)`; the JSONL key
    /// is the max entry timestamp, stamped from each message's SQLite
    /// `created_at`. Both must order the sessions identically.
    #[tokio::test]
    async fn migration_preserves_relative_session_order() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        // Deliberately scrambled creation order so the assertion can't pass by
        // accident of insertion order. Each `created_at` is both the SQLite
        // last_message_at and the JSONL entry timestamp the migration replays.
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
        activities.sort_by(|a, b| b.1.cmp(&a.1));
        let sorted_ids: Vec<&str> = activities.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            sorted_ids,
            expected_desc.to_vec(),
            "post-migration activity ordering must match pre-migration ordering"
        );
    }

    /// No JSONL label is written (so the overlay keeps the SQLite name
    /// authoritative), and a session with zero messages builds no file at all.
    #[tokio::test]
    async fn migration_writes_no_label_and_skips_empty_sessions() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

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

        let act = session_activity(base.path(), &jsonl_cwd, &with_msg.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            act.name, None,
            "migration must not write a JSONL label; SQLite name stays authoritative"
        );

        assert!(
            session_activity(base.path(), &jsonl_cwd, &empty.id)
                .unwrap()
                .is_none(),
            "an empty session must not be materialized into a JSONL file"
        );
    }

    /// A second pass neither doubles the transcript nor rewrites the file.
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

    /// The migration's cwd derivation matches the writer's `session_cwd(None,
    /// base_dir)` fallback, so the reader finds the file.
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

    /// A JSONL session's project group name comes from the same
    /// `default_project_name` the SQLite `agent_projects.name` uses, so every
    /// spelling of one cwd must derive the identical name.
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

        assert_eq!(
            default_project_name("/"),
            "/",
            "root path (empty basename) falls back to the full path"
        );
    }

    /// Valid JSON that is NOT a valid hand-agent `Message`, so
    /// `serde_json::from_value::<Message>` rejects it.
    fn corrupt_payload() -> serde_json::Value {
        serde_json::json!({ "not": "a message", "shape": [1, 2, 3] })
    }

    /// Interleaved corrupt rows are dropped and counted while the session's good
    /// rows migrate; a sibling session is unaffected by the corruption.
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

    /// An all-corrupt session builds no file and is counted as
    /// `skipped_undeserializable`; its good sibling still migrates.
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

    /// A file with fewer messages than SQLite (as if killed mid-write) is
    /// rewritten to one complete transcript — never doubled, never truncated.
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

        // Simulate a crash mid-migration: a file with only 2 of the 4 messages.
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

    /// A file whose header is corrupt (so `session_activity` reads `None`) is
    /// rewritten to a complete, readable transcript.
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

    /// A get-or-create on the same project path between two passes must not
    /// reset the existing project's `created_at` / `name` — the migration never
    /// touches projects, grouping is derived at read time.
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

        let again = project_service.create_project(canonical_str).await.unwrap();
        assert_eq!(again.id, created.id, "same path → same project");
        assert_eq!(again.created_at, created.created_at, "created_at not reset");
        assert_eq!(again.name, created.name, "name not reset");
    }

    /// An unwritable JSONL dir counts the session as `errored_sessions` and moves
    /// on: no file left behind, SQLite untouched, pass still `Ok`. Self-skips
    /// when not root, since stripping the write bit is a no-op for root.
    #[cfg(unix)]
    #[tokio::test]
    async fn migration_into_readonly_dir_errors_session_without_leaving_a_file() {
        use std::os::unix::fs::PermissionsExt;

        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());

        // No working_dir → the JSONL lands under `<base>/sessions/...`, so making
        // `base` itself read-only makes that whole subtree unwritable.
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

    async fn table_exists(db: &Database, name: &str) -> bool {
        let c: i64 = sqlx::query(
            "SELECT COUNT(*) AS c FROM sqlite_master WHERE type = 'table' AND name = $1",
        )
        .bind(name)
        .fetch_one(db.pool())
        .await
        .unwrap()
        .try_get("c")
        .unwrap();
        c > 0
    }

    /// The gated entry migrates every transcript (count parity per session) then
    /// drops the legacy table, LEAVING `agent_sessions` / `agent_projects`
    /// intact — the live config + grouping source must never be dropped.
    #[tokio::test]
    async fn gated_migration_drops_only_the_legacy_transcript_table() {
        let (db, base) = test_db().await;
        let repo = AgentSessionRepository::new(db.clone());
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();
        let cwd_str = cwd.to_string_lossy().into_owned();

        // Seed an agent_projects row + a session attached to it, with messages.
        let project_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO agent_projects (id, path, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&project_id)
        .bind(&cwd_str)
        .bind("proj")
        .bind(1_700_000_000_000_i64)
        .bind(1_700_000_000_000_i64)
        .execute(db.pool())
        .await
        .unwrap();

        let session = session_row("sess-023", "Drop", Some(&cwd_str), 1_700_000_000_000);
        repo.create_session(&session).await.unwrap();
        for i in 0..5 {
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
        assert_eq!(sqlite_count, 5);

        assert!(
            table_exists(&db, "agent_session_messages").await,
            "precondition: legacy table present"
        );

        let report = migrate_and_drop_legacy_if_present(db.clone(), base.path())
            .await
            .unwrap();
        assert!(report.ran, "the gated pass ran because the table existed");
        assert!(
            report.dropped,
            "the legacy table was dropped after migrating"
        );
        assert_eq!(report.migration.as_ref().unwrap().migrated_sessions, 1);

        assert!(
            !table_exists(&db, "agent_session_messages").await,
            "legacy transcript table must be dropped"
        );
        assert!(
            table_exists(&db, "agent_sessions").await,
            "agent_sessions (config) must NOT be dropped"
        );
        assert!(
            table_exists(&db, "agent_projects").await,
            "agent_projects (grouping) must NOT be dropped"
        );

        let jsonl_cwd = session_cwd(Some(&cwd_str), base.path());
        let rows = load_transcript(base.path(), &jsonl_cwd, &session.id)
            .unwrap()
            .expect("a migrated session has a JSONL transcript");
        assert_eq!(
            rows.len() as i64,
            sqlite_count,
            "JSONL message rows equal the pre-drop SQLite count"
        );

        // The config row itself survived.
        assert!(repo.get_session_by_id(&session.id).await.unwrap().is_some());
    }

    /// The table's absence IS the "already done" flag: a second gated pass skips
    /// both migration and drop — no error, no re-scan, no read of a dropped table.
    #[tokio::test]
    async fn gated_migration_second_run_skips_when_table_absent() {
        let (db, base) = test_db().await;

        // Drop the table up-front to model a post-migration second startup.
        sqlx::query("DROP TABLE agent_session_messages")
            .execute(db.pool())
            .await
            .unwrap();
        assert!(!table_exists(&db, "agent_session_messages").await);

        let report = migrate_and_drop_legacy_if_present(db.clone(), base.path())
            .await
            .expect("a second pass over an absent table is a clean no-op");
        assert!(!report.ran, "nothing ran: the table was already absent");
        assert!(!report.dropped);
        assert!(report.migration.is_none());

        // The config + grouping tables are still present and intact.
        assert!(table_exists(&db, "agent_sessions").await);
        assert!(table_exists(&db, "agent_projects").await);
    }

    /// A failed migration must leave the legacy table in place — the
    /// irreversible drop only happens once the data is safely in JSONL. Failure
    /// is injected by dropping `agent_sessions` so `list_sessions` errors.
    #[tokio::test]
    async fn gated_migration_does_not_drop_when_migration_fails() {
        let (db, base) = test_db().await;

        // Sabotage the migration: drop the table it must read first, so the pass
        // returns Err BEFORE any drop of the transcript table.
        sqlx::query("DROP TABLE agent_sessions")
            .execute(db.pool())
            .await
            .unwrap();
        assert!(
            table_exists(&db, "agent_session_messages").await,
            "precondition: the transcript table is still present"
        );

        let err = migrate_and_drop_legacy_if_present(db.clone(), base.path())
            .await
            .expect_err("a failing migration must surface an error");
        assert_eq!(err.code, "INTERNAL_ERROR");

        assert!(
            table_exists(&db, "agent_session_messages").await,
            "a failed migration must leave the transcript table intact (no drop)"
        );
    }

    /// Per-session-id JSONL keying isolates concurrent runs: two ids never
    /// cross-contaminate each other's transcript.
    #[tokio::test]
    async fn concurrent_sessions_write_isolated_transcripts() {
        use crate::services::agent_jsonl_store::append_message_at;
        use hand_ai_model::UserMessage;

        let base = TempDir::new().unwrap();
        let cwd = base.path().join("proj");
        std::fs::create_dir_all(&cwd).unwrap();

        let id_a = "session-aaaa";
        let id_b = "session-bbbb";
        let created = 1_700_000_000_000_i64;

        // Interleave appends to the two sessions, as concurrent runs would.
        for i in 0..3 {
            append_message_at(
                base.path(),
                &cwd,
                id_a,
                created,
                &Message::User(UserMessage::new_text(format!("a-{i}"))),
                created + i,
            )
            .unwrap();
            append_message_at(
                base.path(),
                &cwd,
                id_b,
                created,
                &Message::User(UserMessage::new_text(format!("b-{i}"))),
                created + 100 + i,
            )
            .unwrap();
        }
        // One extra message to A only — counts must stay independent.
        append_message_at(
            base.path(),
            &cwd,
            id_a,
            created,
            &Message::User(UserMessage::new_text("a-extra".to_string())),
            created + 50,
        )
        .unwrap();

        let rows_a = load_transcript(base.path(), &cwd, id_a).unwrap().unwrap();
        let rows_b = load_transcript(base.path(), &cwd, id_b).unwrap().unwrap();
        assert_eq!(rows_a.len(), 4, "A has its own 4 messages");
        assert_eq!(rows_b.len(), 3, "B has its own 3 messages, unpolluted by A");

        // `new_text` serializes `content` as a bare JSON string (untagged
        // UserContent::Text); pull it out robustly for the crossover assertion.
        let text_of = |row: &AgentSessionMessage| -> String {
            row.payload
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let a_texts: Vec<String> = rows_a.iter().map(text_of).collect();
        assert!(
            a_texts.iter().all(|t| t.starts_with("a-")),
            "every A row is an A message, none from B: {a_texts:?}"
        );
        let b_texts: Vec<String> = rows_b.iter().map(text_of).collect();
        assert!(
            b_texts.iter().all(|t| t.starts_with("b-")),
            "every B row is a B message, none from A: {b_texts:?}"
        );

        // Activity counts are independent (A=4, B=3).
        let act_a = session_activity(base.path(), &cwd, id_a).unwrap().unwrap();
        let act_b = session_activity(base.path(), &cwd, id_b).unwrap().unwrap();
        assert_eq!(act_a.message_count, 4);
        assert_eq!(act_b.message_count, 3);

        let dir = crate::services::agent_jsonl_store::session_dir(base.path(), &cwd);
        let jsonl: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".jsonl"))
            .collect();
        assert_eq!(
            jsonl.len(),
            2,
            "exactly one JSONL per session id: {jsonl:?}"
        );
    }
}
