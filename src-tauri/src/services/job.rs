// Scheduled-job business layer.
//
// Wraps `JobRepository` with input validation so the IPC commands stay thin.
// Validation lives here (not in the repository or commands) and surfaces as the
// unified `VALIDATION_ERROR` `AppError`:
// - name must be non-empty after trimming (duplicates across ids are allowed);
// - `JobTarget` must be complete for its kind.
//
// This feature does NOT wire the scheduler/executor: `next_run_at` is computed
// optimistically from the cron on create/update when possible, but a NULL value
// is acceptable and left for the scheduler feature to fill.

use std::sync::Arc;

use crate::models::AppError;
use crate::storage::types::{
    Job, JobExecution, JobTarget, Timestamp, DEFAULT_EXEC_TIMEOUT_SECS, DEFAULT_MAX_RETRIES,
    DEFAULT_RETRY_DELAY_SECS, UUID,
};
use crate::storage::{JobExecutionRepository, JobRepository};
use crate::utils::cron;

/// Default page size for `list` when the caller omits `limit`.
const DEFAULT_LIST_LIMIT: i32 = 50;

/// Default page size for `list_executions` when the caller omits `limit`.
/// Sized for the detail modal's history timeline.
const DEFAULT_EXECUTION_LIST_LIMIT: i32 = 50;

/// Fields needed to create a job. The `target` is already structured, so the
/// command layer only has to deserialize it.
#[derive(Debug, Clone)]
pub struct JobCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub target: JobTarget,
    pub cron_expr: String,
    pub timezone: String,
    /// Defaults to `true` when omitted.
    pub enabled: Option<bool>,
    /// Per-run timeout in seconds; `None` -> [`DEFAULT_EXEC_TIMEOUT_SECS`].
    pub exec_timeout_secs: Option<i64>,
    /// Max retry attempts; `None` -> [`DEFAULT_MAX_RETRIES`].
    pub max_retries: Option<i64>,
    /// Delay between retries in seconds; `None` -> [`DEFAULT_RETRY_DELAY_SECS`].
    pub retry_delay_secs: Option<i64>,
}

/// Fields needed to fully replace a job's definition (run statistics are not
/// touched here — those belong to the scheduler/executor features).
#[derive(Debug, Clone)]
pub struct JobUpdateRequest {
    pub name: String,
    pub description: Option<String>,
    pub target: JobTarget,
    pub cron_expr: String,
    pub timezone: String,
    pub enabled: bool,
    /// Per-run timeout in seconds; `None` -> [`DEFAULT_EXEC_TIMEOUT_SECS`].
    pub exec_timeout_secs: Option<i64>,
    /// Max retry attempts; `None` -> [`DEFAULT_MAX_RETRIES`].
    pub max_retries: Option<i64>,
    /// Delay between retries in seconds; `None` -> [`DEFAULT_RETRY_DELAY_SECS`].
    pub retry_delay_secs: Option<i64>,
}

/// Scheduled-job service: validation + CRUD over `JobRepository`, plus
/// read access to execution history via `JobExecutionRepository`.
#[derive(Clone)]
pub struct JobService {
    repository: JobRepository,
    execution_repository: JobExecutionRepository,
}

impl JobService {
    pub fn new(repository: JobRepository, execution_repository: JobExecutionRepository) -> Self {
        Self {
            repository,
            execution_repository,
        }
    }

    /// Create a job. Validates name + target completeness, generates id and
    /// timestamps, and best-effort computes the first `next_run_at` from the
    /// cron (NULL when the cron yields no upcoming occurrence — the scheduler
    /// feature fills it in later).
    pub async fn create(&self, request: JobCreateRequest) -> Result<Job, AppError> {
        let name = validate_name(&request.name)?;
        validate_target(&request.target)?;
        cron::validate(&request.cron_expr)?;
        let robustness = validate_robustness(
            request.exec_timeout_secs,
            request.max_retries,
            request.retry_delay_secs,
        )?;

        let now = current_timestamp();
        let next_run_at = first_occurrence(&request.cron_expr);

        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description: request.description,
            target: request.target,
            cron_expr: request.cron_expr,
            timezone: request.timezone,
            enabled: request.enabled.unwrap_or(true),
            last_run_at: None,
            next_run_at,
            last_status: None,
            run_count: 0,
            failure_count: 0,
            exec_timeout_secs: robustness.exec_timeout_secs,
            max_retries: robustness.max_retries,
            retry_delay_secs: robustness.retry_delay_secs,
            created_at: now,
            updated_at: now,
        };

