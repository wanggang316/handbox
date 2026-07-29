// Data access for scheduled jobs: `JobRepository` (definitions in `jobs`) and
// `JobExecutionRepository` (run history in `job_executions`). This layer only
// exposes the operations; pruning after a write and reconciling stale `running`
// rows on startup are wired up by the scheduler.

use crate::models::AppError;
use crate::storage::types::{ExecutionStatus, Job, JobExecution, JobTarget, Timestamp, Trigger};
use crate::storage::Database;
use sqlx::Row;
use std::sync::Arc;

/// Default FIFO history cap: how many `job_executions` rows each job keeps.
pub const DEFAULT_EXECUTION_HISTORY_LIMIT: i64 = 100;

/// Column encoding for the `last_status` / `status` columns. Matches the serde
/// `snake_case` wire format of `job.rs`, but is mapped explicitly here so the DB
/// string convention stays owned by the data-access layer.
fn execution_status_as_str(status: ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Running => "running",
        ExecutionStatus::Success => "success",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::Timeout => "timeout",
    }
}

/// Decode the column back into `ExecutionStatus`; an unknown value is treated as
/// data corruption and returned as an error rather than silently swallowed.
fn execution_status_from_str(value: &str) -> Result<ExecutionStatus, AppError> {
    match value {
        "running" => Ok(ExecutionStatus::Running),
        "success" => Ok(ExecutionStatus::Success),
        "failed" => Ok(ExecutionStatus::Failed),
        "timeout" => Ok(ExecutionStatus::Timeout),
        other => Err(AppError::internal_error(&format!(
            "Invalid execution status in database: {}",
            other
        ))),
    }
}

/// How a finished run should update `failure_count`.
///
/// `failure_count` counts *consecutive* failures and resets on success, unlike
/// `run_count`, which only ever grows. Manual triggers never take part in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCountUpdate {
    /// Scheduled failure/timeout: `failure_count + 1`.
    Increment,
    /// Scheduled success: clear `failure_count`, breaking the failure streak.
    Reset,
    /// Leave `failure_count` alone (manual trigger).
    Unchanged,
}

/// Column encoding for the `trigger` column.
fn trigger_as_str(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Schedule => "schedule",
        Trigger::Manual => "manual",
    }
}

fn trigger_from_str(value: &str) -> Result<Trigger, AppError> {
    match value {
        "schedule" => Ok(Trigger::Schedule),
        "manual" => Ok(Trigger::Manual),
        other => Err(AppError::internal_error(&format!(
            "Invalid trigger in database: {}",
            other
        ))),
    }
}

/// Repository for job definitions (`jobs` table).
#[derive(Clone)]
pub struct JobRepository {
    db: Arc<Database>,
}

