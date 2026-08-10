// Data access for declarative hook rules (`agent_hook_rules`, migration 063).
//
// The dispatch path calls [`HookRuleRepository::list_enabled_for_event`] on
// every tool call, so that one is index-backed and returns rules already in
// evaluation order; everything else here serves the settings UI.

use crate::models::AppError;
use crate::storage::types::{
    CreateHookRuleRequest, HookAction, HookEvent, HookRule, UpdateHookRuleRequest,
};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Column encoding for `event`. Matches the serde `snake_case` wire format of
/// `hook_rule.rs`, but is spelled out here so the DB string convention stays
/// owned by the data-access layer.
fn event_as_str(event: HookEvent) -> &'static str {
    match event {
        HookEvent::BeforeToolCall => "before_tool_call",
        HookEvent::AfterToolCall => "after_tool_call",
        HookEvent::UserPromptSubmit => "user_prompt_submit",
        HookEvent::TurnEnd => "turn_end",
        HookEvent::ApprovalRequested => "approval_requested",
    }
}

fn event_from_str(value: &str) -> Result<HookEvent, AppError> {
    match value {
        "before_tool_call" => Ok(HookEvent::BeforeToolCall),
        "after_tool_call" => Ok(HookEvent::AfterToolCall),
        "user_prompt_submit" => Ok(HookEvent::UserPromptSubmit),
        "turn_end" => Ok(HookEvent::TurnEnd),
        "approval_requested" => Ok(HookEvent::ApprovalRequested),
        other => Err(AppError::internal_error(&format!(
            "Invalid hook event in database: {}",
            other
        ))),
    }
}

fn action_as_str(action: HookAction) -> &'static str {
    match action {
        HookAction::Notify => "notify",
        HookAction::RunCommand => "run_command",
    }
}

// The removed decision actions ("deny"/"ask"/"allow") are deleted by migration
// 065, so a row carrying one here is corruption, not legacy data.
fn action_from_str(value: &str) -> Result<HookAction, AppError> {
    match value {
        "notify" => Ok(HookAction::Notify),
        "run_command" => Ok(HookAction::RunCommand),
        other => Err(AppError::internal_error(&format!(
            "Invalid hook action in database: {}",
            other
        ))),
    }
}

/// Every column, in a fixed order shared by all reads.
const RULE_COLUMNS: &str = r#"
    SELECT id, name, event, tool_pattern, arg_field, arg_contains,
           action, message, command, timeout_ms, enabled, sort_order,
           created_at, updated_at
    FROM agent_hook_rules
"#;