        self.repository.create(&job).await?;
        Ok(job)
    }

    /// List jobs, newest-first (delegated to the repository ordering).
    pub async fn list(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<Job>, AppError> {
        let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT) as i64;
        let offset = offset.unwrap_or(0) as i64;
        self.repository.list(limit, offset).await
    }

    /// Fetch one job by id; `NOT_FOUND` when it does not exist.
    pub async fn get(&self, id: UUID) -> Result<Job, AppError> {
        match self.repository.get(&id).await? {
            Some(job) => Ok(job),
            None => Err(AppError::not_found(&format!("Job not found: {}", id))),
        }
    }

    /// Replace a job's definition fields. Validates the new name/target/cron,
    /// preserves run statistics, and recomputes `next_run_at` from the cron.
    pub async fn update(&self, id: UUID, request: JobUpdateRequest) -> Result<Job, AppError> {
        let name = validate_name(&request.name)?;
        validate_target(&request.target)?;
        cron::validate(&request.cron_expr)?;
        let robustness = validate_robustness(
            request.exec_timeout_secs,
            request.max_retries,
            request.retry_delay_secs,
        )?;

        // Load existing to preserve run statistics that the definition update
        // must not clobber.
        let mut job = self.get(id).await?;

        job.name = name;
        job.description = request.description;
        job.target = request.target;
        job.cron_expr = request.cron_expr;
        job.timezone = request.timezone;
        job.enabled = request.enabled;
        job.exec_timeout_secs = robustness.exec_timeout_secs;
        job.max_retries = robustness.max_retries;
        job.retry_delay_secs = robustness.retry_delay_secs;
        job.next_run_at = first_occurrence(&job.cron_expr);
        job.updated_at = current_timestamp();

        self.repository.update(&job).await?;
        Ok(job)
    }

    /// Delete a job (its executions cascade in the repository layer).
    pub async fn delete(&self, id: UUID) -> Result<(), AppError> {
        self.repository.delete(&id).await
    }

    /// Enable/disable a job and return the refreshed record.
    pub async fn set_enabled(&self, id: UUID, enabled: bool) -> Result<Job, AppError> {
        let now = current_timestamp();
        self.repository.set_enabled(&id, enabled, now).await?;
        self.get(id).await
    }

    /// List a job's execution history, newest-first (delegated to the
    /// repository ordering). An empty history yields an empty vec, not an error,
    /// so the detail modal can render its "no executions" empty state.
    ///
    /// Any in-progress (`running`) execution is returned alongside finalized
    /// rows because the repository reads from `job_executions` directly, so the
    /// timeline reflects live runs without depending on event subscriptions.
    pub async fn list_executions(
        &self,
        job_id: UUID,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<JobExecution>, AppError> {
        let limit = limit.unwrap_or(DEFAULT_EXECUTION_LIST_LIMIT) as i64;
        let offset = offset.unwrap_or(0) as i64;
        self.execution_repository
            .list_for_job(&job_id, limit, offset)
            .await
    }
}

/// Trim the name and reject an empty result. Returns the trimmed name on success.
fn validate_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_error("Job name cannot be empty"));
    }
    Ok(trimmed.to_string())
}

/// Validate that a `JobTarget` carries everything its kind needs:
/// - artifact: non-empty `artifact_id`;
/// - prompt: non-empty `provider_id`, `model_id`, and `prompt`;
/// - agent: non-empty `agent_id`.
///
/// Whitespace-only values count as empty.
fn validate_target(target: &JobTarget) -> Result<(), AppError> {
    match target {
        JobTarget::Artifact { artifact_id, .. } => {
            if artifact_id.trim().is_empty() {
                return Err(AppError::validation_error(
                    "Artifact target requires an artifact_id",
                ));
            }
        }
        JobTarget::Prompt {
            provider_id,
            model_id,
            prompt,
            ..
        } => {
            if provider_id.trim().is_empty() {
                return Err(AppError::validation_error(
                    "Prompt target requires a provider_id",
                ));
            }
            if model_id.trim().is_empty() {
                return Err(AppError::validation_error(
                    "Prompt target requires a model_id",
                ));
            }
            if prompt.trim().is_empty() {
                return Err(AppError::validation_error(
                    "Prompt target requires a prompt",
                ));
            }
        }
        JobTarget::Agent { agent_id, .. } => {
            if agent_id.trim().is_empty() {
                return Err(AppError::validation_error(
                    "Agent target requires an agent_id",
                ));
            }
        }
    }
    Ok(())
}

