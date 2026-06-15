// Scheduled-job IPC commands.
//
// Hosts the schedule-preview command plus the job CRUD commands. Execution /
// manual-trigger (`job_run_now`) commands belong to later features.

use crate::models::AppError;
use crate::services::{JobCreateRequest, JobExecutor, JobService, JobUpdateRequest};
use crate::storage::types::{Job, JobExecution, JobTarget, Timestamp, UUID};
use crate::utils::cron;
use serde::{Deserialize, Serialize};
use tauri::State;

/// Preview a cron schedule: return up to `n` upcoming occurrences (default 5),
/// each strictly after "now" in the system local time zone, as millisecond
/// timestamps in ascending order.
///
/// - Valid cron -> array of occurrences (at most `n`).
/// - Sparse schedules whose visible future yields fewer than `n` occurrences
///   return the real count (possibly empty), not padded entries.
/// - Invalid cron -> structured `VALIDATION_ERROR` `AppError`.
#[tauri::command]
pub async fn job_preview_schedule(
    cron_expr: String,
    n: Option<usize>,
) -> Result<Vec<Timestamp>, AppError> {
    let count = n.unwrap_or(cron::DEFAULT_PREVIEW_COUNT);
    tracing::debug!("Previewing schedule for cron '{}' (n={})", cron_expr, count);
    cron::next_occurrences(&cron_expr, count)
}

/// IPC payload to create a job. Field names are camelCase on the wire to match
/// the frontend; `target` is the internally-tagged `JobTarget`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobCreatePayload {
    pub name: String,
    pub description: Option<String>,
    pub target: JobTarget,
    pub cron_expr: String,
    pub timezone: String,
    pub enabled: Option<bool>,
    /// Per-run timeout in seconds; omit/null to use the named default.
    #[serde(default)]
    pub exec_timeout_secs: Option<i64>,
    /// Max retry attempts; omit/null to use the named default.
    #[serde(default)]
    pub max_retries: Option<i64>,
    /// Delay between retries in seconds; omit/null to use the named default.
    #[serde(default)]
    pub retry_delay_secs: Option<i64>,
}

/// IPC payload to fully replace a job's definition.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobUpdatePayload {
    pub name: String,
    pub description: Option<String>,
    pub target: JobTarget,
    pub cron_expr: String,
    pub timezone: String,
    pub enabled: bool,
    /// Per-run timeout in seconds; omit/null to use the named default.
    #[serde(default)]
    pub exec_timeout_secs: Option<i64>,
    /// Max retry attempts; omit/null to use the named default.
    #[serde(default)]
    pub max_retries: Option<i64>,
    /// Delay between retries in seconds; omit/null to use the named default.
    #[serde(default)]
    pub retry_delay_secs: Option<i64>,
}

/// Create a new scheduled job.
#[tauri::command]
pub async fn job_create(
    request: JobCreatePayload,
    job_service: State<'_, JobService>,
) -> Result<Job, AppError> {
    job_service
        .create(JobCreateRequest {
            name: request.name,
            description: request.description,
            target: request.target,
            cron_expr: request.cron_expr,
            timezone: request.timezone,
            enabled: request.enabled,
            exec_timeout_secs: request.exec_timeout_secs,
            max_retries: request.max_retries,
            retry_delay_secs: request.retry_delay_secs,
        })
        .await
}

/// List jobs (newest-first), paginated.
#[tauri::command]
pub async fn job_list(
    limit: Option<i32>,
    offset: Option<i32>,
    job_service: State<'_, JobService>,
) -> Result<Vec<Job>, AppError> {
    job_service.list(limit, offset).await
}

/// Get a single job by id.
#[tauri::command]
pub async fn job_get(job_id: UUID, job_service: State<'_, JobService>) -> Result<Job, AppError> {
    job_service.get(job_id).await
}

/// Replace a job's definition fields.
#[tauri::command]
pub async fn job_update(
    job_id: UUID,
    request: JobUpdatePayload,
    job_service: State<'_, JobService>,
) -> Result<Job, AppError> {
    job_service
        .update(
            job_id,
            JobUpdateRequest {
                name: request.name,
                description: request.description,
                target: request.target,
                cron_expr: request.cron_expr,
                timezone: request.timezone,
                enabled: request.enabled,
                exec_timeout_secs: request.exec_timeout_secs,
                max_retries: request.max_retries,
                retry_delay_secs: request.retry_delay_secs,
            },
        )
        .await
}

/// Delete a job (its execution history cascades).
#[tauri::command]
pub async fn job_delete(job_id: UUID, job_service: State<'_, JobService>) -> Result<(), AppError> {
    job_service.delete(job_id).await
}

/// Enable or disable a job.
#[tauri::command]
pub async fn job_set_enabled(
    job_id: UUID,
    enabled: bool,
    job_service: State<'_, JobService>,
) -> Result<Job, AppError> {
    job_service.set_enabled(job_id, enabled).await
}

/// List a job's execution history (newest-first), paginated. Includes any
/// in-progress (`running`) row so the detail timeline shows live runs. A job
/// that has never run returns an empty array, not an error.
#[tauri::command]
pub async fn job_execution_list(
    job_id: UUID,
    limit: Option<i32>,
    offset: Option<i32>,
    job_service: State<'_, JobService>,
) -> Result<Vec<JobExecution>, AppError> {
    job_service.list_executions(job_id, limit, offset).await
}

/// Manually run a job NOW (`trigger = manual`), independent of its schedule.
///
/// Loads the job (NOT_FOUND if missing), then dispatches it through the
/// `JobExecutor`, which shares ONE in-flight set with the background scheduler:
/// - A disabled job (`enabled = 0`) still runs — disabling only stops automatic
///   scheduling, never an explicit manual run.
/// - If an execution is already in flight (scheduled OR a prior manual run), the
///   executor returns a `CONFLICT` error and writes NO second row, so a job can
///   never have two concurrent `running` rows.
///
/// Returns the finalized `JobExecution` (the detail timeline reloads its history
/// to surface the new manual row).
#[tauri::command]
pub async fn job_run_now(
    job_id: UUID,
    job_service: State<'_, JobService>,
    job_executor: State<'_, JobExecutor>,
) -> Result<JobExecution, AppError> {
    let job = job_service.get(job_id).await?;
    tracing::info!(job_id = %job.id, "[job_run_now] manual run requested");
    job_executor.run_now(&job).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // The Tauri command is a thin wrapper over `cron::next_occurrences`; we
    // exercise the same logic the IPC layer runs (the command itself only adds
    // the default-`n` and tracing). This covers VAL-SCHED-024: an invalid cron
    // surfaces as a structured `{code, message, hint}` error.

    #[tokio::test]
    async fn preview_invalid_cron_returns_structured_error() {
        let err = job_preview_schedule("not-a-cron".to_string(), None)
            .await
            .expect_err("invalid cron must error");
        assert_eq!(err.code, "VALIDATION_ERROR");
        assert!(!err.message.is_empty());
        assert!(err.hint.is_some());
    }

    #[tokio::test]
    async fn preview_valid_cron_defaults_to_five() {
        let times = job_preview_schedule("* * * * *".to_string(), None)
            .await
            .expect("valid cron");
        assert_eq!(times.len(), 5);
    }

    #[tokio::test]
    async fn preview_valid_cron_respects_explicit_count() {
        let times = job_preview_schedule("* * * * *".to_string(), Some(2))
            .await
            .expect("valid cron");
        assert_eq!(times.len(), 2);
    }
}