impl JobRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// `target` is stored split across the `target_kind` + `target_config` columns.
    pub async fn create(&self, job: &Job) -> Result<(), AppError> {
        let (target_kind, target_config) = job
            .target
            .into_db_parts()
            .map_err(|e| AppError::validation_error(&format!("Invalid job target: {}", e)))?;

        let query = r#"
            INSERT INTO jobs (
                id, name, description, target_kind, target_config, cron_expr, timezone,
                enabled, last_run_at, next_run_at, last_status, run_count, failure_count,
                exec_timeout_secs, max_retries, retry_delay_secs,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
        "#;

        sqlx::query(query)
            .bind(&job.id)
            .bind(&job.name)
            .bind(&job.description)
            .bind(&target_kind)
            .bind(&target_config)
            .bind(&job.cron_expr)
            .bind(&job.timezone)
            .bind(job.enabled)
            .bind(job.last_run_at)
            .bind(job.next_run_at)
            .bind(job.last_status.map(execution_status_as_str))
            .bind(job.run_count)
            .bind(job.failure_count)
            .bind(job.exec_timeout_secs)
            .bind(job.max_retries)
            .bind(job.retry_delay_secs)
            .bind(job.created_at)
            .bind(job.updated_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create job: {}", e)))?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<Job>, AppError> {
        let row = sqlx::query(JOB_SELECT_COLUMNS_WITH_WHERE_ID)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to get job: {}", e)))?;

        row.map(Self::row_to_job).transpose()
    }

    /// Paginated, newest first (`created_at` descending).
    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Job>, AppError> {
        let query = format!(
            "{} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            JOB_SELECT_COLUMNS
        );

        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list jobs: {}", e)))?;

        rows.into_iter().map(Self::row_to_job).collect()
    }

    /// Replaces the definition fields only; run statistics stay untouched and are
    /// owned by `update_after_run` / `set_enabled`. Unknown id yields `not_found`.
    pub async fn update(&self, job: &Job) -> Result<(), AppError> {
        let (target_kind, target_config) = job
            .target
            .into_db_parts()
            .map_err(|e| AppError::validation_error(&format!("Invalid job target: {}", e)))?;

        let query = r#"
            UPDATE jobs SET
                name = $1, description = $2, target_kind = $3, target_config = $4,
                cron_expr = $5, timezone = $6, enabled = $7, next_run_at = $8,
                exec_timeout_secs = $9, max_retries = $10, retry_delay_secs = $11,
                updated_at = $12
            WHERE id = $13
        "#;

        let result = sqlx::query(query)
            .bind(&job.name)
            .bind(&job.description)
            .bind(&target_kind)
            .bind(&target_config)
            .bind(&job.cron_expr)
            .bind(&job.timezone)
            .bind(job.enabled)
            .bind(job.next_run_at)
            .bind(job.exec_timeout_secs)
            .bind(job.max_retries)
            .bind(job.retry_delay_secs)
            .bind(job.updated_at)
            .bind(&job.id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to update job: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", job.id)));
        }

        Ok(())
    }

    /// `job_executions` rows go with it via FK `ON DELETE CASCADE`, which requires
    /// `PRAGMA foreign_keys = ON` on the connection (sqlx enables it by default).
    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM jobs WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to delete job: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// Toggles a job and refreshes `updated_at`.
    pub async fn set_enabled(
        &self,
        id: &str,
        enabled: bool,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE jobs SET enabled = $1, updated_at = $2 WHERE id = $3")
            .bind(enabled)
            .bind(updated_at)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to set job enabled: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// Records run statistics and bumps `run_count` by one; the caller decides how
    /// `failure_count` moves via [`FailureCountUpdate`].
    ///
    /// One trigger is one envelope, so `run_count` grows by 1 regardless of how many
    /// retries happened inside it. `next_run_at` is computed by the caller (the
    /// scheduler); this layer never evaluates cron.
    pub async fn update_after_run(
        &self,
        id: &str,
        last_run_at: Timestamp,
        last_status: ExecutionStatus,
        failure_count_update: FailureCountUpdate,
        next_run_at: Option<Timestamp>,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        // `Unchanged` must leave the column out of the SET clause entirely.
        let failure_count_expr = match failure_count_update {
            FailureCountUpdate::Increment => "failure_count = failure_count + 1,",
            FailureCountUpdate::Reset => "failure_count = 0,",
            FailureCountUpdate::Unchanged => "",
        };

        let query = format!(
            r#"
            UPDATE jobs SET
                last_run_at = $1,
                last_status = $2,
                next_run_at = $3,
                run_count = run_count + 1,
                {failure_count_expr}
                updated_at = $4
            WHERE id = $5
        "#
        );

        let result = sqlx::query(&query)
            .bind(last_run_at)
            .bind(execution_status_as_str(last_status))
            .bind(next_run_at)
            .bind(updated_at)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to update job after run: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// Stores a next run time the caller has already computed from the cron expr.
    pub async fn recompute_next_run(
        &self,
        id: &str,
        next_run_at: Option<Timestamp>,
        updated_at: Timestamp,
    ) -> Result<(), AppError> {
        let result =
            sqlx::query("UPDATE jobs SET next_run_at = $1, updated_at = $2 WHERE id = $3")
                .bind(next_run_at)
                .bind(updated_at)
                .bind(id)
                .execute(self.db.pool())
                .await
                .map_err(|e| {
                    AppError::internal_error(&format!("Failed to set job next_run_at: {}", e))
                })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Job not found: {}", id)));
        }

        Ok(())
    }

    /// Due jobs (`enabled` and `next_run_at <= now`), most overdue first; served by
    /// `idx_jobs_enabled_next`. Unscheduled jobs (`next_run_at IS NULL`) never match.
    pub async fn list_due(&self, now: Timestamp) -> Result<Vec<Job>, AppError> {
        let query = format!(
            "{} WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= $1 \
             ORDER BY next_run_at ASC",
            JOB_SELECT_COLUMNS
        );

        let rows = sqlx::query(&query)
            .bind(now)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to list due jobs: {}", e)))?;

        rows.into_iter().map(Self::row_to_job).collect()
    }

    /// Rebuilds the polymorphic `target` from its two columns.
    fn row_to_job(row: sqlx::sqlite::SqliteRow) -> Result<Job, AppError> {
        let target_kind: String = row
            .try_get("target_kind")
            .map_err(|e| AppError::internal_error(&format!("Failed to read target_kind: {}", e)))?;
        let target_config: String = row.try_get("target_config").map_err(|e| {
            AppError::internal_error(&format!("Failed to read target_config: {}", e))
        })?;
        let target = JobTarget::from_db_parts(&target_kind, &target_config).map_err(|e| {
            AppError::internal_error(&format!("Failed to parse job target: {}", e))
        })?;

        // Nullable column: decode into `Option<String>` first, then parse the enum.
        let last_status: Option<String> = row.try_get("last_status").map_err(|e| {
            AppError::internal_error(&format!("Failed to read last_status: {}", e))
        })?;
        let last_status = last_status
            .map(|s| execution_status_from_str(&s))
            .transpose()?;

        Ok(Job {
            id: row
                .try_get("id")
                .map_err(|e| AppError::internal_error(&format!("Failed to read id: {}", e)))?,
            name: row
                .try_get("name")
                .map_err(|e| AppError::internal_error(&format!("Failed to read name: {}", e)))?,
            description: row.try_get("description").map_err(|e| {
                AppError::internal_error(&format!("Failed to read description: {}", e))
            })?,
            target,
            cron_expr: row.try_get("cron_expr").map_err(|e| {
                AppError::internal_error(&format!("Failed to read cron_expr: {}", e))
            })?,
            timezone: row.try_get("timezone").map_err(|e| {
                AppError::internal_error(&format!("Failed to read timezone: {}", e))
            })?,
            enabled: row.try_get("enabled").map_err(|e| {
                AppError::internal_error(&format!("Failed to read enabled: {}", e))
            })?,
            last_run_at: row.try_get("last_run_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read last_run_at: {}", e))
            })?,
            next_run_at: row.try_get("next_run_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read next_run_at: {}", e))
            })?,
            last_status,
            run_count: row.try_get("run_count").map_err(|e| {
                AppError::internal_error(&format!("Failed to read run_count: {}", e))
            })?,
            failure_count: row.try_get("failure_count").map_err(|e| {
                AppError::internal_error(&format!("Failed to read failure_count: {}", e))
            })?,
            exec_timeout_secs: row.try_get("exec_timeout_secs").map_err(|e| {
                AppError::internal_error(&format!("Failed to read exec_timeout_secs: {}", e))
            })?,
            max_retries: row.try_get("max_retries").map_err(|e| {
                AppError::internal_error(&format!("Failed to read max_retries: {}", e))
            })?,
            retry_delay_secs: row.try_get("retry_delay_secs").map_err(|e| {
                AppError::internal_error(&format!("Failed to read retry_delay_secs: {}", e))
            })?,
            created_at: row.try_get("created_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read created_at: {}", e))
            })?,
            updated_at: row.try_get("updated_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read updated_at: {}", e))
            })?,
        })
    }
}