/// Resolved robustness fields after applying named defaults and rejecting
/// negative values.
struct Robustness {
    exec_timeout_secs: i64,
    max_retries: i64,
    retry_delay_secs: i64,
}

/// Resolve the robustness fields: each `None` falls back to its named default,
/// and any negative value is rejected with `VALIDATION_ERROR` (VAL-ROBUST-003).
/// `0` is a valid, meaningful value (no timeout / no retries) and is preserved.
fn validate_robustness(
    exec_timeout_secs: Option<i64>,
    max_retries: Option<i64>,
    retry_delay_secs: Option<i64>,
) -> Result<Robustness, AppError> {
    let exec_timeout_secs = exec_timeout_secs.unwrap_or(DEFAULT_EXEC_TIMEOUT_SECS);
    let max_retries = max_retries.unwrap_or(DEFAULT_MAX_RETRIES);
    let retry_delay_secs = retry_delay_secs.unwrap_or(DEFAULT_RETRY_DELAY_SECS);

    if exec_timeout_secs < 0 {
        return Err(AppError::validation_error(
            "Execution timeout cannot be negative",
        ));
    }
    if max_retries < 0 {
        return Err(AppError::validation_error("Max retries cannot be negative"));
    }
    if retry_delay_secs < 0 {
        return Err(AppError::validation_error("Retry delay cannot be negative"));
    }

    Ok(Robustness {
        exec_timeout_secs,
        max_retries,
        retry_delay_secs,
    })
}

/// Best-effort first upcoming occurrence for a (already-validated) cron, or
/// `None` when the schedule yields nothing in the visible future.
fn first_occurrence(cron_expr: &str) -> Option<Timestamp> {
    cron::next_occurrences(cron_expr, 1)
        .ok()
        .and_then(|mut v| v.drain(..).next())
}

