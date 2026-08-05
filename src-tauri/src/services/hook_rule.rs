//! Hook-rule service: a thin layer over [`HookRuleRepository`] that owns the
//! write-time concerns the storage layer deliberately leaves out — timestamps
//! and the event/action pairing check.

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

/// An action only means something on one of the two events. Rejecting the
/// mismatch here — rather than storing it and ignoring it at dispatch — keeps a
/// rule from looking active in the UI while doing nothing at runtime.
fn validate_pairing(event: HookEvent, action: HookAction) -> Result<(), AppError> {
    let ok = match event {
        HookEvent::BeforeToolCall => matches!(
            action,
            HookAction::Deny | HookAction::Ask | HookAction::Allow
        ),
        HookEvent::AfterToolCall => matches!(action, HookAction::Notify),
    };

    if ok {
        return Ok(());
    }

    Err(AppError::validation_error(match event {
        HookEvent::BeforeToolCall => {
            "A before-tool-call rule must deny, ask, or allow — notify only applies after a call"
        }
        HookEvent::AfterToolCall => {
            "An after-tool-call rule can only notify — the call has already run"
        }
    }))
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
        let mut rules = self
            .repository
            .list_enabled_for_event(HookEvent::BeforeToolCall)
            .await?;
        rules.extend(
            self.repository
                .list_enabled_for_event(HookEvent::AfterToolCall)
                .await?,
        );
        Ok(rules)
    }

    /// Every rule, enabled or not, for the settings UI.
    pub async fn list(&self) -> Result<Vec<HookRule>, AppError> {
        self.repository.list().await
    }

    pub async fn create(&self, request: CreateHookRuleRequest) -> Result<HookRule, AppError> {
        validate_pairing(request.event, request.action)?;
        self.repository.create(request, now_ms()).await
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdateHookRuleRequest,
    ) -> Result<HookRule, AppError> {
        // Either side of the pair may be omitted, so validate the resulting
        // combination rather than whichever half was sent.
        if request.event.is_some() || request.action.is_some() {
            let current = self
                .repository
                .get(id)
                .await?
                .ok_or_else(|| AppError::not_found(&format!("Hook rule not found: {}", id)))?;
            validate_pairing(
                request.event.unwrap_or(current.event),
                request.action.unwrap_or(current.action),
            )?;
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
            sort_order: None,
        }
    }

    #[tokio::test]
    async fn notify_is_rejected_before_a_call() {
        let (service, _dir) = service().await;
        let err = service
            .create(request(HookEvent::BeforeToolCall, HookAction::Notify))
            .await
            .expect_err("notify cannot decide a pending call");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn a_decision_action_is_rejected_after_a_call() {
        let (service, _dir) = service().await;
        for action in [HookAction::Deny, HookAction::Ask, HookAction::Allow] {
            let err = service
                .create(request(HookEvent::AfterToolCall, action))
                .await
                .expect_err("a finished call cannot be gated");
            assert_eq!(err.code, "VALIDATION_ERROR");
        }
    }

    #[tokio::test]
    async fn valid_pairings_are_accepted() {
        let (service, _dir) = service().await;
        for action in [HookAction::Deny, HookAction::Ask, HookAction::Allow] {
            service
                .create(request(HookEvent::BeforeToolCall, action))
                .await
                .expect("before-call decisions are valid");
        }
        service
            .create(request(HookEvent::AfterToolCall, HookAction::Notify))
            .await
            .expect("after-call notify is valid");
    }

    /// Changing only the event must be validated against the STORED action, or a
    /// half-update could produce a combination that never fires.
    #[tokio::test]
    async fn an_update_validates_the_resulting_pair() {
        let (service, _dir) = service().await;
        let created = service
            .create(request(HookEvent::BeforeToolCall, HookAction::Deny))
            .await
            .unwrap();

        let err = service
            .update(
                &created.id,
                UpdateHookRuleRequest {
                    event: Some(HookEvent::AfterToolCall),
                    ..Default::default()
                },
            )
            .await
            .expect_err("deny + after-call is not a valid pair");
        assert_eq!(err.code, "VALIDATION_ERROR");

        // Moving both halves together is fine.
        service
            .update(
                &created.id,
                UpdateHookRuleRequest {
                    event: Some(HookEvent::AfterToolCall),
                    action: Some(HookAction::Notify),
                    ..Default::default()
                },
            )
            .await
            .expect("event and action moved together");
    }

    #[tokio::test]
    async fn list_enabled_covers_both_events() {
        let (service, _dir) = service().await;
        service
            .create(request(HookEvent::BeforeToolCall, HookAction::Deny))
            .await
            .unwrap();
        service
            .create(request(HookEvent::AfterToolCall, HookAction::Notify))
            .await
            .unwrap();

        let rules = service.list_enabled().await.unwrap();
        assert_eq!(rules.len(), 2, "the session snapshot spans both events");
    }
}
