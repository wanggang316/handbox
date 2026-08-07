//! Hook-rule service: a thin layer over [`HookRuleRepository`] that owns the
//! write-time concerns the storage layer deliberately leaves out — timestamps
//! and the command-presence check.

use std::sync::Arc;

use crate::models::AppError;
use crate::storage::types::{
    CreateHookRuleRequest, HookAction, HookEvent, HookRule, UpdateHookRuleRequest,
};
use crate::storage::{Database, HookRuleRepository};

#[derive(Clone)]
pub struct HookRuleService {
    repository: Arc<HookRuleRepository>,
}

/// Milliseconds since the epoch; the storage layer takes the timestamp rather
/// than reading the clock so tests can pin it.
fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A `run_command` rule without a command would match and then do nothing —
/// a rule that looks active in the UI while doing nothing at runtime.
fn validate_command(action: HookAction, command: Option<&str>) -> Result<(), AppError> {
    if action == HookAction::RunCommand && command.is_none_or(|c| c.trim().is_empty()) {
        return Err(AppError::validation_error(
            "A run-command rule needs a command to run",
        ));
    }
    Ok(())
}

impl HookRuleService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            repository: Arc::new(HookRuleRepository::new(db)),
        }
    }

    /// Every enabled rule, in evaluation order — the snapshot a session takes
    /// when it is built.
    pub async fn list_enabled(&self) -> Result<Vec<HookRule>, AppError> {
        let mut rules = Vec::new();
        for event in [
            HookEvent::BeforeToolCall,
            HookEvent::AfterToolCall,
            HookEvent::UserPromptSubmit,
        ] {
            rules.extend(self.repository.list_enabled_for_event(event).await?);
        }
        Ok(rules)
    }

    /// Every rule, enabled or not, for the settings UI.
    pub async fn list(&self) -> Result<Vec<HookRule>, AppError> {
        self.repository.list().await
    }

    pub async fn create(&self, request: CreateHookRuleRequest) -> Result<HookRule, AppError> {
        validate_command(request.action, request.command.as_deref())?;
        self.repository.create(request, now_ms()).await
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdateHookRuleRequest,
    ) -> Result<HookRule, AppError> {
        // Either half may be omitted, so validate the resulting combination
        // rather than whichever part was sent.
        if request.action.is_some() || request.command.is_some() {
            let current = self
                .repository
                .get(id)
                .await?
                .ok_or_else(|| AppError::not_found(&format!("Hook rule not found: {}", id)))?;
            let action = request.action.unwrap_or(current.action);
            let command = request
                .command
                .clone()
                .or_else(|| current.command.clone())
                .filter(|c| !c.trim().is_empty());
            validate_command(action, command.as_deref())?;
        }

        self.repository.update(id, request, now_ms()).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        self.repository.delete(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn service() -> (HookRuleService, TempDir) {
        let dir = TempDir::new().unwrap();
        let db = Arc::new(
            Database::new(&dir.path().join("test.db"))
                .await
                .expect("database"),
        );
        (HookRuleService::new(db), dir)
    }

    fn request(event: HookEvent, action: HookAction) -> CreateHookRuleRequest {
        CreateHookRuleRequest {
            name: "rule".to_string(),
            event,
            tool_pattern: "bash".to_string(),
            arg_field: None,
            arg_contains: None,
            action,
            message: None,
            command: None,
            timeout_ms: None,
            sort_order: None,
        }
    }

    /// Both actions apply on every event — a notify can observe before a call,
    /// after one, or on a prompt — so nothing here should reject a pairing.
    #[tokio::test]
    async fn every_event_accepts_both_actions() {
        let (service, _dir) = service().await;
        for event in [
            HookEvent::BeforeToolCall,
            HookEvent::AfterToolCall,
            HookEvent::UserPromptSubmit,
        ] {
            service
                .create(request(event, HookAction::Notify))
                .await
                .expect("notify is valid everywhere");
            service
                .create(CreateHookRuleRequest {
                    command: Some("true".to_string()),
                    ..request(event, HookAction::RunCommand)
                })
                .await
                .expect("run_command is valid everywhere");
        }
    }

    #[tokio::test]
    async fn a_run_command_rule_without_a_command_is_rejected() {
        let (service, _dir) = service().await;
        let err = service
            .create(request(HookEvent::BeforeToolCall, HookAction::RunCommand))
            .await
            .expect_err("a run-command rule needs a command");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    /// Switching only the action must be validated against the STORED command,
    /// or a half-update could produce a rule that matches and then does nothing.
    #[tokio::test]
    async fn an_update_validates_the_resulting_combination() {
        let (service, _dir) = service().await;
        let created = service
            .create(request(HookEvent::BeforeToolCall, HookAction::Notify))
            .await
            .unwrap();

        let err = service
            .update(
                &created.id,
                UpdateHookRuleRequest {
                    action: Some(HookAction::RunCommand),
                    ..Default::default()
                },
            )
            .await
            .expect_err("run_command without a stored command is invalid");
        assert_eq!(err.code, "VALIDATION_ERROR");

        // Moving both halves together is fine.
        service
            .update(
                &created.id,
                UpdateHookRuleRequest {
                    action: Some(HookAction::RunCommand),
                    command: Some("true".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect("action and command moved together");
    }

    #[tokio::test]
    async fn list_enabled_covers_every_event() {
        let (service, _dir) = service().await;
        service
            .create(request(HookEvent::BeforeToolCall, HookAction::Notify))
            .await
            .unwrap();
        service
            .create(request(HookEvent::AfterToolCall, HookAction::Notify))
            .await
            .unwrap();
        service
            .create(request(HookEvent::UserPromptSubmit, HookAction::Notify))
            .await
            .unwrap();

        let rules = service.list_enabled().await.unwrap();
        assert_eq!(rules.len(), 3, "the session snapshot spans every event");
    }
}