/// `jobs` column list; callers append their own WHERE / ORDER clauses.
const JOB_SELECT_COLUMNS: &str = r#"
    SELECT id, name, description, target_kind, target_config, cron_expr, timezone,
           enabled, last_run_at, next_run_at, last_status, run_count, failure_count,
           exec_timeout_secs, max_retries, retry_delay_secs,
           created_at, updated_at
    FROM jobs
"#;

const JOB_SELECT_COLUMNS_WITH_WHERE_ID: &str = r#"
    SELECT id, name, description, target_kind, target_config, cron_expr, timezone,
           enabled, last_run_at, next_run_at, last_status, run_count, failure_count,
           exec_timeout_secs, max_retries, retry_delay_secs,
           created_at, updated_at
    FROM jobs WHERE id = $1
"#;

/// Repository for run history (`job_executions` table).
#[derive(Clone)]
pub struct JobExecutionRepository {
    db: Arc<Database>,
}

impl JobExecutionRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Opens a run: inserts a `running` row and returns its id. The outcome columns
    /// (stdout/stderr/exit_code/error/result_ref/ended_at/duration) stay NULL until
    /// `finalize` fills them in on the same row.
    pub async fn insert_running(
        &self,
        id: &str,
        job_id: &str,
        trigger: Trigger,
        attempt: i32,
        started_at: Timestamp,
        created_at: Timestamp,
    ) -> Result<String, AppError> {
        let query = r#"
            INSERT INTO job_executions (
                id, job_id, status, trigger, attempt,
                stdout, stderr, exit_code, error, result_ref,
                started_at, ended_at, duration, created_at
            )
            VALUES ($1, $2, $3, $4, $5, NULL, NULL, NULL, NULL, NULL, $6, NULL, NULL, $7)
        "#;

        sqlx::query(query)
            .bind(id)
            .bind(job_id)
            .bind(execution_status_as_str(ExecutionStatus::Running))
            .bind(trigger_as_str(trigger))
            .bind(attempt)
            .bind(started_at)
            .bind(created_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to insert running execution: {}", e))
            })?;

        Ok(id.to_string())
    }

    /// Closes a run in place. `status` must be terminal (success/failed/timeout);
    /// passing `Running` is rejected so a finished row can never go back to running.
    ///
    /// Retries reuse the same row, so `attempt` is overwritten with the last attempt
    /// number reached (1-based: 1 when the first try was already terminal).
    #[allow(clippy::too_many_arguments)]
    pub async fn finalize(
        &self,
        id: &str,
        status: ExecutionStatus,
        attempt: i32,
        stdout: Option<&str>,
        stderr: Option<&str>,
        exit_code: Option<i32>,
        error: Option<&str>,
        result_ref: Option<&str>,
        ended_at: Timestamp,
        duration: i64,
    ) -> Result<(), AppError> {
        if matches!(status, ExecutionStatus::Running) {
            return Err(AppError::validation_error(
                "finalize requires a terminal status (success/failed/timeout)",
            ));
        }

        let query = r#"
            UPDATE job_executions SET
                status = $1, attempt = $2, stdout = $3, stderr = $4, exit_code = $5,
                error = $6, result_ref = $7, ended_at = $8, duration = $9
            WHERE id = $10
        "#;

        let result = sqlx::query(query)
            .bind(execution_status_as_str(status))
            .bind(attempt)
            .bind(stdout)
            .bind(stderr)
            .bind(exit_code)
            .bind(error)
            .bind(result_ref)
            .bind(ended_at)
            .bind(duration)
            .bind(id)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to finalize execution: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found(&format!("Execution not found: {}", id)));
        }

        Ok(())
    }

    /// Newest first (`started_at` descending, id as a stable tiebreaker).
    pub async fn list_for_job(
        &self,
        job_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<JobExecution>, AppError> {
        let query = format!(
            "{} WHERE job_id = $1 ORDER BY started_at DESC, id DESC LIMIT $2 OFFSET $3",
            EXECUTION_SELECT_COLUMNS
        );

        let rows = sqlx::query(&query)
            .bind(job_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to list executions: {}", e))
            })?;

        rows.into_iter().map(Self::row_to_execution).collect()
    }

    /// FIFO-trims a job's history to the most recent `keep` rows by `started_at` and
    /// returns how many were deleted. Rows still `running` are never counted nor
    /// deleted, so an in-flight run cannot be pruned away.
    pub async fn prune_to(&self, job_id: &str, keep: i64) -> Result<u64, AppError> {
        let query = r#"
            DELETE FROM job_executions
            WHERE id IN (
                SELECT id FROM job_executions
                WHERE job_id = $1 AND status != 'running'
                ORDER BY started_at DESC, id DESC
                LIMIT -1 OFFSET $2
            )
        "#;

        let result = sqlx::query(query)
            .bind(job_id)
            .bind(keep)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to prune executions: {}", e))
            })?;

        Ok(result.rows_affected())
    }

    /// Marks every leftover `running` row with the given terminal status (normally
    /// `Failed`: the previous process exited mid-run) and returns the row count.
    /// Meant to be called once at scheduler startup.
    pub async fn reconcile_stale_running(
        &self,
        status: ExecutionStatus,
        error: &str,
        ended_at: Timestamp,
    ) -> Result<u64, AppError> {
        if matches!(status, ExecutionStatus::Running) {
            return Err(AppError::validation_error(
                "reconcile_stale_running requires a terminal status",
            ));
        }

        // Clamp the derived duration at 0 in case of clock skew.
        let query = r#"
            UPDATE job_executions SET
                status = $1,
                error = $2,
                ended_at = $3,
                duration = MAX($3 - started_at, 0)
            WHERE status = 'running'
        "#;

        let result = sqlx::query(query)
            .bind(execution_status_as_str(status))
            .bind(error)
            .bind(ended_at)
            .execute(self.db.pool())
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to reconcile running executions: {}", e))
            })?;

        Ok(result.rows_affected())
    }

    /// Nullable columns must decode into `Option` fields: decoding SQL NULL into a
    /// non-Option type is a sqlx error, not a default value.
    fn row_to_execution(row: sqlx::sqlite::SqliteRow) -> Result<JobExecution, AppError> {
        let status_str: String = row
            .try_get("status")
            .map_err(|e| AppError::internal_error(&format!("Failed to read status: {}", e)))?;
        let status = execution_status_from_str(&status_str)?;

        let trigger_str: String = row
            .try_get("trigger")
            .map_err(|e| AppError::internal_error(&format!("Failed to read trigger: {}", e)))?;
        let trigger = trigger_from_str(&trigger_str)?;

        Ok(JobExecution {
            id: row
                .try_get("id")
                .map_err(|e| AppError::internal_error(&format!("Failed to read id: {}", e)))?,
            job_id: row
                .try_get("job_id")
                .map_err(|e| AppError::internal_error(&format!("Failed to read job_id: {}", e)))?,
            status,
            trigger,
            attempt: row.try_get("attempt").map_err(|e| {
                AppError::internal_error(&format!("Failed to read attempt: {}", e))
            })?,
            stdout: row
                .try_get("stdout")
                .map_err(|e| AppError::internal_error(&format!("Failed to read stdout: {}", e)))?,
            stderr: row
                .try_get("stderr")
                .map_err(|e| AppError::internal_error(&format!("Failed to read stderr: {}", e)))?,
            exit_code: row.try_get("exit_code").map_err(|e| {
                AppError::internal_error(&format!("Failed to read exit_code: {}", e))
            })?,
            error: row
                .try_get("error")
                .map_err(|e| AppError::internal_error(&format!("Failed to read error: {}", e)))?,
            result_ref: row.try_get("result_ref").map_err(|e| {
                AppError::internal_error(&format!("Failed to read result_ref: {}", e))
            })?,
            started_at: row.try_get("started_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read started_at: {}", e))
            })?,
            ended_at: row.try_get("ended_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read ended_at: {}", e))
            })?,
            duration: row.try_get("duration").map_err(|e| {
                AppError::internal_error(&format!("Failed to read duration: {}", e))
            })?,
            created_at: row.try_get("created_at").map_err(|e| {
                AppError::internal_error(&format!("Failed to read created_at: {}", e))
            })?,
        })
    }
}