/// Empty string means "clear the column"; see [`UpdateHookRuleRequest`].
fn blank_to_null(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

pub struct HookRuleRepository {
    db: Arc<Database>,
}

impl HookRuleRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    fn row_to_rule(row: sqlx::sqlite::SqliteRow) -> Result<HookRule, AppError> {
        Ok(HookRule {
            id: row.try_get("id").map_err(decode_err)?,
            name: row.try_get("name").map_err(decode_err)?,
            event: event_from_str(
                row.try_get::<String, _>("event")
                    .map_err(decode_err)?
                    .as_str(),
            )?,
            tool_pattern: row.try_get("tool_pattern").map_err(decode_err)?,
            arg_field: row.try_get("arg_field").map_err(decode_err)?,
            arg_contains: row.try_get("arg_contains").map_err(decode_err)?,
            action: action_from_str(
                row.try_get::<String, _>("action")
                    .map_err(decode_err)?
                    .as_str(),
            )?,
            message: row.try_get("message").map_err(decode_err)?,
            command: row.try_get("command").map_err(decode_err)?,
            timeout_ms: row.try_get("timeout_ms").map_err(decode_err)?,
            enabled: row.try_get::<i64, _>("enabled").map_err(decode_err)? != 0,
            sort_order: row.try_get("sort_order").map_err(decode_err)?,
            created_at: row.try_get("created_at").map_err(decode_err)?,
            updated_at: row.try_get("updated_at").map_err(decode_err)?,
        })
    }

    /// Enabled rules for one event, already in evaluation order.
    ///
    /// This runs on every tool call, so it is deliberately the narrowest query
    /// here and is served by `idx_agent_hook_rules_event`.
    pub async fn list_enabled_for_event(
        &self,
        event: HookEvent,
    ) -> Result<Vec<HookRule>, AppError> {
        let query = format!("{RULE_COLUMNS} WHERE event = $1 AND enabled = 1 ORDER BY sort_order ASC, created_at ASC");
        let rows = sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(event_as_str(event))
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list hook rules: {}", e)))?;

        rows.into_iter().map(Self::row_to_rule).collect()
    }

    /// All rules, enabled or not, for the settings UI.
    pub async fn list(&self) -> Result<Vec<HookRule>, AppError> {
        let query = format!("{RULE_COLUMNS} ORDER BY sort_order ASC, created_at ASC");
        let rows = sqlx::query(sqlx::AssertSqlSafe(query))
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list hook rules: {}", e)))?;

        rows.into_iter().map(Self::row_to_rule).collect()
    }

    pub async fn get(&self, id: &str) -> Result<Option<HookRule>, AppError> {
        let query = format!("{RULE_COLUMNS} WHERE id = $1");
        let row = sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get hook rule: {}", e)))?;

        row.map(Self::row_to_rule).transpose()
    }

    /// Insert a rule. An omitted `sort_order` appends to the end, so a rule
    /// created from the UI never silently preempts the existing ones.
    pub async fn create(
        &self,
        request: CreateHookRuleRequest,
        now: i64,
    ) -> Result<HookRule, AppError> {
        let id = uuid::Uuid::new_v4().to_string();
        let sort_order = match request.sort_order {
            Some(order) => order,
            None => self.next_sort_order().await?,
        };

        let rule = HookRule {
            id,
            name: request.name,
            event: request.event,
            tool_pattern: request.tool_pattern,
            arg_field: blank_to_null(request.arg_field),
            arg_contains: blank_to_null(request.arg_contains),
            action: request.action,
            message: blank_to_null(request.message),
            command: blank_to_null(request.command),
            timeout_ms: request.timeout_ms,
            enabled: true,
            sort_order,
            created_at: now,
            updated_at: now,
        };

        sqlx::query(
            r#"
            INSERT INTO agent_hook_rules
                (id, name, event, tool_pattern, arg_field, arg_contains,
                 action, message, command, timeout_ms, enabled, sort_order,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
        )
        .bind(&rule.id)
        .bind(&rule.name)
        .bind(event_as_str(rule.event))
        .bind(&rule.tool_pattern)
        .bind(&rule.arg_field)
        .bind(&rule.arg_contains)
        .bind(action_as_str(rule.action))
        .bind(&rule.message)
        .bind(&rule.command)
        .bind(rule.timeout_ms)
        .bind(i64::from(rule.enabled))
        .bind(rule.sort_order)
        .bind(rule.created_at)
        .bind(rule.updated_at)
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to create hook rule: {}", e)))?;

        Ok(rule)
    }

    /// Read-modify-write: an omitted field keeps the stored value. Done in Rust
    /// rather than a dynamic SET clause because the update is not hot and the
    /// column list is small.
    pub async fn update(
        &self,
        id: &str,
        request: UpdateHookRuleRequest,
        now: i64,
    ) -> Result<HookRule, AppError> {
        let Some(current) = self.get(id).await? else {
            return Err(AppError::not_found(&format!("Hook rule not found: {}", id)));
        };

        let updated = HookRule {
            name: request.name.unwrap_or(current.name),
            event: request.event.unwrap_or(current.event),
            tool_pattern: request.tool_pattern.unwrap_or(current.tool_pattern),
            arg_field: request
                .arg_field
                .map_or(current.arg_field, |v| blank_to_null(Some(v))),
            arg_contains: request
                .arg_contains
                .map_or(current.arg_contains, |v| blank_to_null(Some(v))),
            action: request.action.unwrap_or(current.action),
            message: request
                .message
                .map_or(current.message, |v| blank_to_null(Some(v))),
            command: request
                .command
                .map_or(current.command, |v| blank_to_null(Some(v))),
            timeout_ms: request.timeout_ms.or(current.timeout_ms),
            enabled: request.enabled.unwrap_or(current.enabled),
            sort_order: request.sort_order.unwrap_or(current.sort_order),
            updated_at: now,
            ..current
        };

        sqlx::query(
            r#"
            UPDATE agent_hook_rules SET
                name = $1, event = $2, tool_pattern = $3, arg_field = $4,
                arg_contains = $5, action = $6, message = $7, command = $8,
                timeout_ms = $9, enabled = $10, sort_order = $11, updated_at = $12
            WHERE id = $13
        "#,
        )
        .bind(&updated.name)
        .bind(event_as_str(updated.event))
        .bind(&updated.tool_pattern)
        .bind(&updated.arg_field)
        .bind(&updated.arg_contains)
        .bind(action_as_str(updated.action))
        .bind(&updated.message)
        .bind(&updated.command)
        .bind(updated.timeout_ms)
        .bind(i64::from(updated.enabled))
        .bind(updated.sort_order)
        .bind(updated.updated_at)
        .bind(id)
        .execute(self.db.pool())
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to update hook rule: {}", e)))?;

        Ok(updated)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM agent_hook_rules WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete hook rule: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Hook rule not found: {}", id)));
        }

        Ok(())
    }

    /// One past the highest stored order, so a new rule lands last.
    async fn next_sort_order(&self) -> Result<i64, AppError> {
        let max: Option<i64> = sqlx::query_scalar("SELECT MAX(sort_order) FROM agent_hook_rules")
            .fetch_one(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to read sort order: {}", e)))?;

        Ok(max.map_or(0, |m| m + 1))
    }
}

fn decode_err(e: sqlx::Error) -> AppError {
    AppError::internal_error(&format!("Failed to decode hook rule row: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;
    use tempfile::TempDir;

    async fn repo() -> (HookRuleRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(
            Database::new(&dir.path().join("test.db"))
                .await
                .expect("database"),
        );
        (HookRuleRepository::new(db), dir)
    }

    fn create_request(name: &str, event: HookEvent, action: HookAction) -> CreateHookRuleRequest {
        CreateHookRuleRequest {
            name: name.to_string(),
            event,
            tool_pattern: "bash".to_string(),
            arg_field: Some("command".to_string()),
            arg_contains: Some("rm -rf".to_string()),
            action,
            message: Some("blocked by policy".to_string()),
            command: None,
            timeout_ms: None,
            sort_order: None,
        }
    }

    #[tokio::test]
    async fn create_then_get_roundtrips_every_field() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(
                create_request("no rm", HookEvent::BeforeToolCall, HookAction::RunCommand),
                1_700_000_000_000,
            )
            .await
            .unwrap();

        let fetched = repo.get(&created.id).await.unwrap().expect("stored rule");
        assert_eq!(fetched, created);
        assert_eq!(fetched.action, HookAction::RunCommand);
        assert_eq!(fetched.event, HookEvent::BeforeToolCall);
        assert!(fetched.enabled, "a new rule is enabled");
    }

    /// Every event value survives the column encoding — the failure this
    /// guards is adding an enum variant without teaching the codec its string.
    #[tokio::test]
    async fn every_event_roundtrips_through_the_column_encoding() {
        let (repo, _dir) = repo().await;
        for event in [
            HookEvent::BeforeToolCall,
            HookEvent::AfterToolCall,
            HookEvent::UserPromptSubmit,
            HookEvent::TurnEnd,
            HookEvent::ApprovalRequested,
        ] {
            let created = repo
                .create(create_request("evt", event, HookAction::Notify), 1)
                .await
                .unwrap();
            let fetched = repo.get(&created.id).await.unwrap().expect("stored rule");
            assert_eq!(fetched.event, event);
        }
    }

    /// Empty optional fields land as NULL rather than as empty strings, so
    /// matching sees "no constraint" instead of "contains the empty string".
    #[tokio::test]
    async fn blank_optional_fields_are_stored_as_null() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(
                CreateHookRuleRequest {
                    arg_field: Some(String::new()),
                    arg_contains: Some(String::new()),
                    message: Some(String::new()),
                    ..create_request("bare", HookEvent::BeforeToolCall, HookAction::Notify)
                },
                1,
            )
            .await
            .unwrap();

        assert_eq!(created.arg_field, None);
        assert_eq!(created.arg_contains, None);
        assert_eq!(created.message, None);
    }

    #[tokio::test]
    async fn list_enabled_for_event_filters_by_event_and_enablement() {
        let (repo, _dir) = repo().await;
        let before = repo
            .create(
                create_request("before", HookEvent::BeforeToolCall, HookAction::RunCommand),
                1,
            )
            .await
            .unwrap();
        repo.create(
            create_request("after", HookEvent::AfterToolCall, HookAction::Notify),
            2,
        )
        .await
        .unwrap();
        let disabled = repo
            .create(
                create_request("off", HookEvent::BeforeToolCall, HookAction::RunCommand),
                3,
            )
            .await
            .unwrap();
        repo.update(
            &disabled.id,
            UpdateHookRuleRequest {
                enabled: Some(false),
                ..Default::default()
            },
            4,
        )
        .await
        .unwrap();

        let enabled = repo
            .list_enabled_for_event(HookEvent::BeforeToolCall)
            .await
            .unwrap();
        assert_eq!(enabled.len(), 1, "other event and disabled rule excluded");
        assert_eq!(enabled[0].id, before.id);
    }

    /// Evaluation order is the contract the dispatch loop relies on: first match
    /// wins, so the query must return `sort_order` ascending.
    #[tokio::test]
    async fn enabled_rules_come_back_in_sort_order() {
        let (repo, _dir) = repo().await;
        for (name, order) in [("third", 30), ("first", 10), ("second", 20)] {
            repo.create(
                CreateHookRuleRequest {
                    sort_order: Some(order),
                    ..create_request(name, HookEvent::BeforeToolCall, HookAction::RunCommand)
                },
                1,
            )
            .await
            .unwrap();
        }

        let rules = repo
            .list_enabled_for_event(HookEvent::BeforeToolCall)
            .await
            .unwrap();
        let names: Vec<&str> = rules.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["first", "second", "third"]);
    }

    /// A rule created without an explicit order appends, so adding one from the
    /// UI never preempts the rules already there.
    #[tokio::test]
    async fn omitted_sort_order_appends() {
        let (repo, _dir) = repo().await;
        repo.create(
            CreateHookRuleRequest {
                sort_order: Some(5),
                ..create_request(
                    "existing",
                    HookEvent::BeforeToolCall,
                    HookAction::RunCommand,
                )
            },
            1,
        )
        .await
        .unwrap();

        let appended = repo
            .create(
                create_request("new", HookEvent::BeforeToolCall, HookAction::RunCommand),
                2,
            )
            .await
            .unwrap();
        assert_eq!(appended.sort_order, 6);
    }

    #[tokio::test]
    async fn update_leaves_omitted_fields_untouched() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(
                create_request("rule", HookEvent::BeforeToolCall, HookAction::RunCommand),
                1,
            )
            .await
            .unwrap();

        let updated = repo
            .update(
                &created.id,
                UpdateHookRuleRequest {
                    name: Some("renamed".to_string()),
                    ..Default::default()
                },
                2,
            )
            .await
            .unwrap();

        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.tool_pattern, created.tool_pattern);
        assert_eq!(updated.arg_contains, created.arg_contains);
        assert_eq!(updated.action, created.action);
        assert_eq!(
            updated.created_at, created.created_at,
            "create time is fixed"
        );
        assert_eq!(updated.updated_at, 2);
    }

    /// The documented escape hatch for a flat `Option`: empty string clears.
    #[tokio::test]
    async fn an_empty_string_clears_a_nullable_column() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(
                create_request("rule", HookEvent::BeforeToolCall, HookAction::RunCommand),
                1,
            )
            .await
            .unwrap();
        assert!(created.arg_contains.is_some(), "precondition");

        let updated = repo
            .update(
                &created.id,
                UpdateHookRuleRequest {
                    arg_contains: Some(String::new()),
                    ..Default::default()
                },
                2,
            )
            .await
            .unwrap();
        assert_eq!(updated.arg_contains, None);
    }

    #[tokio::test]
    async fn delete_removes_the_rule_and_reports_a_missing_one() {
        let (repo, _dir) = repo().await;
        let created = repo
            .create(
                create_request("rule", HookEvent::BeforeToolCall, HookAction::RunCommand),
                1,
            )
            .await
            .unwrap();

        repo.delete(&created.id).await.unwrap();
        assert!(repo.get(&created.id).await.unwrap().is_none());
        assert!(
            repo.delete(&created.id).await.is_err(),
            "deleting a missing rule is an error, not a silent no-op"
        );
    }
}