fn current_timestamp() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Wrap a raw database handle in a `JobService`, used by the app wiring. Builds
/// both the definition repository and the execution-history repository over the
/// same pool.
impl JobService {
    pub fn from_db(db: Arc<crate::storage::Database>) -> Self {
        Self::new(
            JobRepository::new(db.clone()),
            JobExecutionRepository::new(db),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::SessionStrategy;
    use crate::storage::Database;
    use std::collections::HashMap;
    use tempfile::tempdir;

    async fn create_service() -> (JobService, tempfile::TempDir) {
        let temp_dir = tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let db = Database::new(&db_path).await.expect("db");
        (JobService::from_db(Arc::new(db)), temp_dir)
    }

    fn artifact_target() -> JobTarget {
        JobTarget::Artifact {
            artifact_id: "artifact_1".to_string(),
            args: vec![],
            env: HashMap::new(),
        }
    }

    fn prompt_target() -> JobTarget {
        JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "Summarize today".to_string(),
            session_strategy: SessionStrategy::NewSession,
        }
    }

    fn agent_target() -> JobTarget {
        JobTarget::Agent {
            agent_id: "agent_1".to_string(),
            initial_message: "go".to_string(),
            project_id: None,
        }
    }

    fn create_request(name: &str, target: JobTarget) -> JobCreateRequest {
        JobCreateRequest {
            name: name.to_string(),
            description: Some("a job".to_string()),
            target,
            cron_expr: "0 9 * * *".to_string(),
            timezone: "local".to_string(),
            enabled: None,
            exec_timeout_secs: None,
            max_retries: None,
            retry_delay_secs: None,
        }
    }

    #[tokio::test]
    async fn create_rejects_empty_name() {
        let (service, _tmp) = create_service().await;
        let err = service
            .create(create_request("", artifact_target()))
            .await
            .expect_err("empty name must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn create_rejects_whitespace_name() {
        let (service, _tmp) = create_service().await;
        let err = service
            .create(create_request("   ", artifact_target()))
            .await
            .expect_err("whitespace name must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn create_trims_name() {
        let (service, _tmp) = create_service().await;
        let job = service
            .create(create_request("  Daily  ", artifact_target()))
            .await
            .expect("create");
        assert_eq!(job.name, "Daily");
    }

    #[tokio::test]
    async fn create_allows_duplicate_names_with_distinct_ids() {
        let (service, _tmp) = create_service().await;
        let a = service
            .create(create_request("Same Name", artifact_target()))
            .await
            .expect("first create");
        let b = service
            .create(create_request("Same Name", artifact_target()))
            .await
            .expect("second create with same name");
        assert_eq!(a.name, b.name);
        assert_ne!(a.id, b.id);
    }

    #[tokio::test]
    async fn create_rejects_artifact_missing_artifact_id() {
        let (service, _tmp) = create_service().await;
        let target = JobTarget::Artifact {
            artifact_id: "  ".to_string(),
            args: vec![],
            env: HashMap::new(),
        };
        let err = service
            .create(create_request("Job", target))
            .await
            .expect_err("missing artifact_id must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn create_rejects_prompt_missing_fields() {
        let (service, _tmp) = create_service().await;

        // Missing provider_id.
        let no_provider = JobTarget::Prompt {
            provider_id: "".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "do it".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        assert_eq!(
            service
                .create(create_request("Job", no_provider))
                .await
                .expect_err("missing provider_id")
                .code,
            "VALIDATION_ERROR"
        );

        // Missing model_id.
        let no_model = JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "".to_string(),
            prompt: "do it".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        assert_eq!(
            service
                .create(create_request("Job", no_model))
                .await
                .expect_err("missing model_id")
                .code,
            "VALIDATION_ERROR"
        );

        // Whitespace-only prompt counts as empty.
        let blank_prompt = JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "   ".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        assert_eq!(
            service
                .create(create_request("Job", blank_prompt))
                .await
                .expect_err("blank prompt")
                .code,
            "VALIDATION_ERROR"
        );
    }

    #[tokio::test]
    async fn create_rejects_agent_missing_agent_id() {
        let (service, _tmp) = create_service().await;
        let target = JobTarget::Agent {
            agent_id: "".to_string(),
            initial_message: "go".to_string(),
            project_id: None,
        };
        let err = service
            .create(create_request("Job", target))
            .await
            .expect_err("missing agent_id must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn create_rejects_invalid_cron() {
        let (service, _tmp) = create_service().await;
        let mut req = create_request("Job", prompt_target());
        req.cron_expr = "not-a-cron".to_string();
        let err = service
            .create(req)
            .await
            .expect_err("invalid cron must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn create_defaults_enabled_true() {
        let (service, _tmp) = create_service().await;
        let job = service
            .create(create_request("Job", agent_target()))
            .await
            .expect("create");
        assert!(job.enabled);
    }

    #[tokio::test]
    async fn create_respects_explicit_enabled() {
        let (service, _tmp) = create_service().await;
        let mut req = create_request("Job", agent_target());
        req.enabled = Some(false);
        let job = service.create(req).await.expect("create");
        assert!(!job.enabled);
    }

    #[tokio::test]
    async fn crud_roundtrip_via_service() {
        let (service, _tmp) = create_service().await;

        // create
        let created = service
            .create(create_request("Original", prompt_target()))
            .await
            .expect("create");
        assert_eq!(created.name, "Original");
        assert_eq!(created.run_count, 0);

        // get
        let fetched = service.get(created.id.clone()).await.expect("get");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.target, prompt_target());

        // list
        let listed = service.list(Some(10), Some(0)).await.expect("list");
        assert_eq!(listed.len(), 1);

        // update (definition fields replaced, statistics preserved)
        let updated = service
            .update(
                created.id.clone(),
                JobUpdateRequest {
                    name: "Renamed".to_string(),
                    description: None,
                    target: agent_target(),
                    cron_expr: "*/5 * * * *".to_string(),
                    timezone: "local".to_string(),
                    enabled: false,
                    exec_timeout_secs: Some(120),
                    max_retries: Some(3),
                    retry_delay_secs: Some(45),
                },
            )
            .await
            .expect("update");
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.target, agent_target());
        assert_eq!(updated.cron_expr, "*/5 * * * *");
        assert!(!updated.enabled);
        assert_eq!(updated.run_count, 0);
        assert_eq!(updated.exec_timeout_secs, 120);
        assert_eq!(updated.max_retries, 3);
        assert_eq!(updated.retry_delay_secs, 45);

        // set_enabled toggles back on
        let toggled = service
            .set_enabled(created.id.clone(), true)
            .await
            .expect("set_enabled");
        assert!(toggled.enabled);

        // delete
        service.delete(created.id.clone()).await.expect("delete");
        let err = service
            .get(created.id)
            .await
            .expect_err("deleted job is gone");
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let (service, _tmp) = create_service().await;
        let err = service
            .get("nope".to_string())
            .await
            .expect_err("missing job");
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn list_executions_empty_for_job_without_runs() {
        let (service, _tmp) = create_service().await;
        let job = service
            .create(create_request("Job", artifact_target()))
            .await
            .expect("create");

        // A job that has never run yields an empty history (not an error), so the
        // detail modal can render its "no executions" empty state.
        let history = service
            .list_executions(job.id, None, None)
            .await
            .expect("list_executions");
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn list_executions_returns_running_and_finalized_newest_first() {
        use crate::storage::types::{ExecutionStatus, Trigger};
        use crate::storage::JobExecutionRepository;
        use crate::storage::JobRepository;

        let temp_dir = tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(Database::new(&db_path).await.expect("db"));
        let service = JobService::from_db(db.clone());

        // Seed a job through the service, then drive the execution repository
        // directly to mirror what the executor would produce.
        let job = service
            .create(create_request("Job", artifact_target()))
            .await
            .expect("create");
        let execs = JobExecutionRepository::new(db.clone());
        let _jobs = JobRepository::new(db);
        let now = 1_000_000i64;

        // Older finalized success run.
        execs
            .insert_running("exec_old", &job.id, Trigger::Schedule, 1, now, now)
            .await
            .unwrap();
        execs
            .finalize(
                "exec_old",
                ExecutionStatus::Success,
                Some("stdout text"),
                Some(""),
                Some(0),
                None,
                None,
                now + 123,
                123,
            )
            .await
            .unwrap();

        // Newer still-running run (terminal fields NULL).
        execs
            .insert_running(
                "exec_new",
                &job.id,
                Trigger::Manual,
                1,
                now + 500,
                now + 500,
            )
            .await
            .unwrap();

        let history = service
            .list_executions(job.id, None, None)
            .await
            .expect("list_executions");
        assert_eq!(history.len(), 2);
        // Newest-first: the running row comes before the finalized one.
        assert_eq!(history[0].id, "exec_new");
        assert_eq!(history[0].status, ExecutionStatus::Running);
        assert_eq!(history[0].ended_at, None);
        assert_eq!(history[0].duration, None);
        assert_eq!(history[1].id, "exec_old");
        assert_eq!(history[1].status, ExecutionStatus::Success);
        // Sub-second duration is preserved as a non-zero value.
        assert_eq!(history[1].duration, Some(123));
        // Empty stdout/stderr survive as empty strings, not NULL.
        assert_eq!(history[1].stdout.as_deref(), Some("stdout text"));
        assert_eq!(history[1].stderr.as_deref(), Some(""));
    }

    #[tokio::test]
    async fn update_rejects_invalid_name() {
        let (service, _tmp) = create_service().await;
        let created = service
            .create(create_request("Job", artifact_target()))
            .await
            .expect("create");
        let err = service
            .update(
                created.id,
                JobUpdateRequest {
                    name: "   ".to_string(),
                    description: None,
                    target: artifact_target(),
                    cron_expr: "0 9 * * *".to_string(),
                    timezone: "local".to_string(),
                    enabled: true,
                    exec_timeout_secs: None,
                    max_retries: None,
                    retry_delay_secs: None,
                },
            )
            .await
            .expect_err("blank name on update must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn create_applies_robustness_defaults_when_omitted() {
        // Omitting the robustness fields yields the named defaults persisted to
        // the row (VAL-ROBUST-002): 0/0/60, never an unexpected NULL.
        let (service, _tmp) = create_service().await;
        let job = service
            .create(create_request("Job", artifact_target()))
            .await
            .expect("create");
        assert_eq!(job.exec_timeout_secs, DEFAULT_EXEC_TIMEOUT_SECS);
        assert_eq!(job.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(job.retry_delay_secs, DEFAULT_RETRY_DELAY_SECS);

        // And the defaults survive a fetch (read back from the row, not just the
        // returned struct).
        let fetched = service.get(job.id).await.expect("get");
        assert_eq!(fetched.exec_timeout_secs, 0);
        assert_eq!(fetched.max_retries, 0);
        assert_eq!(fetched.retry_delay_secs, 60);
    }

    #[tokio::test]
    async fn create_persists_explicit_robustness_values() {
        // VAL-ROBUST-001: supplied values are stored and read back verbatim.
        let (service, _tmp) = create_service().await;
        let mut req = create_request("Job", artifact_target());
        req.exec_timeout_secs = Some(300);
        req.max_retries = Some(4);
        req.retry_delay_secs = Some(15);
        let job = service.create(req).await.expect("create");

        let fetched = service.get(job.id).await.expect("get");
        assert_eq!(fetched.exec_timeout_secs, 300);
        assert_eq!(fetched.max_retries, 4);
        assert_eq!(fetched.retry_delay_secs, 15);
    }

    #[tokio::test]
    async fn create_rejects_negative_robustness_values() {
        // VAL-ROBUST-003: a negative timeout / retries / delay is rejected and
        // no out-of-range row is written.
        let (service, _tmp) = create_service().await;

        let mut neg_timeout = create_request("Job", artifact_target());
        neg_timeout.exec_timeout_secs = Some(-1);
        assert_eq!(
            service
                .create(neg_timeout)
                .await
                .expect_err("negative timeout must fail")
                .code,
            "VALIDATION_ERROR"
        );

        let mut neg_retries = create_request("Job", artifact_target());
        neg_retries.max_retries = Some(-1);
        assert_eq!(
            service
                .create(neg_retries)
                .await
                .expect_err("negative retries must fail")
                .code,
            "VALIDATION_ERROR"
        );

        let mut neg_delay = create_request("Job", artifact_target());
        neg_delay.retry_delay_secs = Some(-5);
        assert_eq!(
            service
                .create(neg_delay)
                .await
                .expect_err("negative delay must fail")
                .code,
            "VALIDATION_ERROR"
        );

        // No row was written for any rejected request.
        assert!(service
            .list(Some(10), Some(0))
            .await
            .expect("list")
            .is_empty());
    }

    #[tokio::test]
    async fn create_allows_zero_robustness_values() {
        // 0 is meaningful (no timeout / no retries) and must NOT be rejected.
        let (service, _tmp) = create_service().await;
        let mut req = create_request("Job", artifact_target());
        req.exec_timeout_secs = Some(0);
        req.max_retries = Some(0);
        req.retry_delay_secs = Some(0);
        let job = service.create(req).await.expect("create with zeros");
        assert_eq!(job.exec_timeout_secs, 0);
        assert_eq!(job.max_retries, 0);
        assert_eq!(job.retry_delay_secs, 0);
    }

    #[tokio::test]
    async fn update_rejects_negative_robustness_values() {
        // VAL-ROBUST-003 on the update path.
        let (service, _tmp) = create_service().await;
        let created = service
            .create(create_request("Job", artifact_target()))
            .await
            .expect("create");
        let err = service
            .update(
                created.id,
                JobUpdateRequest {
                    name: "Job".to_string(),
                    description: None,
                    target: artifact_target(),
                    cron_expr: "0 9 * * *".to_string(),
                    timezone: "local".to_string(),
                    enabled: true,
                    exec_timeout_secs: Some(-1),
                    max_retries: None,
                    retry_delay_secs: None,
                },
            )
            .await
            .expect_err("negative timeout on update must fail");
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
}