/// `job_executions` column list; callers append their own WHERE / ORDER clauses.
const EXECUTION_SELECT_COLUMNS: &str = r#"
    SELECT id, job_id, status, trigger, attempt, stdout, stderr, exit_code, error,
           result_ref, started_at, ended_at, duration, created_at
    FROM job_executions
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::{Job, JobTarget, SessionStrategy};
    use tempfile::tempdir;

    /// Temp SQLite database with all migrations applied.
    async fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.unwrap();
        (db, temp_dir)
    }

    fn sample_job(id: &str, now: Timestamp) -> Job {
        Job {
            id: id.to_string(),
            name: format!("Job {}", id),
            description: Some("a scheduled job".to_string()),
            target: JobTarget::Agent {
                agent_id: "agent_1".to_string(),
                model_id: "gpt-4".to_string(),
                initial_message: "go".to_string(),
                project_id: None,
            },
            cron_expr: "0 9 * * *".to_string(),
            timezone: "local".to_string(),
            enabled: true,
            last_run_at: None,
            next_run_at: Some(now + 1000),
            last_status: None,
            run_count: 0,
            failure_count: 0,
            exec_timeout_secs: 30,
            max_retries: 2,
            retry_delay_secs: 90,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_job_crud_roundtrip() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        let job = sample_job("job_1", now);
        repo.create(&job).await.unwrap();

        // get round-trips including the polymorphic target.
        let fetched = repo.get("job_1").await.unwrap().expect("job exists");
        assert_eq!(fetched.id, job.id);
        assert_eq!(fetched.name, job.name);
        assert_eq!(fetched.description, job.description);
        assert_eq!(fetched.target, job.target);
        assert_eq!(fetched.cron_expr, job.cron_expr);
        assert!(fetched.enabled);
        assert_eq!(fetched.next_run_at, Some(now + 1000));
        assert_eq!(fetched.last_status, None);
        assert_eq!(fetched.run_count, 0);
        // Robustness columns round-trip with the values they were created with.
        assert_eq!(fetched.exec_timeout_secs, 30);
        assert_eq!(fetched.max_retries, 2);
        assert_eq!(fetched.retry_delay_secs, 90);

        // missing id -> None.
        assert!(repo.get("nope").await.unwrap().is_none());

        // update changes definition fields.
        let mut updated = fetched.clone();
        updated.name = "Renamed".to_string();
        updated.target = JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "summarize".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        updated.cron_expr = "*/5 * * * *".to_string();
        updated.next_run_at = Some(now + 2000);
        updated.exec_timeout_secs = 120;
        updated.max_retries = 5;
        updated.retry_delay_secs = 30;
        updated.updated_at = now + 50;
        repo.update(&updated).await.unwrap();

        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.name, "Renamed");
        assert_eq!(after.target, updated.target);
        assert_eq!(after.cron_expr, "*/5 * * * *");
        assert_eq!(after.next_run_at, Some(now + 2000));
        // Robustness columns are part of the definition update.
        assert_eq!(after.exec_timeout_secs, 120);
        assert_eq!(after.max_retries, 5);
        assert_eq!(after.retry_delay_secs, 30);

        // updating a missing job is not_found.
        let mut ghost = sample_job("ghost", now);
        ghost.name = "x".to_string();
        assert!(repo.update(&ghost).await.is_err());

        // delete.
        repo.delete("job_1").await.unwrap();
        assert!(repo.get("job_1").await.unwrap().is_none());
        assert!(repo.delete("job_1").await.is_err());
    }

    #[tokio::test]
    async fn test_robustness_columns_default_when_omitted() {
        // Migration 051 adds the robustness columns as NOT NULL with DEFAULTs,
        // so a raw INSERT that omits them (mirroring a pre-051 / partial write)
        // reads back the named defaults — never an unexpected NULL.
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, name, target_kind, target_config, cron_expr, timezone,
                enabled, run_count, failure_count, created_at, updated_at
            )
            VALUES ($1, $2, 'prompt', '{"providerId":"p1","modelId":"m1","prompt":"hi"}', '0 9 * * *', 'local',
                    1, 0, 0, $3, $3)
            "#,
        )
        .bind("job_defaults")
        .bind("Defaults job")
        .bind(now)
        .execute(repo.db.pool())
        .await
        .unwrap();

        let fetched = repo.get("job_defaults").await.unwrap().expect("job exists");
        assert_eq!(
            fetched.exec_timeout_secs, 0,
            "exec_timeout_secs default is 0 (no timeout)"
        );
        assert_eq!(
            fetched.max_retries, 0,
            "max_retries default is 0 (no retries)"
        );
        assert_eq!(
            fetched.retry_delay_secs, 60,
            "retry_delay_secs default is 60"
        );
    }

    #[tokio::test]
    async fn test_job_list_pagination() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        for i in 0..5 {
            let mut job = sample_job(&format!("job_{}", i), now + i);
            job.created_at = now + i;
            repo.create(&job).await.unwrap();
        }

        // newest-first by created_at.
        let page = repo.list(2, 0).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, "job_4");
        assert_eq!(page[1].id, "job_3");

        let page2 = repo.list(2, 2).await.unwrap();
        assert_eq!(page2[0].id, "job_2");
        assert_eq!(page2[1].id, "job_1");

        let total = repo.list(100, 0).await.unwrap();
        assert_eq!(total.len(), 5);
    }

    #[tokio::test]
    async fn test_set_enabled_and_recompute_next_run() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        let job = sample_job("job_1", now);
        repo.create(&job).await.unwrap();

        repo.set_enabled("job_1", false, now + 10).await.unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert!(!after.enabled);
        assert_eq!(after.updated_at, now + 10);

        // recompute_next_run can set NULL (job no longer scheduled).
        repo.recompute_next_run("job_1", None, now + 20)
            .await
            .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.next_run_at, None);

        repo.recompute_next_run("job_1", Some(now + 5000), now + 30)
            .await
            .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.next_run_at, Some(now + 5000));

        assert!(repo.set_enabled("ghost", true, now).await.is_err());
        assert!(repo.recompute_next_run("ghost", None, now).await.is_err());
    }

    #[tokio::test]
    async fn test_update_after_run_counts() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let now = 1_000_000i64;

        let job = sample_job("job_1", now);
        repo.create(&job).await.unwrap();

        // success run: run_count +1, failure_count reset (stays 0 here).
        repo.update_after_run(
            "job_1",
            now + 100,
            ExecutionStatus::Success,
            FailureCountUpdate::Reset,
            Some(now + 9000),
            now + 100,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 1);
        assert_eq!(after.failure_count, 0);
        assert_eq!(after.last_run_at, Some(now + 100));
        assert_eq!(after.last_status, Some(ExecutionStatus::Success));
        assert_eq!(after.next_run_at, Some(now + 9000));

        // failed run: run_count +1, failure_count +1.
        repo.update_after_run(
            "job_1",
            now + 200,
            ExecutionStatus::Failed,
            FailureCountUpdate::Increment,
            None,
            now + 200,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 2);
        assert_eq!(after.failure_count, 1);
        assert_eq!(after.last_status, Some(ExecutionStatus::Failed));
        assert_eq!(after.next_run_at, None);

        // timeout counts as a continuous failure too: failure_count 1 -> 2.
        repo.update_after_run(
            "job_1",
            now + 300,
            ExecutionStatus::Timeout,
            FailureCountUpdate::Increment,
            Some(now + 9999),
            now + 300,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 3);
        assert_eq!(after.failure_count, 2);

        // a success now resets the continuous-failure counter to 0 (the chain
        // is broken), while run_count keeps climbing.
        repo.update_after_run(
            "job_1",
            now + 400,
            ExecutionStatus::Success,
            FailureCountUpdate::Reset,
            None,
            now + 400,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 4);
        assert_eq!(after.failure_count, 0, "success resets the failure chain");

        // a manual run leaves failure_count untouched (Unchanged), but still
        // advances run_count. Seed a non-zero failure_count first to prove the
        // counter is genuinely left alone rather than coincidentally 0.
        repo.update_after_run(
            "job_1",
            now + 500,
            ExecutionStatus::Failed,
            FailureCountUpdate::Increment,
            None,
            now + 500,
        )
        .await
        .unwrap();
        let before_manual = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(before_manual.failure_count, 1);
        repo.update_after_run(
            "job_1",
            now + 600,
            ExecutionStatus::Failed,
            FailureCountUpdate::Unchanged,
            None,
            now + 600,
        )
        .await
        .unwrap();
        let after = repo.get("job_1").await.unwrap().unwrap();
        assert_eq!(after.run_count, 6, "manual run still advances run_count");
        assert_eq!(
            after.failure_count, 1,
            "manual run leaves failure_count untouched"
        );

        assert!(repo
            .update_after_run(
                "ghost",
                now,
                ExecutionStatus::Success,
                FailureCountUpdate::Reset,
                None,
                now
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_list_due_selects_enabled_and_past_due() {
        let (db, _tmp) = create_test_db().await;
        let repo = JobRepository::new(Arc::new(db));
        let base = 1_000_000i64;
        let now = base + 5000;

        // due: enabled, next_run_at <= now.
        let mut due_a = sample_job("due_a", base);
        due_a.next_run_at = Some(now - 100);
        repo.create(&due_a).await.unwrap();

        let mut due_b = sample_job("due_b", base);
        due_b.next_run_at = Some(now); // boundary: == now is due
        repo.create(&due_b).await.unwrap();

        // not due: future next_run_at.
        let mut future = sample_job("future", base);
        future.next_run_at = Some(now + 100);
        repo.create(&future).await.unwrap();

        // not due: disabled even though past.
        let mut disabled = sample_job("disabled", base);
        disabled.enabled = false;
        disabled.next_run_at = Some(now - 100);
        repo.create(&disabled).await.unwrap();

        // not due: next_run_at IS NULL.
        let mut unscheduled = sample_job("unscheduled", base);
        unscheduled.next_run_at = None;
        repo.create(&unscheduled).await.unwrap();

        let due = repo.list_due(now).await.unwrap();
        let ids: Vec<&str> = due.iter().map(|j| j.id.as_str()).collect();
        // ordered by next_run_at ASC: due_a (now-100) before due_b (now).
        assert_eq!(ids, vec!["due_a", "due_b"]);
    }

    #[tokio::test]
    async fn test_list_due_uses_enabled_next_index() {
        // Confirm the query planner picks idx_jobs_enabled_next for the due scan.
        // EXPLAIN QUERY PLAN returns (id, parent, notused, detail); the plan
        // text lives in the `detail` column.
        let (db, _tmp) = create_test_db().await;
        let rows = sqlx::query(
            "EXPLAIN QUERY PLAN \
             SELECT id FROM jobs \
             WHERE enabled = 1 AND next_run_at IS NOT NULL AND next_run_at <= 0 \
             ORDER BY next_run_at ASC",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let plan: Vec<String> = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").unwrap_or_default())
            .collect();
        let joined = plan.join(" ");
        assert!(
            joined.contains("idx_jobs_enabled_next"),
            "due query must use idx_jobs_enabled_next, plan was: {}",
            joined
        );
    }

    // ---- JobExecutionRepository ----

    async fn seed_job(repo: &JobRepository, id: &str, now: Timestamp) {
        repo.create(&sample_job(id, now)).await.unwrap();
    }

    #[tokio::test]
    async fn test_execution_insert_running_then_finalize_in_place() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;

        let exec_id = execs
            .insert_running("exec_1", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        assert_eq!(exec_id, "exec_1");

        // running row: terminal fields are NULL (decode into Option, no panic).
        let running = execs.list_for_job("job_1", 10, 0).await.unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].status, ExecutionStatus::Running);
        assert_eq!(running[0].trigger, Trigger::Schedule);
        assert_eq!(running[0].stdout, None);
        assert_eq!(running[0].exit_code, None);
        assert_eq!(running[0].ended_at, None);
        assert_eq!(running[0].duration, None);

        // finalize updates the SAME row in place, and writes the final attempt
        // (here 3, to prove the column is overwritten from its insert-time 1).
        execs
            .finalize(
                "exec_1",
                ExecutionStatus::Success,
                3,
                Some("out"),
                Some("err"),
                Some(0),
                None,
                Some("session_42"),
                now + 500,
                500,
            )
            .await
            .unwrap();

        let after = execs.list_for_job("job_1", 10, 0).await.unwrap();
        assert_eq!(after.len(), 1, "finalize must not create a new row");
        assert_eq!(after[0].id, "exec_1");
        assert_eq!(after[0].status, ExecutionStatus::Success);
        assert_eq!(after[0].attempt, 3, "finalize overwrites the attempt column");
        assert_eq!(after[0].stdout.as_deref(), Some("out"));
        assert_eq!(after[0].exit_code, Some(0));
        assert_eq!(after[0].error, None);
        assert_eq!(after[0].result_ref.as_deref(), Some("session_42"));
        assert_eq!(after[0].ended_at, Some(now + 500));
        assert_eq!(after[0].duration, Some(500));

        // finalize with Running status is rejected.
        assert!(execs
            .finalize(
                "exec_1",
                ExecutionStatus::Running,
                1,
                None,
                None,
                None,
                None,
                None,
                now,
                0,
            )
            .await
            .is_err());

        // finalize a missing row is not_found.
        assert!(execs
            .finalize(
                "ghost",
                ExecutionStatus::Failed,
                1,
                None,
                None,
                None,
                None,
                None,
                now,
                0,
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_list_for_job_newest_first() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        for i in 0..3 {
            execs
                .insert_running(
                    &format!("exec_{}", i),
                    "job_1",
                    Trigger::Manual,
                    1,
                    now + i * 100,
                    now + i * 100,
                )
                .await
                .unwrap();
        }

        let listed = execs.list_for_job("job_1", 10, 0).await.unwrap();
        let ids: Vec<&str> = listed.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["exec_2", "exec_1", "exec_0"]);
    }

    /// Helper: insert `count` finalized executions for a job with ascending
    /// started_at, then return their ids in insertion order (oldest-first).
    async fn seed_finalized(
        execs: &JobExecutionRepository,
        job_id: &str,
        count: i64,
        base: Timestamp,
    ) -> Vec<String> {
        let mut ids = Vec::new();
        for i in 0..count {
            let id = format!("{}_e{}", job_id, i);
            let started = base + i;
            execs
                .insert_running(&id, job_id, Trigger::Schedule, 1, started, started)
                .await
                .unwrap();
            execs
                .finalize(
                    &id,
                    ExecutionStatus::Success,
                    1,
                    None,
                    None,
                    Some(0),
                    None,
                    None,
                    started + 1,
                    1,
                )
                .await
                .unwrap();
            ids.push(id);
        }
        ids
    }

    #[tokio::test]
    async fn test_prune_exactly_n_keeps_all() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        seed_finalized(&execs, "job_1", 100, now).await;

        let removed = execs.prune_to("job_1", 100).await.unwrap();
        assert_eq!(removed, 0, "exactly N keeps all");
        assert_eq!(execs.list_for_job("job_1", 1000, 0).await.unwrap().len(), 100);
    }

    #[tokio::test]
    async fn test_prune_n_plus_one_drops_oldest() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        let ids = seed_finalized(&execs, "job_1", 101, now).await;

        let removed = execs.prune_to("job_1", 100).await.unwrap();
        assert_eq!(removed, 1, "N+1 drops exactly the oldest");

        let remaining = execs.list_for_job("job_1", 1000, 0).await.unwrap();
        assert_eq!(remaining.len(), 100);
        // Oldest (ids[0]) must be gone; newest must remain.
        let remaining_ids: Vec<&str> = remaining.iter().map(|e| e.id.as_str()).collect();
        assert!(!remaining_ids.contains(&ids[0].as_str()));
        assert!(remaining_ids.contains(&ids[100].as_str()));
    }

    #[tokio::test]
    async fn test_prune_is_per_job_fifo() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_a", now).await;
        seed_job(&jobs, "job_b", now).await;
        let a_ids = seed_finalized(&execs, "job_a", 5, now).await;
        seed_finalized(&execs, "job_b", 5, now).await;

        // Prune job_a to 3: drops its 2 oldest; job_b untouched.
        let removed = execs.prune_to("job_a", 3).await.unwrap();
        assert_eq!(removed, 2);

        let a_remaining = execs.list_for_job("job_a", 100, 0).await.unwrap();
        assert_eq!(a_remaining.len(), 3);
        let a_remaining_ids: Vec<&str> = a_remaining.iter().map(|e| e.id.as_str()).collect();
        // Kept the 3 newest (a_ids[2..5]); dropped a_ids[0], a_ids[1].
        assert!(!a_remaining_ids.contains(&a_ids[0].as_str()));
        assert!(!a_remaining_ids.contains(&a_ids[1].as_str()));
        assert!(a_remaining_ids.contains(&a_ids[4].as_str()));

        // job_b is fully intact.
        assert_eq!(execs.list_for_job("job_b", 100, 0).await.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn test_prune_never_deletes_running_rows() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;
        // 3 finalized + 2 still-running (oldest started_at).
        execs
            .insert_running("run_old_0", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        execs
            .insert_running("run_old_1", "job_1", Trigger::Schedule, 1, now + 1, now + 1)
            .await
            .unwrap();
        seed_finalized(&execs, "job_1", 3, now + 100).await;

        // Keep only 1 finalized row; running rows must survive regardless.
        let removed = execs.prune_to("job_1", 1).await.unwrap();
        assert_eq!(removed, 2, "only the 2 oldest finalized rows are pruned");

        let remaining = execs.list_for_job("job_1", 100, 0).await.unwrap();
        let remaining_ids: Vec<&str> = remaining.iter().map(|e| e.id.as_str()).collect();
        assert!(remaining_ids.contains(&"run_old_0"));
        assert!(remaining_ids.contains(&"run_old_1"));
        // 2 running + 1 kept finalized = 3 rows.
        assert_eq!(remaining.len(), 3);
    }

    #[tokio::test]
    async fn test_reconcile_stale_running_marks_terminal() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        seed_job(&jobs, "job_1", now).await;

        // 2 stale running rows + 1 already-finalized row.
        execs
            .insert_running("stale_0", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        execs
            .insert_running("stale_1", "job_1", Trigger::Manual, 1, now + 10, now + 10)
            .await
            .unwrap();
        execs
            .insert_running("done", "job_1", Trigger::Schedule, 1, now + 20, now + 20)
            .await
            .unwrap();
        execs
            .finalize(
                "done",
                ExecutionStatus::Success,
                1,
                None,
                None,
                Some(0),
                None,
                None,
                now + 25,
                5,
            )
            .await
            .unwrap();

        let affected = execs
            .reconcile_stale_running(ExecutionStatus::Failed, "interrupted", now + 1000)
            .await
            .unwrap();
        assert_eq!(affected, 2, "only the running rows are reconciled");

        let all = execs.list_for_job("job_1", 100, 0).await.unwrap();
        for e in &all {
            assert_ne!(e.status, ExecutionStatus::Running, "no running rows remain");
        }
        let stale: Vec<&JobExecution> =
            all.iter().filter(|e| e.id.starts_with("stale_")).collect();
        for e in stale {
            assert_eq!(e.status, ExecutionStatus::Failed);
            assert_eq!(e.error.as_deref(), Some("interrupted"));
            assert_eq!(e.ended_at, Some(now + 1000));
            assert!(e.duration.unwrap() >= 0);
        }
        // The already-finalized row keeps its success status untouched.
        let done = all.iter().find(|e| e.id == "done").unwrap();
        assert_eq!(done.status, ExecutionStatus::Success);
        assert_eq!(done.error, None);
    }

    #[tokio::test]
    async fn test_delete_job_cascades_executions_with_fk_on() {
        let (db, _tmp) = create_test_db().await;
        let db_arc = Arc::new(db);
        let jobs = JobRepository::new(db_arc.clone());
        let execs = JobExecutionRepository::new(db_arc.clone());
        let now = 1_000_000i64;

        // sqlx enables PRAGMA foreign_keys by default; assert it for this test.
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(db_arc.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(fk, 1, "FK enforcement must be ON for cascade");

        seed_job(&jobs, "job_1", now).await;
        execs
            .insert_running("exec_1", "job_1", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        assert_eq!(execs.list_for_job("job_1", 10, 0).await.unwrap().len(), 1);

        jobs.delete("job_1").await.unwrap();

        // Cascade removed the execution row.
        assert_eq!(execs.list_for_job("job_1", 10, 0).await.unwrap().len(), 0);
    }
}
