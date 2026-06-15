// Scheduled-job IPC commands.
//
// This file currently hosts only the schedule-preview command; later features
// add the job CRUD / execution commands here.

use crate::models::AppError;
use crate::storage::types::Timestamp;
use crate::utils::cron;

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
