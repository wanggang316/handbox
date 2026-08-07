// Hook-rule management IPC commands.

use crate::models::AppError;
use crate::services::HookRuleService;
use crate::storage::types::{CreateHookRuleRequest, HookRule, UpdateHookRuleRequest};
use tauri::State;

#[tauri::command]
pub async fn hook_rule_list(
    hook_rule_service: State<'_, HookRuleService>,
) -> Result<Vec<HookRule>, AppError> {
    hook_rule_service.list().await
}

#[tauri::command]
pub async fn hook_rule_create(
    request: CreateHookRuleRequest,
    hook_rule_service: State<'_, HookRuleService>,
) -> Result<HookRule, AppError> {
    hook_rule_service.create(request).await
}

#[tauri::command]
pub async fn hook_rule_update(
    rule_id: String,
    request: UpdateHookRuleRequest,
    hook_rule_service: State<'_, HookRuleService>,
) -> Result<HookRule, AppError> {
    hook_rule_service.update(&rule_id, request).await
}

#[tauri::command]
pub async fn hook_rule_delete(
    rule_id: String,
    hook_rule_service: State<'_, HookRuleService>,
) -> Result<(), AppError> {
    hook_rule_service.delete(&rule_id).await
}
