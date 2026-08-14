// Persistence for Agent-mode sessions and their transcripts, built on the
// `agent_sessions` / `agent_session_messages` tables. Independent of
// chat-mode storage.

use crate::models::AppError;
use crate::storage::types::{AgentSession, AgentSessionMessage, Timestamp, UUID};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

#[derive(Clone)]
pub struct AgentSessionRepository {
    db: Arc<Database>,
}

impl AgentSessionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// The deprecated `enabled_skills` column stays in the schema but is never
    /// read or written — new rows leave it NULL.
    pub async fn create_session(&self, session: &AgentSession) -> Result<(), AppError> {
        let enabled_tools_json = serde_json::to_string(&session.enabled_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid enabled tools: {}", e)))?;
        let mcp_servers_json = serde_json::to_string(&session.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid mcp servers: {}", e)))?;

        let query = r#"
            INSERT INTO agent_sessions (id, name, project_id, agent_definition_id, model_id, provider_id, system_prompt, thinking_level, temperature, max_tokens, working_dir, enabled_tools, mcp_servers, tool_execution_mode, message_count, last_message_at, pinned, archived, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
        "#;

        sqlx::query(query)
            .bind(&session.id)
            .bind(&session.name)
            .bind(&session.project_id)
            .bind(&session.agent_definition_id)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&session.thinking_level)
            .bind(session.temperature)
            .bind(session.max_tokens)
            .bind(&session.working_dir)
            .bind(&enabled_tools_json)
            .bind(&mcp_servers_json)
            .bind(&session.tool_execution_mode)
            .bind(session.message_count)
            .bind(session.last_message_at)
            .bind(session.pinned)
            .bind(session.archived)
            .bind(session.created_at)
            .bind(session.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to create agent session: {}", e))
            })?;

        Ok(())
    }

    pub async fn list_sessions(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<AgentSession>, AppError> {
        let query = r#"
            SELECT id, name, project_id, agent_definition_id, model_id, provider_id, system_prompt, thinking_level, temperature, max_tokens, working_dir, enabled_tools, mcp_servers, tool_execution_mode, message_count, last_message_at, pinned, archived, created_at, updated_at
            FROM agent_sessions ORDER BY updated_at DESC LIMIT $1 OFFSET $2
        "#;

        let rows = sqlx::query(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list agent sessions: {}", e))
            })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(self.row_to_session(row)?);
        }

        Ok(sessions)
    }

    pub async fn get_session_by_id(
        &self,
        session_id: &UUID,
    ) -> Result<Option<AgentSession>, AppError> {
        let query = r#"
            SELECT id, name, project_id, agent_definition_id, model_id, provider_id, system_prompt, thinking_level, temperature, max_tokens, working_dir, enabled_tools, mcp_servers, tool_execution_mode, message_count, last_message_at, pinned, archived, created_at, updated_at
            FROM agent_sessions WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(session_id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to get agent session: {}", e))
            })?;

        if let Some(row) = row {
            Ok(Some(self.row_to_session(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update_session(&self, session: &AgentSession) -> Result<(), AppError> {
        let enabled_tools_json = serde_json::to_string(&session.enabled_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid enabled tools: {}", e)))?;
        let mcp_servers_json = serde_json::to_string(&session.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid mcp servers: {}", e)))?;

        // NOTE: `message_count` and `last_message_at` are deliberately OMITTED here.
        // Session-field edits go through a read-modify-write (`get_session` then
        // `update_session`) and can be triggered mid-run (e.g. the user changes the
        // thinking level / model while a run streams). Writing those two columns back
        // would clobber the atomic `message_count = message_count + 1` performed by
        // `append_message`, reverting the run's increments and mis-sorting the list.
        // `append_message` is therefore the SOLE writer of these two columns.
        //
        // `project_id` is likewise deliberately OMITTED: the project attachment is
        // write-once at `create_session` and must never be rewritten through the
        // generic update path (no "move session between projects" semantics).
        // `agent_definition_id` is OMITTED for the same reason: the originating
        // definition is a write-once provenance link set at instantiation; a session
        // never gets re-pointed at a different definition through field edits.
        //
        // `pinned` / `archived` are OMITTED too, for the read-modify-write reason:
        // toggling them is a one-column write ([`set_session_pinned`] /
        // [`set_session_archived`]), so a field edit built from a snapshot taken
        // before the toggle can never write the stale flag back.
        let query = r#"
            UPDATE agent_sessions SET name = $1, model_id = $2, provider_id = $3, system_prompt = $4, thinking_level = $5, temperature = $6, max_tokens = $7, working_dir = $8, enabled_tools = $9, mcp_servers = $10, tool_execution_mode = $11, updated_at = $12
            WHERE id = $13
        "#;

        let result = sqlx::query(query)
            .bind(&session.name)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&session.thinking_level)
            .bind(session.temperature)
            .bind(session.max_tokens)
            .bind(&session.working_dir)
            .bind(&enabled_tools_json)
            .bind(&mcp_servers_json)
            .bind(&session.tool_execution_mode)
            .bind(session.updated_at)
            .bind(&session.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update agent session: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session.id
            )));
        }

        Ok(())
    }

    /// Whole-row rewrite used only by `reinstantiate_from_definition` to
    /// re-point a session at another AgentDefinition in place.
    ///
    /// Key difference from the generic [`update_session`]: this DOES rewrite
    /// `agent_definition_id` and `project_id` — reinstantiation is the single
    /// controlled exception to those write-once-at-create fields (switching
    /// the Agent on a message-less session is equivalent to recreating it from
    /// the new definition). Like `update_session`, `message_count` /
    /// `last_message_at` stay deliberately omitted; `append_message` remains
    /// their sole writer.
    pub async fn reinstantiate_session(&self, session: &AgentSession) -> Result<(), AppError> {
        let enabled_tools_json = serde_json::to_string(&session.enabled_tools)
            .map_err(|e| AppError::validation_error(&format!("Invalid enabled tools: {}", e)))?;
        let mcp_servers_json = serde_json::to_string(&session.mcp_servers)
            .map_err(|e| AppError::validation_error(&format!("Invalid mcp servers: {}", e)))?;

        let query = r#"
            UPDATE agent_sessions SET agent_definition_id = $1, name = $2, model_id = $3, provider_id = $4, system_prompt = $5, thinking_level = $6, temperature = $7, max_tokens = $8, project_id = $9, working_dir = $10, enabled_tools = $11, mcp_servers = $12, tool_execution_mode = $13, updated_at = $14
            WHERE id = $15
        "#;

        let result = sqlx::query(query)
            .bind(&session.agent_definition_id)
            .bind(&session.name)
            .bind(&session.model_id)
            .bind(&session.provider_id)
            .bind(&session.system_prompt)
            .bind(&session.thinking_level)
            .bind(session.temperature)
            .bind(session.max_tokens)
            .bind(&session.project_id)
            .bind(&session.working_dir)
            .bind(&enabled_tools_json)
            .bind(&mcp_servers_json)
            .bind(&session.tool_execution_mode)
            .bind(session.updated_at)
            .bind(&session.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to reinstantiate agent session: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session.id
            )));
        }

        Ok(())
    }

    /// Renaming also bumps `updated_at`.
    pub async fn rename_session(&self, session_id: &UUID, name: &str) -> Result<(), AppError> {
        let now = Self::now_ms();

        let result =
            sqlx::query("UPDATE agent_sessions SET name = $1, updated_at = $2 WHERE id = $3")
                .bind(name)
                .bind(now)
                .bind(session_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to rename agent session: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            )));
        }

        Ok(())
    }

    /// Sets the sidebar pin flag.
    ///
    /// A single-column write rather than a read-modify-write through
    /// [`update_session`]: pinning is triggered from a hover control that may fire
    /// while a run streams, and only the flag may change. `updated_at` is bumped
    /// like [`rename_session`] does; sidebar order is driven by the activity key
    /// (`last_message_at` / `created_at`), so this cannot reorder anything by itself.
    pub async fn set_session_pinned(
        &self,
        session_id: &UUID,
        pinned: bool,
    ) -> Result<(), AppError> {
        let result =
            sqlx::query("UPDATE agent_sessions SET pinned = $1, updated_at = $2 WHERE id = $3")
                .bind(pinned)
                .bind(Self::now_ms())
                .bind(session_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to pin agent session: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            )));
        }

        Ok(())
    }

    /// Sets the archive flag. Same single-column discipline as
    /// [`set_session_pinned`]; the row and its transcript are left intact, so
    /// unarchiving restores the session exactly as it was.
    pub async fn set_session_archived(
        &self,
        session_id: &UUID,
        archived: bool,
    ) -> Result<(), AppError> {
        let result =
            sqlx::query("UPDATE agent_sessions SET archived = $1, updated_at = $2 WHERE id = $3")
                .bind(archived)
                .bind(Self::now_ms())
                .bind(session_id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to archive agent session: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            )));
        }

        Ok(())
    }

    /// Deletes a session and its transcript rows in one transaction.
    ///
    /// The transcript is deleted explicitly rather than through
    /// `ON DELETE CASCADE`, so no orphan rows survive regardless of the
    /// connection's `PRAGMA foreign_keys` state.
    ///
    /// The legacy `agent_session_messages` table is absent once transcripts live in
    /// JSONL, so its existence is probed via `sqlite_master` inside the transaction
    /// (same schema view as the DELETE) and the DELETE skipped when it is gone —
    /// "no such table" would otherwise fail the whole session delete. The
    /// `<id>.jsonl` file is removed by the command layer.
    pub async fn delete_session(&self, session_id: &UUID) -> Result<(), AppError> {
        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to begin transaction: {}", e))
        })?;

        if legacy_transcript_table_exists(&mut tx).await? {
            sqlx::query("DELETE FROM agent_session_messages WHERE session_id = $1")
                .bind(session_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!(
                        "Failed to delete agent session messages: {}",
                        e
                    ))
                })?;
        }

        let result = sqlx::query("DELETE FROM agent_sessions WHERE id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to delete agent session: {}", e))
            })?;

        if result.rows_affected() == 0 {
            // Unknown session: roll back so the transcript delete never lands.
            tx.rollback().await.map_err(|e| {
                AppError::internal_error(&format!("Failed to rollback transaction: {}", e))
            })?;
            return Err(AppError::not_found(&format!(
                "Agent session not found: {}",
                session_id
            )));
        }

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(())
    }

    /// Appends a message and updates the session counters in one transaction, so
    /// `message_count` / `last_message_at` never drift from the transcript. `seq` is
    /// allocated per session as gap-free and monotonic, starting at 0.
    pub async fn append_message(
        &self,
        session_id: &UUID,
        role: &str,
        payload: &serde_json::Value,
        created_at: Timestamp,
    ) -> Result<AgentSessionMessage, AppError> {
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| AppError::validation_error(&format!("Invalid payload: {}", e)))?;

        let mut tx = self.db.pool().begin().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to begin transaction: {}", e))
        })?;

        let seq_row = sqlx::query(
            "SELECT COALESCE(MAX(seq), -1) + 1 AS next_seq FROM agent_session_messages WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to compute next seq: {}", e)))?;

        let seq: i64 = seq_row.try_get("next_seq")?;

        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO agent_session_messages (id, session_id, seq, role, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(&id)
        .bind(session_id)
        .bind(seq)
        .bind(role)
        .bind(&payload_json)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to append agent session message: {}", e))
        })?;

        sqlx::query(
            r#"
            UPDATE agent_sessions
            SET message_count = message_count + 1, last_message_at = $1, updated_at = $1
            WHERE id = $2
        "#,
        )
        .bind(created_at)
        .bind(session_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to update agent session counters: {}", e))
        })?;

        tx.commit().await.map_err(|e| {
            AppError::internal_error(&format!("Failed to commit transaction: {}", e))
        })?;

        Ok(AgentSessionMessage {
            id,
            session_id: session_id.clone(),
            seq,
            role: role.to_string(),
            payload: payload.clone(),
            created_at,
        })
    }

    /// Full transcript of a session in `seq` order.
    ///
    /// This is the SQLite fallback of the transcript read path, reached only for
    /// legacy sessions without a JSONL transcript. On databases where
    /// `agent_session_messages` has already been dropped, a missing table is the
    /// correct post-state (such a session has no messages), so it maps to an empty
    /// transcript instead of bubbling up as INTERNAL_ERROR and blanking the whole
    /// timeline. Every other DB failure still bubbles.
    pub async fn list_messages(
        &self,
        session_id: &UUID,
    ) -> Result<Vec<AgentSessionMessage>, AppError> {
        let query = r#"
            SELECT id, session_id, seq, role, payload, created_at
            FROM agent_session_messages WHERE session_id = $1 ORDER BY seq ASC
        "#;

        let rows = match sqlx::query(query)
            .bind(session_id)
            .fetch_all(self.db.pool())
            .await
        {
            Ok(rows) => rows,
            // Only a missing `agent_session_messages` table is swallowed.
            Err(e) if is_missing_legacy_table(&e) => return Ok(Vec::new()),
            Err(e) => {
                return Err(AppError::internal_error(&format!(
                    "Failed to list agent session messages: {}",
                    e
                )))
            }
        };

        // Payload parsing is isolated per row: one corrupt payload is logged and
        // skipped instead of failing the whole transcript load.
        let mut messages = Vec::new();
        for row in rows {
            match Self::row_to_message(row) {
                Ok(Some(message)) => messages.push(message),
                Ok(None) => {}           // corrupt payload: already logged
                Err(e) => return Err(e), // real column-read failure
            }
        }

        Ok(messages)
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    // Nullable columns must be read as `try_get::<Option<T>, _>(...)?`, never as
    // `try_get::<T, _>(...).ok()`: sqlx-sqlite decodes SQL NULL into a non-Option
    // type as `Ok(0)` / `Ok("")` rather than an error (see
    // `test_sqlx_sqlite_null_decode_footgun_probe`), so the `.ok()` idiom turns NULL
    // into `Some(0)` / `Some("")` and puts 0/"" on the wire instead of null — which
    // renders a messageless session as a 1970 `last_message_at`.
    fn row_to_session(&self, row: sqlx::sqlite::SqliteRow) -> Result<AgentSession, AppError> {
        let enabled_tools_json: Option<String> = row.try_get("enabled_tools")?;
        let enabled_tools: Vec<String> = if let Some(json) = enabled_tools_json {
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mcp_servers_json: Option<String> = row.try_get("mcp_servers")?;
        let mcp_servers: Vec<crate::storage::types::McpServerConfig> =
            if let Some(json) = mcp_servers_json {
                serde_json::from_str(&json).unwrap_or_default()
            } else {
                Vec::new()
            };

        let temperature: Option<f32> = row.try_get::<Option<f32>, _>("temperature")?;
        let max_tokens: Option<i32> = row.try_get::<Option<i32>, _>("max_tokens")?;

        Ok(AgentSession {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            project_id: row.try_get::<Option<String>, _>("project_id")?,
            agent_definition_id: row.try_get::<Option<String>, _>("agent_definition_id")?,
            model_id: row.try_get::<Option<String>, _>("model_id")?,
            provider_id: row.try_get::<Option<String>, _>("provider_id")?,
            system_prompt: row.try_get::<Option<String>, _>("system_prompt")?,
            thinking_level: row.try_get::<Option<String>, _>("thinking_level")?,
            temperature,
            max_tokens,
            working_dir: row.try_get::<Option<String>, _>("working_dir")?,
            enabled_tools,
            mcp_servers,
            tool_execution_mode: row.try_get::<Option<String>, _>("tool_execution_mode")?,
            message_count: row.try_get("message_count")?,
            last_message_at: row.try_get::<Option<i64>, _>("last_message_at")?,
            // NOT NULL DEFAULT 0 columns, so a plain `bool` decode is safe here —
            // there is no NULL for the footgun above to turn into `false`.
            pinned: row.try_get("pinned")?,
            archived: row.try_get("archived")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    // Two failure kinds are kept apart: a column-read error bubbles as `Err`, while
    // an unparsable stored payload returns `Ok(None)` so `list_messages` can skip
    // just that row.
    fn row_to_message(
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<Option<AgentSessionMessage>, AppError> {
        let id: UUID = row.try_get("id")?;
        let session_id: UUID = row.try_get("session_id")?;
        let seq: i64 = row.try_get("seq")?;
        let role: String = row.try_get("role")?;
        let created_at: Timestamp = row.try_get("created_at")?;
        let payload_json: String = row.try_get("payload")?;

        let payload: serde_json::Value = match serde_json::from_str(&payload_json) {
            Ok(value) => value,
            Err(e) => {
                eprintln!(
                    "Skipping corrupt agent transcript row (session_id={}, seq={}): {}",
                    session_id, seq, e
                );
                return Ok(None);
            }
        };

        Ok(Some(AgentSessionMessage {
            id,
            session_id,
            seq,
            role,
            payload,
            created_at,
        }))
    }
}

/// Whether the legacy `agent_session_messages` transcript table still exists in
/// the given transaction's connection.
///
/// The table is dropped once every transcript lives in JSONL, so the cascading
/// transcript DELETEs in `delete_session` / `delete_project` probe first and issue
/// the DELETE only when the table is present — otherwise a "no such table" error
/// would fail the whole delete. Read from `sqlite_master` (the schema catalog)
/// inside the same transaction, so it sees the exact schema the DELETE would.
///
/// `pub(crate)` so `agent_project_repository::delete_project` reuses the same
/// probe rather than duplicating the gating logic.
pub(crate) async fn legacy_transcript_table_exists(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<bool, AppError> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS c FROM sqlite_master \
         WHERE type = 'table' AND name = 'agent_session_messages'",
    )
    .fetch_one(&mut **tx)
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

/// Whether a sqlx error is SQLite's "no such table: agent_session_messages",
/// which happens once the legacy transcript table has been dropped.
///
/// Matched on the database error text rather than a code: sqlx-sqlite surfaces
/// SQLITE_ERROR (code 1) for a missing table with the table name only in the
/// message, so the table name is the discriminating signal. Scoped to the
/// `agent_session_messages` table by name so an unrelated "no such table" (a
/// real schema bug) still bubbles up rather than being silently swallowed.
fn is_missing_legacy_table(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Database(db_err)
    if {
        let msg = db_err.message();
        msg.contains("no such table") && msg.contains("agent_session_messages")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::tempdir;

    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_service = Database::new(&db_path).await.unwrap();
        (db_service, temp_dir)
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn sample_session(id: &str, name: &str, now: i64) -> AgentSession {
        AgentSession {
            id: id.to_string(),
            name: name.to_string(),
            project_id: None,
            agent_definition_id: None,
            model_id: Some("gpt-4o".to_string()),
            provider_id: Some("openai".to_string()),
            system_prompt: Some("You are a coding agent.".to_string()),
            thinking_level: Some("high".to_string()),
            temperature: Some(0.7),
            max_tokens: Some(2048),
            working_dir: Some("/tmp/project".to_string()),
            enabled_tools: vec!["read".to_string(), "write".to_string()],
            mcp_servers: Vec::new(),
            tool_execution_mode: Some("auto".to_string()),
            message_count: 0,
            last_message_at: None,
            pinned: false,
            archived: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Transcript row count for a session, used to assert no orphans remain.
    async fn count_messages(db: &Database, session_id: &str) -> i64 {
        let row = sqlx::query(
            "SELECT COUNT(*) AS count FROM agent_session_messages WHERE session_id = $1",
        )
        .bind(session_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        row.try_get::<i64, _>("count").unwrap()
    }

    /// Empirical probe documenting the sqlx-sqlite NULL-decode footgun: decoding
    /// SQL NULL into a NON-Option type does NOT error — it yields `Ok(0)` for i64
    /// and `Ok("")` for String. The
    /// `try_get(...).ok()` idiom therefore silently turns NULL into `Some(0)` /
    /// `Some("")` instead of `None`. This is why `row_to_session` must read
    /// nullable columns with an explicit `try_get::<Option<T>, _>(...)?`.
    #[tokio::test]
    async fn test_sqlx_sqlite_null_decode_footgun_probe() {
        let (db, _temp_dir) = create_test_db().await;
        let now = now_ms();

        // Insert a row whose nullable columns are all SQL NULL.
        sqlx::query(
            r#"
            INSERT INTO agent_sessions (id, name, message_count, created_at, updated_at)
            VALUES ('probe', 'Probe', 0, $1, $1)
        "#,
        )
        .bind(now)
        .execute(db.pool())
        .await
        .unwrap();

        let row =
            sqlx::query("SELECT model_id, last_message_at FROM agent_sessions WHERE id = 'probe'")
                .fetch_one(db.pool())
                .await
                .unwrap();

        // The footgun: non-Option decode of NULL succeeds with a zero value.
        assert_eq!(
            row.try_get::<i64, _>("last_message_at").ok(),
            Some(0),
            "sqlx-sqlite decodes NULL INTEGER into i64 as Ok(0), not Err"
        );
        assert_eq!(
            row.try_get::<String, _>("model_id").ok(),
            Some(String::new()),
            "sqlx-sqlite decodes NULL TEXT into String as Ok(\"\"), not Err"
        );

        // The correct idiom: Option<T> decode maps NULL to None.
        assert_eq!(
            row.try_get::<Option<i64>, _>("last_message_at").unwrap(),
            None
        );
        assert_eq!(row.try_get::<Option<String>, _>("model_id").unwrap(), None);
    }

    /// A messageless session whose optional columns are all NULL must round-trip
    /// create→get→list as `None` — not `Some(0)` for last_message_at (which renders
    /// as a 1970 timestamp and sinks the session to the bottom of the list) and not
    /// `Some("")` for the TEXT fields.
    #[tokio::test]
    async fn test_messageless_session_null_columns_round_trip_as_none() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let session = AgentSession {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Fresh Session".to_string(),
            project_id: None,
            agent_definition_id: None,
            model_id: None,
            provider_id: None,
            system_prompt: None,
            thinking_level: None,
            temperature: None,
            max_tokens: None,
            working_dir: None,
            enabled_tools: Vec::new(),
            mcp_servers: Vec::new(),
            tool_execution_mode: None,
            message_count: 0,
            last_message_at: None,
            pinned: false,
            archived: false,
            created_at: now,
            updated_at: now,
        };
        repo.create_session(&session).await.unwrap();

        let assert_all_none = |s: &AgentSession| {
            assert_eq!(s.last_message_at, None, "NULL must not become Some(0)");
            assert_eq!(s.project_id, None);
            assert_eq!(s.agent_definition_id, None, "NULL must not become Some(\"\")");
            assert_eq!(s.model_id, None);
            assert_eq!(s.provider_id, None);
            assert_eq!(s.system_prompt, None);
            assert_eq!(s.thinking_level, None);
            assert_eq!(s.working_dir, None);
            assert_eq!(s.tool_execution_mode, None);
        };

        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_all_none(&fetched);

        let listed = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_all_none(&listed[0]);

        // Once a message lands, last_message_at becomes a real timestamp.
        repo.append_message(&session.id, "user", &serde_json::json!({ "x": 1 }), now + 5)
            .await
            .unwrap();
        let after = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(after.last_message_at, Some(now + 5));
    }

    /// P3: a session instantiated from a definition carries its
    /// `agent_definition_id` back-link through create→get→list verbatim, and the
    /// generic `update_session` path never rewrites it (write-once provenance,
    /// same discipline as `project_id`).
    #[tokio::test]
    async fn test_agent_definition_id_round_trips_and_is_write_once() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let mut session = sample_session(&uuid::Uuid::new_v4().to_string(), "From Coding", now);
        session.agent_definition_id = Some("builtin-coding".to_string());
        repo.create_session(&session).await.unwrap();

        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(
            fetched.agent_definition_id,
            Some("builtin-coding".to_string())
        );
        let listed = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].agent_definition_id,
            Some("builtin-coding".to_string())
        );

        // A field edit (even one that tries to null the link) must not rewrite it.
        let mut edited = fetched;
        edited.agent_definition_id = None;
        edited.name = "renamed".to_string();
        repo.update_session(&edited).await.unwrap();
        let after = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(after.name, "renamed");
        assert_eq!(
            after.agent_definition_id,
            Some("builtin-coding".to_string()),
            "agent_definition_id is write-once and must survive a generic update"
        );
    }

    /// The sidebar flags default to false, round-trip through the dedicated
    /// setters, and — like `project_id` — are NOT rewritable through the generic
    /// `update_session` path, so a field edit built from a pre-toggle snapshot
    /// cannot revert a pin/archive.
    #[tokio::test]
    async fn test_pinned_archived_default_false_and_survive_generic_update() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Flagged", now);
        repo.create_session(&session).await.unwrap();

        // A legacy row inserted without the columns also reads back as false.
        sqlx::query(
            "INSERT INTO agent_sessions (id, name, message_count, created_at, updated_at) \
             VALUES ('legacy-flags', 'Legacy', 0, $1, $1)",
        )
        .bind(now)
        .execute(repo.db.pool())
        .await
        .unwrap();
        let legacy = repo
            .get_session_by_id(&"legacy-flags".to_string())
            .await
            .unwrap()
            .unwrap();
        assert!(!legacy.pinned);
        assert!(!legacy.archived);

        // Capture the pre-toggle snapshot the generic update path would write back.
        let stale = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert!(!stale.pinned);
        assert!(!stale.archived);

        repo.set_session_pinned(&session.id, true).await.unwrap();
        repo.set_session_archived(&session.id, true).await.unwrap();

        let toggled = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert!(toggled.pinned);
        assert!(toggled.archived);
        let listed = repo.list_sessions(10, 0).await.unwrap();
        let listed = listed.iter().find(|s| s.id == session.id).unwrap();
        assert!(listed.pinned, "list must carry the flags too");
        assert!(listed.archived);

        // The stale-snapshot write-back leaves both flags alone.
        let mut edit = stale;
        edit.name = "Renamed".to_string();
        edit.updated_at = now + 1000;
        repo.update_session(&edit).await.unwrap();

        let reloaded = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(reloaded.name, "Renamed");
        assert!(reloaded.pinned, "update_session must not clear pinned");
        assert!(reloaded.archived, "update_session must not clear archived");

        // Toggling back off works, and an unknown id is a clean NotFound.
        repo.set_session_pinned(&session.id, false).await.unwrap();
        repo.set_session_archived(&session.id, false).await.unwrap();
        let cleared = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert!(!cleared.pinned);
        assert!(!cleared.archived);

        assert_eq!(
            repo.set_session_pinned(&"nope".to_string(), true)
                .await
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
        assert_eq!(
            repo.set_session_archived(&"nope".to_string(), true)
                .await
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
    }

    #[tokio::test]
    async fn test_agent_session_crud_roundtrip() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Coding Session", now);

        // Create
        repo.create_session(&session).await.unwrap();

        // Get by ID
        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, session.name);
        assert_eq!(fetched.model_id, session.model_id);
        assert_eq!(fetched.enabled_tools, session.enabled_tools);
        assert_eq!(fetched.thinking_level, session.thinking_level);
        assert_eq!(fetched.message_count, 0);

        // List
        let sessions = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(sessions.len(), 1);

        // Update
        let mut updated = session.clone();
        updated.name = "Renamed Session".to_string();
        updated.enabled_tools = vec!["read".to_string()];
        updated.updated_at = now + 1000;
        repo.update_session(&updated).await.unwrap();

        let fetched_updated = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched_updated.name, "Renamed Session");
        assert_eq!(fetched_updated.enabled_tools, vec!["read".to_string()]);

        // Rename
        repo.rename_session(&session.id, "Final Name")
            .await
            .unwrap();
        let after_rename = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(after_rename.name, "Final Name");

        // Delete
        repo.delete_session(&session.id).await.unwrap();
        assert!(repo.get_session_by_id(&session.id).await.unwrap().is_none());
    }

    /// New sessions never write the deactivated enabled_skills column — it stays
    /// SQL NULL after create AND after a full-row update_session.
    #[tokio::test]
    async fn test_new_sessions_leave_enabled_skills_column_null() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "No Skills Column", now);
        repo.create_session(&session).await.unwrap();

        async fn column_value(repo: &AgentSessionRepository, id: &str) -> Option<String> {
            sqlx::query("SELECT enabled_skills FROM agent_sessions WHERE id = $1")
                .bind(id)
                .fetch_one(repo.db.pool())
                .await
                .unwrap()
                .try_get("enabled_skills")
                .unwrap()
        }

        assert_eq!(
            column_value(&repo, &session.id).await,
            None,
            "create must leave enabled_skills NULL"
        );

        // A full-row update no longer touches the column either.
        let mut edit = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        edit.name = "Renamed".to_string();
        edit.updated_at = now + 1000;
        repo.update_session(&edit).await.unwrap();

        assert_eq!(
            column_value(&repo, &session.id).await,
            None,
            "update_session must leave enabled_skills NULL"
        );
    }

    /// The deactivated column is kept in the schema (never dropped by a migration),
    /// so PRAGMA table_info still lists it.
    #[tokio::test]
    async fn test_enabled_skills_column_still_in_schema() {
        let (db, _temp_dir) = create_test_db().await;

        let rows = sqlx::query("PRAGMA table_info(agent_sessions)")
            .fetch_all(db.pool())
            .await
            .unwrap();
        let columns: Vec<String> = rows
            .iter()
            .map(|row| row.try_get::<String, _>("name").unwrap())
            .collect();
        assert!(
            columns.contains(&"enabled_skills".to_string()),
            "enabled_skills column must remain in the schema, got: {columns:?}"
        );
    }

    /// Legacy rows whose enabled_skills column holds a real value, NULL, or non-JSON
    /// garbage all load via get AND list without error — the column is never parsed.
    #[tokio::test]
    async fn test_legacy_enabled_skills_column_values_load_without_error() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        for (id, column_value) in [
            ("legacy-value", Some(r#"["only-foo"]"#)),
            ("legacy-null", None),
            ("legacy-garbage", Some("{not valid json")),
        ] {
            sqlx::query(
                r#"
                INSERT INTO agent_sessions
                    (id, name, enabled_skills, message_count, created_at, updated_at)
                VALUES ($1, $2, $3, 0, $4, $4)
            "#,
            )
            .bind(id)
            .bind(format!("Legacy {id}"))
            .bind(column_value)
            .bind(now)
            .execute(repo.db.pool())
            .await
            .unwrap();

            let fetched = repo
                .get_session_by_id(&id.to_string())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(fetched.id, id, "row {id} must load via get");
        }

        let listed = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(listed.len(), 3, "all legacy rows must load via list");
    }

    /// The deactivated column is no longer in the UPDATE SET clause: a
    /// full-row update_session leaves a legacy column value byte-for-byte
    /// untouched on disk (and errors never surface from it).
    #[tokio::test]
    async fn test_update_session_leaves_legacy_enabled_skills_column_untouched() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Legacy Holder", now);
        repo.create_session(&session).await.unwrap();

        // Plant a legacy value directly (the struct field is gone).
        sqlx::query("UPDATE agent_sessions SET enabled_skills = $1 WHERE id = $2")
            .bind(r#"["legacy"]"#)
            .bind(&session.id)
            .execute(repo.db.pool())
            .await
            .unwrap();

        let mut edit = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        edit.name = "Renamed Only".to_string();
        edit.updated_at = now + 1000;
        repo.update_session(&edit).await.unwrap();

        let reloaded = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(reloaded.name, "Renamed Only");

        let column: Option<String> =
            sqlx::query("SELECT enabled_skills FROM agent_sessions WHERE id = $1")
                .bind(&session.id)
                .fetch_one(repo.db.pool())
                .await
                .unwrap()
                .try_get("enabled_skills")
                .unwrap();
        assert_eq!(
            column,
            Some(r#"["legacy"]"#.to_string()),
            "update_session must not rewrite the deactivated column"
        );
    }

    /// A legacy row carrying no enabled_skills value loads with every other column
    /// unchanged; simulated by an INSERT that omits the column entirely.
    #[tokio::test]
    async fn test_legacy_row_without_enabled_skills_loads_and_preserves_other_columns() {
        let (db, _temp_dir) = create_test_db().await;
        let repo = AgentSessionRepository::new(Arc::new(db));
        let now = now_ms();

        sqlx::query(
            r#"
            INSERT INTO agent_sessions
                (id, name, model_id, enabled_tools, message_count, created_at, updated_at)
            VALUES ('legacy', 'Legacy Session', 'gpt-4o', '["read"]', 7, $1, $1)
        "#,
        )
        .bind(now)
        .execute(repo.db.pool())
        .await
        .unwrap();

        let fetched = repo
            .get_session_by_id(&"legacy".to_string())
            .await
            .unwrap()
            .unwrap();
        // Other columns survive the migration untouched.
        assert_eq!(fetched.name, "Legacy Session");
        assert_eq!(fetched.model_id, Some("gpt-4o".to_string()));
        assert_eq!(fetched.enabled_tools, vec!["read".to_string()]);
        assert_eq!(fetched.message_count, 7);
    }

    /// A session-field edit (rename / thinking-level change) via the
    /// read-modify-write `update_session` path must NOT touch `message_count` /
    /// `last_message_at`. Those columns are owned exclusively by `append_message`.
    /// Regression guard for the lost-update race: a stale in-memory
    /// `AgentSession` (with its original counters) written back mid-run must not
    /// revert the increments performed by concurrent appends.
    #[tokio::test]
    async fn test_update_session_does_not_clobber_message_count() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Counter Session", now);
        repo.create_session(&session).await.unwrap();

        // Capture the original (stale) snapshot BEFORE any appends. This mirrors a
        // client holding an AgentInput it read at session-open time.
        let stale = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(stale.message_count, 0);
        let stale_last_message_at = stale.last_message_at;

        // A run streams in: 3 messages get appended, advancing the counters.
        for i in 0..3 {
            repo.append_message(
                &session.id,
                "user",
                &serde_json::json!({ "i": i }),
                now + 10 + i,
            )
            .await
            .unwrap();
        }

        let after_appends = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(after_appends.message_count, 3);
        assert_eq!(after_appends.last_message_at, Some(now + 12));
        // Sanity: the stale snapshot really does differ from the live counters, so
        // a clobbering write-back would be observable as a regression.
        assert_ne!(stale.message_count, after_appends.message_count);
        assert_ne!(stale_last_message_at, after_appends.last_message_at);

        // Now the user edits a field (rename + thinking-level change) using the
        // STALE snapshot — the read-modify-write this guard is about.
        let mut edit = stale.clone();
        edit.name = "Renamed Mid-Run".to_string();
        edit.thinking_level = Some("low".to_string());
        edit.updated_at = now + 20;
        repo.update_session(&edit).await.unwrap();

        // The edit applied, but the counters set by append_message are intact:
        // they were NOT reverted to the stale snapshot's values.
        let reloaded = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(reloaded.name, "Renamed Mid-Run");
        assert_eq!(reloaded.thinking_level, Some("low".to_string()));
        assert_eq!(
            reloaded.message_count, 3,
            "update_session must not revert message_count"
        );
        assert_eq!(
            reloaded.last_message_at,
            Some(now + 12),
            "update_session must not revert last_message_at"
        );
    }

    /// `project_id` persists on create, round-trips through get/list, and is
    /// NOT rewritable through `update_session` (write-once at create).
    #[tokio::test]
    async fn test_project_id_persists_and_is_immutable_via_update() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        // Seed a real agent_projects row to satisfy the FK on project_id.
        let project_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO agent_projects (id, path, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&project_id)
        .bind("/tmp/project")
        .bind("project")
        .bind(now)
        .bind(now)
        .execute(db_arc.pool())
        .await
        .unwrap();

        let mut session = sample_session(&uuid::Uuid::new_v4().to_string(), "Attached", now);
        session.project_id = Some(project_id.clone());
        repo.create_session(&session).await.unwrap();

        // Round-trip via get and list.
        let fetched = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(fetched.project_id, Some(project_id.clone()));
        let listed = repo.list_sessions(10, 0).await.unwrap();
        assert_eq!(listed[0].project_id, Some(project_id.clone()));

        // An update_session carrying a different (even cleared) project_id must
        // NOT rewrite the stored attachment.
        let mut edit = fetched.clone();
        edit.name = "Renamed".to_string();
        edit.project_id = None;
        edit.updated_at = now + 1000;
        repo.update_session(&edit).await.unwrap();

        let reloaded = repo.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(reloaded.name, "Renamed");
        assert_eq!(
            reloaded.project_id,
            Some(project_id),
            "update_session must not rewrite project_id"
        );
    }

    /// delete_session removes the session AND all transcript rows even with
    /// `PRAGMA foreign_keys` OFF (explicit delete, not FK cascade).
    #[tokio::test]
    async fn test_delete_session_explicit_cascade_with_fk_off() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        // Force FK enforcement OFF on this connection so the SQL-level
        // `ON DELETE CASCADE` cannot fire. This proves the explicit
        // repository cascade — not the FK — is what removes the transcript.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(db_arc.pool())
            .await
            .unwrap();
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(db_arc.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(fk, 0, "FK enforcement must be OFF for this test");

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Cascade Session", now);
        repo.create_session(&session).await.unwrap();

        // Append N messages.
        for i in 0..5 {
            repo.append_message(
                &session.id,
                "user",
                &serde_json::json!({ "text": format!("msg {}", i) }),
                now + i,
            )
            .await
            .unwrap();
        }
        assert_eq!(count_messages(db_arc.as_ref(), &session.id).await, 5);

        // Delete the session.
        repo.delete_session(&session.id).await.unwrap();

        // Session gone AND zero orphan transcript rows.
        assert!(repo.get_session_by_id(&session.id).await.unwrap().is_none());
        assert_eq!(
            count_messages(db_arc.as_ref(), &session.id).await,
            0,
            "explicit cascade must leave zero orphan transcript rows"
        );
    }

    /// Deleting session A leaves session B's message_count / last_message_at
    /// exactly unchanged.
    #[tokio::test]
    async fn test_delete_session_sibling_isolation() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session_a = sample_session(&uuid::Uuid::new_v4().to_string(), "Session A", now);
        let session_b = sample_session(&uuid::Uuid::new_v4().to_string(), "Session B", now);
        repo.create_session(&session_a).await.unwrap();
        repo.create_session(&session_b).await.unwrap();

        // A gets 2 messages, B gets 3 (with distinct timestamps).
        for i in 0..2 {
            repo.append_message(
                &session_a.id,
                "user",
                &serde_json::json!({ "i": i }),
                now + i,
            )
            .await
            .unwrap();
        }
        for i in 0..3 {
            repo.append_message(
                &session_b.id,
                "user",
                &serde_json::json!({ "i": i }),
                now + 100 + i,
            )
            .await
            .unwrap();
        }

        let b_before = repo
            .get_session_by_id(&session_b.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b_before.message_count, 3);
        assert_eq!(b_before.last_message_at, Some(now + 102));

        // Delete A.
        repo.delete_session(&session_a.id).await.unwrap();

        // B unchanged in count / last_message_at, and B's transcript intact.
        let b_after = repo
            .get_session_by_id(&session_b.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(b_after.message_count, b_before.message_count);
        assert_eq!(b_after.last_message_at, b_before.last_message_at);
        assert_eq!(count_messages(db_arc.as_ref(), &session_b.id).await, 3);

        // A and its transcript are gone.
        assert!(repo
            .get_session_by_id(&session_a.id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(count_messages(db_arc.as_ref(), &session_a.id).await, 0);
    }

    /// Deleting an already-removed id is a clean NotFound: no panic, no orphan rows,
    /// other sessions unaffected.
    #[tokio::test]
    async fn test_delete_session_double_delete_is_clean() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Doomed", now);
        let bystander = sample_session(&uuid::Uuid::new_v4().to_string(), "Bystander", now);
        repo.create_session(&session).await.unwrap();
        repo.create_session(&bystander).await.unwrap();

        repo.append_message(&session.id, "user", &serde_json::json!({ "x": 1 }), now)
            .await
            .unwrap();
        repo.append_message(&bystander.id, "user", &serde_json::json!({ "y": 1 }), now)
            .await
            .unwrap();

        // First delete succeeds.
        repo.delete_session(&session.id).await.unwrap();

        // Second delete: clean NotFound, no panic.
        let err = repo.delete_session(&session.id).await.unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");

        // No orphan rows for the removed session.
        assert_eq!(count_messages(db_arc.as_ref(), &session.id).await, 0);

        // Bystander untouched.
        let bystander_after = repo
            .get_session_by_id(&bystander.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(bystander_after.message_count, 1);
        assert_eq!(count_messages(db_arc.as_ref(), &bystander.id).await, 1);

        // Deleting a never-existed id is also a clean NotFound.
        let err2 = repo
            .delete_session(&"never-existed".to_string())
            .await
            .unwrap_err();
        assert_eq!(err2.code, "NOT_FOUND");
    }

    /// On a database where the legacy `agent_session_messages` table has been
    /// dropped, `delete_session` must still succeed — removing the `agent_sessions`
    /// row — rather than failing with "no such table" → INTERNAL_ERROR. The
    /// authoritative transcript lives in JSONL there, so the stale table DELETE has
    /// to be a safe no-op.
    #[tokio::test]
    async fn test_delete_session_succeeds_after_legacy_table_dropped() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        // Seed a session WITH several transcript rows while the table still exists.
        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Doomed Post-Drop", now);
        repo.create_session(&session).await.unwrap();
        for i in 0..3 {
            repo.append_message(&session.id, "user", &serde_json::json!({ "i": i }), now + i)
                .await
                .unwrap();
        }
        assert_eq!(count_messages(db_arc.as_ref(), &session.id).await, 3);

        // Model a JSONL-only database: drop the legacy transcript table.
        sqlx::query("DROP TABLE agent_session_messages")
            .execute(db_arc.pool())
            .await
            .unwrap();

        // Delete must succeed — not error on the missing table — and remove the row.
        repo.delete_session(&session.id)
            .await
            .expect("delete_session must tolerate a dropped legacy transcript table");
        assert!(
            repo.get_session_by_id(&session.id).await.unwrap().is_none(),
            "the agent_sessions row must be deleted even after the table was dropped"
        );

        // Deleting a non-existent id post-drop is still a clean NotFound
        // (NotFound semantics preserved, not masked by the missing table).
        let err = repo
            .delete_session(&"never-existed".to_string())
            .await
            .unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
    }

    /// append_message assigns gap-free monotonic seq per session, persisting
    /// across reload; list_messages returns them ordered by seq.
    #[tokio::test]
    async fn test_append_message_seq_monotonic_gap_free() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Seq Session", now);
        repo.create_session(&session).await.unwrap();

        // Append 4 messages; seq should be 0,1,2,3 (gap-free, starting at 0).
        let mut returned_seqs = Vec::new();
        for i in 0..4 {
            let msg = repo
                .append_message(&session.id, "user", &serde_json::json!({ "n": i }), now + i)
                .await
                .unwrap();
            returned_seqs.push(msg.seq);
        }
        assert_eq!(returned_seqs, vec![0, 1, 2, 3]);

        // Reload via a fresh repo over the same DB (persistence across reload).
        let repo2 = AgentSessionRepository::new(db_arc.clone());
        let messages = repo2.list_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), 4);

        // Ordered by seq, gap-free, monotonic.
        let seqs: Vec<i64> = messages.iter().map(|m| m.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2, 3]);

        // Payload round-trips intact, and session counters reflect appends.
        assert_eq!(messages[2].payload, serde_json::json!({ "n": 2 }));
        let reloaded_session = repo2.get_session_by_id(&session.id).await.unwrap().unwrap();
        assert_eq!(reloaded_session.message_count, 4);
        assert_eq!(reloaded_session.last_message_at, Some(now + 3));
    }

    /// A long transcript (>200 messages) loads completely in strict seq order — no
    /// silent truncation / pagination in list_messages.
    #[tokio::test]
    async fn test_list_messages_long_transcript_full_no_truncation() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Long Session", now);
        repo.create_session(&session).await.unwrap();

        let total = 250;
        for i in 0..total {
            repo.append_message(
                &session.id,
                "user",
                &serde_json::json!({ "role": "user", "content": format!("m{}", i) }),
                now + i,
            )
            .await
            .unwrap();
        }

        // Reload via a fresh repo: full count, strictly monotonic gap-free seq.
        let repo2 = AgentSessionRepository::new(db_arc.clone());
        let messages = repo2.list_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), total as usize);

        let seqs: Vec<i64> = messages.iter().map(|m| m.seq).collect();
        let expected: Vec<i64> = (0..total).collect();
        assert_eq!(seqs, expected);
    }

    /// A session whose `project_id` points at a project that no longer exists still
    /// appears in `list_sessions`: the query is a plain `SELECT ... FROM
    /// agent_sessions` with NO join onto `agent_projects`, so a dangling project_id
    /// can never filter the row out. The frontend then buckets it as "ungrouped";
    /// losing the row here would make the session vanish entirely.
    #[tokio::test]
    async fn test_list_sessions_keeps_session_with_dangling_project_id() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        // Insert a session referencing a project id that has no agent_projects
        // row. FK enforcement is turned OFF for this connection so the dangling
        // reference can be planted directly — modelling the state left after a
        // project row was removed out from under its sessions.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(db_arc.pool())
            .await
            .unwrap();
        sqlx::query(
            r#"
            INSERT INTO agent_sessions
                (id, name, project_id, message_count, created_at, updated_at)
            VALUES ('dangling', 'Orphaned Session', 'project-that-was-deleted', 0, $1, $1)
        "#,
        )
        .bind(now)
        .execute(db_arc.pool())
        .await
        .unwrap();

        // A normally-attached session alongside it, to prove the dangling one is
        // not the only row and ordering/listing is otherwise intact.
        let attached = sample_session(&uuid::Uuid::new_v4().to_string(), "Attached", now);
        repo.create_session(&attached).await.unwrap();

        let listed = repo.list_sessions(100, 0).await.unwrap();
        assert_eq!(listed.len(), 2, "both sessions must be listed");
        let dangling = listed
            .iter()
            .find(|s| s.id == "dangling")
            .expect("the session with a dangling project_id must still appear in the list");
        assert_eq!(
            dangling.project_id,
            Some("project-that-was-deleted".to_string()),
            "the dangling project_id is preserved verbatim (not nulled / not filtered)"
        );

        // And get_session_by_id also returns it (no join filter there either).
        let fetched = repo
            .get_session_by_id(&"dangling".to_string())
            .await
            .unwrap()
            .expect("a dangling-project session is still fetchable by id");
        assert_eq!(fetched.id, "dangling");
    }

    /// Once the legacy `agent_session_messages` table is dropped, the SQLite
    /// transcript fallback (`list_messages`, taken only for a session with no JSONL
    /// file) must return an empty Vec rather than erroring with "no such table":
    /// such a session genuinely has no messages.
    #[tokio::test]
    async fn test_list_messages_returns_empty_when_legacy_table_dropped() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());

        // Drop the legacy transcript table to model a JSONL-only database.
        sqlx::query("DROP TABLE agent_session_messages")
            .execute(db_arc.pool())
            .await
            .unwrap();

        // Querying transcript for ANY session must now return empty, not error.
        let messages = repo
            .list_messages(&"any-session".to_string())
            .await
            .expect("a dropped legacy table maps to an empty transcript, not an error");
        assert!(
            messages.is_empty(),
            "a session with no JSONL and no legacy table has no messages"
        );
    }

    /// `is_missing_legacy_table` is scoped to the legacy transcript table by
    /// name: a "no such table" for an UNRELATED table is a real schema bug and
    /// must still surface as an error from `list_messages`, never be swallowed.
    #[tokio::test]
    async fn test_list_messages_surfaces_unrelated_missing_table_errors() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);

        // A genuine missing-table error for a DIFFERENT table is not swallowed.
        let err = sqlx::query("SELECT 1 FROM some_other_missing_table")
            .fetch_all(db_arc.pool())
            .await
            .map(|_| ())
            .expect_err("querying a missing table errors");
        assert!(
            !is_missing_legacy_table(&err),
            "only the legacy agent_session_messages table is treated as droppable"
        );
    }

    /// A row whose stored payload is malformed JSON is skipped on load; the rest of
    /// the transcript still returns instead of the whole batch failing.
    #[tokio::test]
    async fn test_list_messages_skips_corrupt_payload_row() {
        let (db, _temp_dir) = create_test_db().await;
        let db_arc = Arc::new(db);
        let repo = AgentSessionRepository::new(db_arc.clone());
        let now = now_ms();

        let session = sample_session(&uuid::Uuid::new_v4().to_string(), "Corrupt Session", now);
        repo.create_session(&session).await.unwrap();

        // Two well-formed rows (seq 0, 2) bracketing one corrupt row (seq 1).
        repo.append_message(
            &session.id,
            "user",
            &serde_json::json!({ "role": "user", "content": "first" }),
            now,
        )
        .await
        .unwrap();

        // Inject a row whose payload column holds non-JSON text directly.
        sqlx::query(
            r#"
            INSERT INTO agent_session_messages (id, session_id, seq, role, payload, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session.id)
        .bind(1_i64)
        .bind("assistant")
        .bind("{not valid json")
        .bind(now + 1)
        .execute(db_arc.pool())
        .await
        .unwrap();

        repo.append_message(
            &session.id,
            "user",
            &serde_json::json!({ "role": "user", "content": "third" }),
            now + 2,
        )
        .await
        .unwrap();

        // list_messages must NOT error; it returns the two valid rows, skipping
        // the corrupt one, in seq order.
        let messages = repo.list_messages(&session.id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].seq, 0);
        assert_eq!(messages[1].seq, 2);
        assert_eq!(
            messages[0].payload,
            serde_json::json!({ "role": "user", "content": "first" })
        );
        assert_eq!(
            messages[1].payload,
            serde_json::json!({ "role": "user", "content": "third" })
        );
    }
}
