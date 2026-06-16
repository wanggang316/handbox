// Scheduled-job executor.
//
// Runs one job and persists the outcome. This M1 feature only dispatches the
// `artifact` target (delegating to `ArtifactService::execute_artifact`); the
// `agent` / `prompt` targets are explicit "unsupported in M1" branches that the
// M3 features replace.
//
// One trigger produces exactly ONE `job_executions` row: a `running` row is
// inserted up front, then the SAME row is finalized in place to its terminal
// state (success / failed). Job-level run statistics (run_count / last_run_at /
// last_status / failure_count) are updated afterwards.
//
// Out of scope here (left to other milestones): the scheduler loop (M1
// scheduler feature drives this executor) and job-level timeout/retry overrides
// (M4). `next_run_at` is NOT recomputed here — that belongs to the scheduler;
// this executor preserves the job's existing `next_run_at` when writing run
// statistics.
//
// Re-entrancy ownership (M2 run-now): the executor owns the single in-flight set
// (`Arc<Mutex<HashSet<JobId>>>`) shared by EVERY trigger path. A caller (the
// scheduler tick or the `job_run_now` command) reserves a job's slot via
// [`JobExecutor::try_claim`] BEFORE dispatching; the returned [`InFlightGuard`]
// releases the slot on drop (normal return OR panic-unwind). Because both the
// scheduler and run-now claim against the SAME set on the SAME executor
// instance, a job whose execution is still in flight cannot be re-dispatched by
// any path — the at-most-one-concurrent-run guarantee. `execute` itself does NOT
// touch the set; claiming is the caller's responsibility so the scheduler can
// decide (under the claim) whether to advance `next_run_at` before dispatching.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::models::{AppError, UserMessageSendRequest};
use crate::services::coding_agent_session::{build_agent_session, config_from_rows};
use crate::services::{
    agent_jsonl_store, drive_agent_run, AgentService, AgentSessionService, ArtifactService,
    CodingRunSink, MessageService, ProviderService, SessionService,
};
use crate::storage::job_repository::{FailureCountUpdate, DEFAULT_EXECUTION_HISTORY_LIMIT};
use crate::storage::types::{
    CreateAgentSessionRequest, ExecuteArtifactRequest, ExecutionStatus, Job, JobExecution,
    JobTarget, Provider, Timestamp, Trigger,
};
use crate::storage::{JobExecutionRepository, JobRepository};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, Wry};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::{oneshot, Mutex};

/// Frontend event channel: emitted when an execution's lifecycle state changes
/// (a `running` row is written, then the SAME row reaches its terminal state).
/// The app wires an `AppHandle` so the executor — which runs on the background
/// scheduler / run-now paths, with no `Window` — can broadcast to every window.
pub const JOB_EXECUTED_EVENT: &str = "job_executed";

/// Payload of [`JOB_EXECUTED_EVENT`]. `jobId` lets the `/jobs` list refresh the
/// matching card; `executionId` lets the open detail timeline flip the matching
/// row in place (matched by id, so expansion / scroll are preserved). `status`
/// is the row's current state (`running` on start, terminal on completion).
///
/// The frontend treats the `job_execution_list` command as the source of truth
/// and uses this event only as a refresh trigger — a missed event cannot corrupt
/// state (VAL-HISTORY-030).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobExecutedEvent {
    pub job_id: String,
    pub execution_id: String,
    pub status: ExecutionStatus,
}

/// The terminal outcome of dispatching a job's target, before it is persisted.
///
/// `status` is always a terminal state (success / failed / timeout). The other
/// fields mirror the `job_executions` columns that `finalize` writes.
struct DispatchOutcome {
    status: ExecutionStatus,
    stdout: Option<String>,
    stderr: Option<String>,
    exit_code: Option<i32>,
    error: Option<String>,
    /// Opaque reference to an external result (e.g. a session id). Unused by the
    /// artifact target; reserved for the agent/prompt targets in M3.
    result_ref: Option<String>,
}

/// Runs a single job and records the result.
///
/// Generic over the Tauri `Runtime` to match `ArtifactService`; the app wiring
/// manages a `JobExecutor<Wry>` so the scheduler / run-now features can take it
/// as Tauri `State`.
pub struct JobExecutor<R: Runtime = Wry> {
    artifact_service: Arc<ArtifactService<R>>,
    jobs: JobRepository,
    executions: JobExecutionRepository,
    /// Ids of jobs with an execution currently in flight. The single shared
    /// re-entrancy gate: the scheduler tick and the `job_run_now` command both
    /// claim against this set on the SAME executor instance (cloned via its
    /// `Arc`), so a job already running cannot be re-dispatched by any path.
    /// Claimed via [`JobExecutor::try_claim`]; released by [`InFlightGuard`].
    in_flight: Arc<Mutex<HashSet<String>>>,
    /// Optional handle used to broadcast [`JOB_EXECUTED_EVENT`] to all windows
    /// when an execution starts and completes. `None` in unit tests (the
    /// `MockRuntime` setup builds the executor without one) so emit is a clean
    /// no-op and the existing executor tests are untouched; the app wiring
    /// injects a real handle via [`JobExecutor::with_app_handle`].
    app_handle: Option<AppHandle<R>>,
    /// Collaborators for the `prompt` target: create a fresh chat, send the
    /// prompt non-streaming, and pre-validate the provider. `None` in the unit
    /// wiring (`from_db`) so the artifact-only tests keep building; the app
    /// injects real handles via [`JobExecutor::with_prompt_services`]. When
    /// absent, a `prompt` dispatch fails with a stable "not configured" error
    /// rather than panicking — the same shape as any other prompt failure.
    prompt_services: Option<PromptServices>,
    /// Collaborators for the `agent` target: resolve the agent template, mint a
    /// fresh isolated agent session from it, and drive one run to completion.
    /// `None` in the unit wiring (`from_db`) so the artifact-only tests keep
    /// building; the app injects real handles via
    /// [`JobExecutor::with_agent_services`]. When absent, an `agent` dispatch
    /// fails with a stable "not configured" error rather than panicking — the
    /// same shape as any other agent failure.
    agent_services: Option<AgentServices>,
}

/// The three services the `prompt` target needs, bundled so the field stays a
/// single `Option`. All are `Arc`-shared with the app-managed instances (cheap
/// clones); none is generic over the Tauri `Runtime`.
#[derive(Clone)]
struct PromptServices {
    sessions: Arc<SessionService>,
    messages: Arc<MessageService>,
    providers: Arc<ProviderService>,
}

/// The collaborators the `agent` target needs, bundled so the field stays a
/// single `Option`. All are `Arc`-shared (cheap clones); none is generic over
/// the Tauri `Runtime`.
///
/// The native `AgentRuntime` was retired on main; the executor now drives an
/// agent run through the coding-agent [`build_agent_session`] +
/// [`drive_agent_run`](crate::services::drive_agent_run) path, exactly like the
/// foreground `agent_run_stream` command but headless (no `Window`). `agents`
/// resolves the agent template referenced by the target; `sessions` mints the
/// per-run SQLite session row from that template and reads the persisted JSONL
/// transcript back to classify the terminal outcome; `providers` resolves a
/// usable provider for the template's model (the template stores only a model
/// id) and loads the full provider row for session construction.
///
/// `app_data_dir` is the Tauri per-app data directory: it is the coding-agent
/// session's `base_dir` (where the JSONL transcript persists) and the cwd
/// fallback for a session with no working dir — the same role the `Window`'s
/// `PathResolver` plays for the foreground command, threaded in directly here
/// because the background executor has no `Window`.
#[derive(Clone)]
struct AgentServices {
    agents: Arc<AgentService>,
    sessions: Arc<AgentSessionService>,
    providers: Arc<ProviderService>,
    app_data_dir: PathBuf,
}

// Manual `Clone` so the bound is on the fields, not on `R: Clone`. Tauri
// runtimes (`Wry` / `MockRuntime`) are not themselves `Clone`, and
// `ArtifactService`'s derived `Clone` carries an `R: Clone` bound — so the
// executor holds it behind an `Arc` and clones the `Arc`, never the service.
// The in-flight set is shared (not copied) by cloning its `Arc`, so every clone
// of an executor guards the same jobs.
impl<R: Runtime> Clone for JobExecutor<R> {
    fn clone(&self) -> Self {
        Self {
            artifact_service: self.artifact_service.clone(),
            jobs: self.jobs.clone(),
            executions: self.executions.clone(),
            in_flight: self.in_flight.clone(),
            app_handle: self.app_handle.clone(),
            prompt_services: self.prompt_services.clone(),
            agent_services: self.agent_services.clone(),
        }
    }
}

impl<R: Runtime> JobExecutor<R> {
    /// Build an executor from its collaborators. All inputs are cheap
    /// (`Arc`-backed) handles.
    pub fn new(
        artifact_service: Arc<ArtifactService<R>>,
        jobs: JobRepository,
        executions: JobExecutionRepository,
    ) -> Self {
        Self {
            artifact_service,
            jobs,
            executions,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            app_handle: None,
            prompt_services: None,
            agent_services: None,
        }
    }

    /// Convenience constructor from a shared `Database` plus a shared
    /// `ArtifactService`, mirroring `JobService::from_db`. Used by the app
    /// wiring in `lib.rs`.
    pub fn from_db(
        db: Arc<crate::storage::Database>,
        artifact_service: Arc<ArtifactService<R>>,
    ) -> Self {
        Self::new(
            artifact_service,
            JobRepository::new(db.clone()),
            JobExecutionRepository::new(db),
        )
    }

    /// Attach an `AppHandle` so each execution start / completion broadcasts a
    /// [`JOB_EXECUTED_EVENT`] to all windows. Consuming builder used by the app
    /// wiring; without it (unit tests) emit is a no-op.
    pub fn with_app_handle(mut self, app_handle: AppHandle<R>) -> Self {
        self.app_handle = Some(app_handle);
        self
    }

    /// Inject the collaborators the `prompt` target needs (a fresh chat per run,
    /// a non-streaming send, and provider pre-validation). Consuming builder used
    /// by the app wiring with `Arc`s shared with the managed services; without it
    /// (artifact-only unit wiring) a `prompt` dispatch fails cleanly rather than
    /// running.
    pub fn with_prompt_services(
        mut self,
        sessions: Arc<SessionService>,
        messages: Arc<MessageService>,
        providers: Arc<ProviderService>,
    ) -> Self {
        self.prompt_services = Some(PromptServices {
            sessions,
            messages,
            providers,
        });
        self
    }

    /// Inject the collaborators the `agent` target needs (resolve the template,
    /// mint a fresh session, drive one run to completion through the coding-agent
    /// session path). Consuming builder used by the app wiring; without it
    /// (artifact-only unit wiring) an `agent` dispatch fails cleanly rather than
    /// running. `app_data_dir` is the coding-agent session's `base_dir` / cwd
    /// fallback — see [`AgentServices`].
    pub fn with_agent_services(
        mut self,
        agents: Arc<AgentService>,
        sessions: Arc<AgentSessionService>,
        providers: Arc<ProviderService>,
        app_data_dir: PathBuf,
    ) -> Self {
        self.agent_services = Some(AgentServices {
            agents,
            sessions,
            providers,
            app_data_dir,
        });
        self
    }

    /// Broadcast a [`JOB_EXECUTED_EVENT`] when one is wired. A clean no-op when
    /// no `AppHandle` is attached (unit tests) and best-effort otherwise: an
    /// emit failure is logged and swallowed, never failing the execution — the
    /// `job_execution_list` command stays the source of truth (VAL-HISTORY-030).
    fn emit_executed(&self, job_id: &str, execution_id: &str, status: ExecutionStatus) {
        let Some(handle) = self.app_handle.as_ref() else {
            return;
        };
        let payload = JobExecutedEvent {
            job_id: job_id.to_string(),
            execution_id: execution_id.to_string(),
            status,
        };
        if let Err(e) = handle.emit(JOB_EXECUTED_EVENT, payload) {
            tracing::warn!(
                "[job_executor] failed to emit {}: {}",
                JOB_EXECUTED_EVENT,
                e
            );
        }
    }

    /// Raise the continuous-failure desktop banner for `job_name`, naming the
    /// job and its consecutive-failure count.
    ///
    /// A clean no-op when no `AppHandle` is attached (unit tests build the
    /// executor without one, and the test app has no notification plugin state
    /// to resolve), so the failure-count / history logic stays fully testable
    /// without a real desktop. With a handle it is BEST-EFFORT: the banner is
    /// raised AFTER the row + job statistics are already persisted, so a missing
    /// macOS permission (or any other send error) degrades gracefully — it is
    /// logged and swallowed, never panicking and never blocking the execution
    /// flow (VAL-ROBUST-021). The persisted failure_count / history are
    /// unaffected by whether the banner reaches the screen.
    fn notify_failure_threshold(&self, job_name: &str, failure_count: i32) {
        let Some(handle) = self.app_handle.as_ref() else {
            // Headless / unit wiring: the decision to notify was made, but there
            // is no desktop to show it on. Log so the path is observable.
            tracing::info!(
                "[job_executor] failure-threshold notification suppressed (no AppHandle): job '{}' failed {} times",
                job_name,
                failure_count
            );
            return;
        };

        let result = handle
            .notification()
            .builder()
            .title(FAILURE_NOTIFY_TITLE)
            .body(failure_notify_body(job_name, failure_count))
            .show();

        match result {
            Ok(()) => {
                tracing::info!(
                    "[job_executor] raised continuous-failure notification for job '{}' ({} consecutive failures)",
                    job_name,
                    failure_count
                );
            }
            Err(e) => {
                // Graceful degradation: no banner is shown (e.g. macOS
                // notification permission not granted), but the failure was
                // already recorded. Surface a hint on stdout/log; do NOT block
                // or fail the run (VAL-ROBUST-021).
                tracing::warn!(
                    "[job_executor] failure-threshold notification not delivered for job '{}' (permission missing or notification error): {}",
                    job_name,
                    e
                );
            }
        }
    }

    /// Try to reserve `job_id`'s in-flight slot, returning an [`InFlightGuard`]
    /// that releases it on drop, or `None` when the job is already in flight.
    ///
    /// This is the single re-entrancy primitive shared by every trigger path.
    /// The caller holds the guard for the lifetime of the dispatch (the
    /// scheduler moves it into the detached run task; `run_now` holds it across
    /// the awaited `execute`). `execute` itself does NOT claim — the caller
    /// reserves the slot first so it can act under the reservation (e.g. the
    /// scheduler advances `next_run_at` only after a successful claim).
    pub async fn try_claim(&self, job_id: &str) -> Option<InFlightGuard> {
        let claimed = self.in_flight.lock().await.insert(job_id.to_string());
        if claimed {
            Some(InFlightGuard::new(
                self.in_flight.clone(),
                job_id.to_string(),
            ))
        } else {
            None
        }
    }

    /// Manually run `job` now (`Trigger::Manual`), honoring the shared
    /// re-entrancy gate.
    ///
    /// Claims the job's in-flight slot first: if an execution is already in
    /// flight (scheduled OR manual), this returns a `CONFLICT` `AppError`
    /// WITHOUT writing a second row — the at-most-one-concurrent-run guarantee.
    /// Otherwise it dispatches via [`execute`], holding the claim across the
    /// awaited run so a concurrent tick or run-now cannot also fire it. Disabled
    /// jobs are runnable here on purpose: `enabled = false` only stops automatic
    /// scheduling, never an explicit manual run.
    pub async fn run_now(&self, job: &Job) -> Result<JobExecution, AppError> {
        let _guard = self.try_claim(&job.id).await.ok_or_else(|| {
            // A `CONFLICT` rather than a failed execution row: nothing ran, so
            // we must NOT write a second row (VAL-HISTORY-028). The frontend
            // also disables the button while a run is in flight; this is the
            // server-side backstop against a racing double-click.
            AppError::with_hint(
                "CONFLICT",
                &format!("Job '{}' already has an execution in flight", job.id),
                "请等待当前运行结束后再试",
            )
        })?;
        // The guard is held until this scope ends — i.e. until `execute`
        // resolves — so the slot stays reserved for the whole run and is
        // released on both the success and error paths (including a panic).
        self.execute(job, Trigger::Manual).await
    }

    /// Execute `job` and persist the outcome, retrying a failed/timed-out run
    /// with exponential backoff up to the job's `max_retries`.
    ///
    /// The whole retry envelope is ONE `job_executions` row (VAL-HISTORY-032):
    /// a single `running` row is inserted up front, the target is dispatched up
    /// to `max_retries + 1` times inside that row, and the SAME row is finalized
    /// once to its terminal state carrying the FINAL `attempt`. Job statistics
    /// are updated once per envelope: `run_count + 1` always (one trigger),
    /// while `failure_count` (the continuous-failure counter) is reset on a
    /// terminal success and incremented on a terminal failure — but only for a
    /// scheduled trigger; a manual trigger leaves `failure_count` untouched
    /// (VAL-ROBUST-024).
    ///
    /// Attempt numbering starts at 1; `max_retries = N` allows up to `N + 1`
    /// attempts. Between attempt `k` and `k + 1` the task sleeps
    /// `retry_delay_secs * 2^(k-1)` seconds (base, 2·base, 4·base, …); a
    /// `retry_delay_secs = 0` collapses every backoff to zero so the envelope
    /// still converges in a bounded number of attempts without busy-looping
    /// forever (VAL-ROBUST-012). The sleep lives inside this task, so an app
    /// shutdown that drops the task discards a pending backoff — a late retry is
    /// never replayed on restart (VAL-ROBUST-014).
    ///
    /// Re-entrancy is the CALLER's responsibility: callers that need the
    /// at-most-one-concurrent-run guarantee must hold a slot from
    /// [`try_claim`] (or call [`run_now`], which does) for the duration of this
    /// call — which now spans the WHOLE envelope (every attempt and every
    /// backoff sleep), so a tick that fires mid-backoff is skipped by the
    /// in-flight guard rather than opening a second row (VAL-ROBUST-013/015).
    ///
    /// A dispatch failure (artifact missing / not installed, process that fails
    /// to start, a timeout, or an unsupported target) is NOT propagated as
    /// `Err`: it is recorded as a terminal execution row with a non-empty
    /// `error`, and the finalized row is returned. `Err` is reserved for
    /// persistence failures where no consistent row could be written.
    ///
    /// If the job is deleted mid-envelope (between attempts), the envelope is
    /// aborted cleanly: the `running` row was cascade-deleted with the job
    /// (FK `ON DELETE CASCADE`), so there is nothing to finalize and no new
    /// execution is started (VAL-ROBUST-025). The deletion is surfaced as an
    /// `Err(not_found)`.
    pub async fn execute(&self, job: &Job, trigger: Trigger) -> Result<JobExecution, AppError> {
        let exec_id = uuid::Uuid::new_v4().to_string();
        let started_at = current_timestamp();
        const FIRST_ATTEMPT: i32 = 1;
        // `max_retries = N` => up to N+1 attempts (the first run plus N retries).
        // Clamp defensively so a negative value cannot underflow the loop bound.
        let max_attempts: i32 = 1 + job.max_retries.max(0) as i32;

        self.executions
            .insert_running(
                &exec_id,
                &job.id,
                trigger,
                FIRST_ATTEMPT,
                started_at,
                started_at,
            )
            .await?;

        // Tell an open detail timeline a `running` row exists now, so it can be
        // shown immediately and later flipped in place to its terminal state.
        self.emit_executed(&job.id, &exec_id, ExecutionStatus::Running);

        // Dispatch up to `max_attempts` times inside the single row. A success
        // (or running out of attempts) ends the loop; between attempts we back
        // off exponentially and re-check the job still exists.
        let mut attempt: i32 = FIRST_ATTEMPT;
        let outcome = loop {
            let outcome = self.dispatch_with_timeout(job).await;

            // A success terminates the envelope immediately, keeping the attempt
            // number it succeeded on (preserving the failure trail, ROBUST-016).
            if matches!(outcome.status, ExecutionStatus::Success) {
                break outcome;
            }

            // No attempts left: this failure/timeout is the terminal outcome.
            if attempt >= max_attempts {
                break outcome;
            }

            // Back off before the next attempt: base * 2^(attempt-1). A zero
            // base yields zero delay (bounded, no busy-loop). The sleep is in
            // this task, so a shutdown drops it (no late replay, ROBUST-014).
            let delay = backoff_delay(job.retry_delay_secs, attempt);
            tracing::info!(
                "[job_executor] job {} attempt {} failed ({:?}); retrying in {:?}",
                job.id,
                attempt,
                outcome.status,
                delay
            );
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }

            // If the job was deleted during the backoff, the running row was
            // cascade-deleted with it. Abort cleanly: do not start another
            // attempt and do not try to finalize a row that no longer exists
            // (VAL-ROBUST-025).
            match self.jobs.get(&job.id).await {
                Ok(Some(_)) => {}
                Ok(None) => {
                    return Err(AppError::not_found(&format!(
                        "Job '{}' was deleted during a retry; envelope aborted",
                        job.id
                    )));
                }
                Err(e) => return Err(e),
            }

            attempt += 1;
        };

        let ended_at = current_timestamp();
        let duration = (ended_at - started_at).max(0);

        // Finalize the SAME row to its terminal state, recording the FINAL
        // attempt the envelope reached.
        self.executions
            .finalize(
                &exec_id,
                outcome.status,
                attempt,
                outcome.stdout.as_deref(),
                outcome.stderr.as_deref(),
                outcome.exit_code,
                outcome.error.as_deref(),
                outcome.result_ref.as_deref(),
                ended_at,
                duration,
            )
            .await?;

        // Update job-level run statistics. `run_count` advances once per
        // envelope. `failure_count` is the CONTINUOUS-failure counter: a
        // scheduled success resets it, a scheduled failure increments it, and a
        // manual run never touches it (VAL-ROBUST-017/018/024). `next_run_at` is
        // preserved as-is; the scheduler owns cron recomputation.
        let failure_update = failure_count_update(trigger, outcome.status);
        self.jobs
            .update_after_run(
                &job.id,
                ended_at,
                outcome.status,
                failure_update,
                job.next_run_at,
                ended_at,
            )
            .await?;

        // Continuous-failure desktop notification. Read the AUTHORITATIVE new
        // `failure_count` back from the row just written, rather than deriving it
        // from the in-memory `job` (whose snapshot can be stale across repeated
        // calls): the persisted counter is the single source of truth, so the
        // `== threshold` crossing is detected correctly however the caller holds
        // the job. Fires once when the chain first hits the threshold, stays
        // silent on further failures, and re-arms after a success resets the
        // counter (VAL-ROBUST-019/020). A read failure here must not fail the
        // run (the row is already finalized): log and skip the notification.
        match self.jobs.get(&job.id).await {
            Ok(Some(updated)) => {
                if should_notify_failure(updated.failure_count, FAILURE_NOTIFY_THRESHOLD, trigger) {
                    self.notify_failure_threshold(&updated.name, updated.failure_count);
                }
            }
            Ok(None) => {
                // The job was deleted between the stats write and this read — no
                // notification target remains. Nothing to do.
            }
            Err(e) => {
                tracing::warn!(
                    "[job_executor] could not re-read job '{}' to evaluate failure notification: {e:?}",
                    job.id
                );
            }
        }

        // FIFO-prune this job's execution history to the most recent N rows. Run
        // AFTER finalize so the just-written row is terminal (never pruned as a
        // running row) and the persisted count for this job stays <= N — no
        // transient N+1 is exposed. `prune_to` is per-job and never deletes
        // running rows, so a concurrent in-flight execution survives.
        self.executions
            .prune_to(&job.id, DEFAULT_EXECUTION_HISTORY_LIMIT)
            .await?;

        // All writes are settled (row finalized + job stats updated + history
        // pruned): tell an open timeline to flip the SAME row to its terminal
        // state in place and the `/jobs` list to refresh the matching card.
        self.emit_executed(&job.id, &exec_id, outcome.status);

        Ok(JobExecution {
            id: exec_id,
            job_id: job.id.clone(),
            status: outcome.status,
            trigger,
            attempt,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            exit_code: outcome.exit_code,
            error: outcome.error,
            result_ref: outcome.result_ref,
            started_at,
            ended_at: Some(ended_at),
            duration: Some(duration),
            created_at: started_at,
        })
    }

    /// Dispatch a job's target under the job's `exec_timeout_secs` bound.
    ///
    /// The timeout is enforced on the EXECUTION side (here), decoupled from the
    /// scheduler's 30s tick, so a `timeout > tick interval` is honored at the
    /// threshold rather than at the next tick (VAL-ROBUST-007). `0` means no
    /// bound — the dispatch runs to its natural end (VAL-ROBUST-008).
    ///
    /// When the bound elapses the execution is interrupted near the threshold
    /// and recorded as a `timeout` outcome (VAL-ROBUST-004). Orphan cleanup is
    /// per-target:
    /// - artifact: the spawned `tokio::process::Command` carries
    ///   `kill_on_drop(true)`, so dropping the timed-out future kills the OS
    ///   child — no orphan process (VAL-ROBUST-005). The job-level bound here is
    ///   the single upper limit and overrides the artifact's built-in 30s
    ///   `ExecutionConfig.timeout` (when `>0`), and a hit is recorded as
    ///   `timeout`, NOT the artifact's generalized `failed` (VAL-TARGET-036).
    /// - prompt: dropping the future cancels the in-flight non-streaming send;
    ///   the chat and (already-persisted) user message stay reachable, no
    ///   running session is left behind (VAL-ROBUST-006 / VAL-ROBUST-022).
    /// - agent: the run is driven inside `dispatch_agent`, which on timeout
    ///   issues the cooperative `abort_run` for the minted session so the agent
    ///   loop unwinds and the driver fires its single `closed`, and the same job
    ///   can be triggered again — no orphan running turn (VAL-ROBUST-006 /
    ///   VAL-ROBUST-022).
    ///
    /// Never returns `Err`: a failed or timed-out dispatch is a terminal
    /// `DispatchOutcome` so the caller can finalize one consistent row.
    async fn dispatch_with_timeout(&self, job: &Job) -> DispatchOutcome {
        let timeout = timeout_duration(job.exec_timeout_secs);

        match &job.target {
            JobTarget::Artifact {
                artifact_id,
                args,
                env,
            } => {
                let dispatch = self.dispatch_artifact(artifact_id, args, env);
                match timeout {
                    // Dropping the timed-out future drops the artifact's
                    // `Command::output` future; `kill_on_drop(true)` reaps the
                    // OS child (VAL-ROBUST-005).
                    Some(dur) => match tokio::time::timeout(dur, dispatch).await {
                        Ok(outcome) => outcome,
                        Err(_) => DispatchOutcome::timeout(job.exec_timeout_secs),
                    },
                    None => dispatch.await,
                }
            }
            JobTarget::Agent {
                agent_id,
                initial_message,
                project_id,
            } => {
                // The agent path takes the bound directly: only it knows the
                // minted session id needed for the cooperative abort on timeout.
                self.dispatch_agent(agent_id, initial_message, project_id.as_deref(), timeout)
                    .await
            }
            JobTarget::Prompt {
                provider_id,
                model_id,
                prompt,
                ..
            } => {
                let dispatch = self.dispatch_prompt(&job.name, provider_id, model_id, prompt);
                match timeout {
                    // Dropping the timed-out future cancels the in-flight send;
                    // the chat + persisted user message remain reachable, no
                    // running session leaks (VAL-ROBUST-006).
                    Some(dur) => match tokio::time::timeout(dur, dispatch).await {
                        Ok(outcome) => outcome,
                        Err(_) => DispatchOutcome::timeout(job.exec_timeout_secs),
                    },
                    None => dispatch.await,
                }
            }
        }
    }

    /// Run an artifact target through `ArtifactService::execute_artifact`.
    ///
    /// `args` / `env` come straight from the typed `JobTarget::Artifact` and are
    /// passed as a single argv vector (no shell), so values with spaces or
    /// quotes cannot inject extra commands. Empty `args` / `env` are sent as
    /// `None` so the artifact's own `execution_config` defaults apply.
    async fn dispatch_artifact(
        &self,
        artifact_id: &str,
        args: &[String],
        env: &std::collections::HashMap<String, String>,
    ) -> DispatchOutcome {
        let request = ExecuteArtifactRequest {
            artifact_id: artifact_id.to_string(),
            args: if args.is_empty() {
                None
            } else {
                Some(args.to_vec())
            },
            env: if env.is_empty() {
                None
            } else {
                Some(env.clone())
            },
        };

        match self.artifact_service.execute_artifact(request).await {
            // `execute_artifact` returns `Ok` for both ran-and-exited and
            // spawn-failure cases; the `success` flag encodes which.
            Ok(result) => {
                let status = if result.success {
                    ExecutionStatus::Success
                } else {
                    ExecutionStatus::Failed
                };
                DispatchOutcome {
                    status,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    exit_code: result.exit_code,
                    error: result.error,
                    result_ref: None,
                }
            }
            // `Err` here is a pre-flight failure (artifact missing, not
            // installed, sandbox path errors): no process ran.
            Err(e) => DispatchOutcome::failed(e.message),
        }
    }

    /// Run a `prompt` target: create a fresh chat, send the prompt text through
    /// `MessageService::send_user_message` (non-streaming), and persist a user
    /// message plus an assistant reply. On success the outcome's `result_ref`
    /// points at the chat; on failure it still points at the chat IF one was
    /// created, so a partial transcript stays reachable (VAL-TARGET-023).
    ///
    /// SECURITY: the provider is pre-validated (deleted / disabled / missing key)
    /// and every error is run through [`sanitize_send_error`] /
    /// [`provider_failure_message`] before it is persisted, so no raw upstream
    /// URL, `Authorization` header, or API key fragment can reach the
    /// `job_executions.error` column or any window — raw detail goes to
    /// `tracing` only (VAL-TARGET-026 / VAL-TARGET-027).
    async fn dispatch_prompt(
        &self,
        job_name: &str,
        provider_id: &str,
        model_id: &str,
        prompt: &str,
    ) -> DispatchOutcome {
        let Some(services) = self.prompt_services.as_ref() else {
            // No prompt collaborators wired (artifact-only unit harness): fail
            // cleanly with a stable, non-leaking message rather than panic.
            return DispatchOutcome::failed(PROMPT_NOT_CONFIGURED.to_string());
        };

        // 1. Pre-validate the provider: deleted / disabled / missing key are all
        //    config failures we can name precisely without touching the network.
        let provider_result = services
            .providers
            .get_provider(&provider_id.to_string())
            .await;
        if let Some(failure) = classify_provider(provider_result.as_ref()) {
            if let Err(e) = &provider_result {
                tracing::warn!(
                    "[job_executor] prompt provider check failed (provider={}): {}",
                    provider_id,
                    e
                );
            }
            return DispatchOutcome::failed(provider_failure_message(failure));
        }

        // 2. Create a fresh, isolated chat for this run (never reused — two jobs
        //    in the same tick get distinct chats, VAL-TARGET-034). No `Window`
        //    is required, so this works headless (VAL-TARGET-033).
        let chat = match services
            .sessions
            .create_chat(
                prompt_chat_name(job_name),
                None,
                None,
                None,
                None,
                Some(false), // non-streaming: send_user_message returns the full reply
                Some(model_id.to_string()),
                Some(provider_id.to_string()),
                None,
                None,
            )
            .await
        {
            Ok(chat) => chat,
            Err(e) => {
                tracing::warn!(
                    "[job_executor] prompt chat creation failed (provider={}): {}",
                    provider_id,
                    e
                );
                // No chat exists, so there is nothing to reference.
                return DispatchOutcome::failed(sanitize_send_error(&e));
            }
        };

        // 3. Send the prompt. `send_user_message` persists the `user` message
        //    first, then the `assistant` reply; a failure after the user message
        //    is saved leaves a partial chat (user, no assistant) that the outcome
        //    still references (VAL-TARGET-023).
        let request = UserMessageSendRequest {
            chat_id: chat.id.clone(),
            content: prompt.to_string(),
            temp_user_message_id: String::new(),
            attachments: None,
        };

        match services.messages.send_user_message(request).await {
            Ok(response) => DispatchOutcome {
                status: ExecutionStatus::Success,
                stdout: Some(response.content),
                stderr: None,
                exit_code: None,
                error: None,
                // The run's result is the chat it produced.
                result_ref: Some(chat.id),
            },
            Err(e) => {
                tracing::warn!(
                    "[job_executor] prompt send failed (chat={}, provider={}): {}",
                    chat.id,
                    provider_id,
                    e
                );
                DispatchOutcome {
                    status: ExecutionStatus::Failed,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    error: Some(sanitize_send_error(&e)),
                    // The (possibly partial) chat is still reachable.
                    result_ref: Some(chat.id),
                }
            }
        }
    }

    /// Run an `agent` target: resolve the agent template, mint a fresh isolated
    /// agent session from it, drive ONE run to completion through the
    /// coding-agent session path, then classify the terminal outcome from the
    /// persisted JSONL transcript.
    ///
    /// Flow (VAL-TARGET-006 / 020 / 021 / 024 / 031):
    /// 1. resolve the template (`AgentService::get_agent`); a missing template is
    ///    a distinct "template missing" failure with NO session created
    ///    (VAL-TARGET-020);
    /// 2. resolve a usable provider for the template's model — the template
    ///    stores only a model id — failing pre-flight with a model/config-class
    ///    error (distinct from "template missing") when the model is unset or no
    ///    enabled provider serves it (VAL-TARGET-021), again with NO session
    ///    created;
    /// 3. load the resolved provider's row (needed to construct the coding-agent
    ///    session); a provider that vanished between resolution and load is a
    ///    config-class failure, still with no session;
    /// 4. mint a fresh isolated SQLite session row carrying the template's model +
    ///    the resolved provider + system prompt / sampling config — never reused,
    ///    so two jobs in the same tick get distinct sessions and the
    ///    one-run-per-session race is moot;
    /// 5. construct a coding-agent `AgentSession` from that row
    ///    ([`config_from_rows`] + [`build_agent_session`], headless: no approval
    ///    emitter, so the dangerous-tool gate fails CLOSED — the safe default for
    ///    an unattended run), build a oneshot-signalling [`CodingRunSink`] (its
    ///    `on_closed` fires the oneshot AFTER the turn closes; `on_error`
    ///    captures the sanitized run-level envelope), drive ONE run via
    ///    [`drive_agent_run`](crate::services::drive_agent_run), then await the
    ///    oneshot to block until the turn ends;
    /// 6. classify: a run-level error envelope (`on_error`, e.g. the provider /
    ///    model was removed) OR an in-band-error terminal assistant turn
    ///    (`stopReason == "error"`, VAL-TARGET-024) is `failed`; otherwise
    ///    `success`. In EVERY post-session outcome `result_ref` points at the
    ///    minted session so its (possibly partial) transcript stays reachable.
    ///
    /// SECURITY: every persisted error is a sanitized, stable message. A
    /// run-level envelope is already sanitized by `drive_agent_run`
    /// (`sanitize_coding_agent_error`); a construction `AppError` is run through
    /// [`sanitize_agent_dispatch_error`], so no raw upstream URL, header, or key
    /// fragment can reach `job_executions.error` — raw detail goes to `tracing`
    /// only.
    ///
    /// `timeout` is the job's `exec_timeout_secs` bound (`None` = unbounded).
    /// When set, only the RUN itself (`drive_agent_run` + awaiting the close
    /// signal) is bounded; the offline pre-flight (template / provider / session
    /// construction) is fast and intentionally not counted. On elapse the run's
    /// cooperative abort is issued through [`abort_run`](crate::services::abort_run)
    /// so the agent loop unwinds at its next await point, synthesizes a
    /// `stopReason=aborted` terminal turn, and the driver fires the single
    /// `closed` — no orphan running turn, the job can fire again. A `timeout`
    /// outcome is returned with `result_ref` pointing at the minted session
    /// (VAL-ROBUST-006 / VAL-ROBUST-022).
    async fn dispatch_agent(
        &self,
        agent_id: &str,
        initial_message: &str,
        project_id: Option<&str>,
        timeout: Option<Duration>,
    ) -> DispatchOutcome {
        let Some(services) = self.agent_services.as_ref() else {
            // No agent collaborators wired (artifact-only unit harness): fail
            // cleanly with a stable, non-leaking message rather than panic.
            return DispatchOutcome::failed(AGENT_NOT_CONFIGURED.to_string());
        };

        // 1. Resolve the agent template. A missing template is a distinct
        //    failure class (VAL-TARGET-020) — no session is created.
        let agent = match services.agents.get_agent(agent_id.to_string()).await {
            Ok(agent) => agent,
            Err(e) => {
                tracing::warn!(
                    "[job_executor] agent template lookup failed (agent={}): {}",
                    agent_id,
                    e
                );
                return DispatchOutcome::failed(agent_failure_message(
                    AgentFailure::TemplateMissing,
                ));
            }
        };

        // 2. Resolve a usable provider for the template's model. The template
        //    stores only a model id, so we find an enabled provider whose model
        //    catalog still serves it. An unset model or a removed provider/model
        //    is a model/config-class failure (VAL-TARGET-021), distinct from a
        //    missing template — and still no session is created.
        let provider_id = match self.resolve_agent_provider(&agent.model).await {
            Ok(provider_id) => provider_id,
            Err(failure) => return DispatchOutcome::failed(agent_failure_message(failure)),
        };
        // `resolve_agent_provider` only returns `Ok` when `agent.model` is set.
        let model_id = agent.model.clone().unwrap_or_default();

        // 3. Load the resolved provider's row — `build_agent_session` needs the
        //    full record (type / base_url / key). A provider that vanished
        //    between resolution and load is a config-class failure, still with no
        //    session created.
        let provider = match services.providers.get_provider(&provider_id).await {
            Ok(provider) => provider,
            Err(e) => {
                tracing::warn!(
                    "[job_executor] agent provider load failed (provider={}, agent={}): {}",
                    provider_id,
                    agent_id,
                    e
                );
                return DispatchOutcome::failed(agent_failure_message(AgentFailure::ConfigError));
            }
        };

        // 4. Mint a fresh, isolated SQLite session row from the template
        //    (VAL-TARGET-006). It carries the resolved model + provider so the
        //    coding-agent session built from it resolves the same pair, plus the
        //    template's system prompt / sampling config.
        let request = CreateAgentSessionRequest {
            name: agent_session_name(&agent.name),
            project_id: project_id.map(str::to_string),
            model_id: Some(model_id),
            provider_id: Some(provider_id),
            system_prompt: agent.system_prompt.clone(),
            thinking_level: None,
            temperature: agent.temperature,
            max_tokens: agent.max_tokens,
            working_dir: None,
            enabled_tools: None,
            tool_execution_mode: None,
        };
        let session = match services.sessions.create_session(request).await {
            Ok(session) => session,
            Err(e) => {
                tracing::warn!(
                    "[job_executor] agent session creation failed (agent={}): {}",
                    agent_id,
                    e
                );
                // No session exists, so there is nothing to reference.
                return DispatchOutcome::failed(sanitize_agent_dispatch_error(&e));
            }
        };
        // Every post-creation outcome references the minted session so its
        // (possibly partial) transcript stays reachable.
        let result_ref = Some(session.id.clone());

        // 5. Construct the coding-agent session from the minted row + provider.
        //    Headless: no approval emitter, so the dangerous-tool gate fails
        //    CLOSED (write/edit/bash denied without prompting) — the safe default
        //    for an unattended scheduled run.
        let config = match config_from_rows(&session, &provider, services.app_data_dir.clone()) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    "[job_executor] agent session config assembly failed (session={}, agent={}): {}",
                    session.id,
                    agent_id,
                    e
                );
                return DispatchOutcome {
                    status: ExecutionStatus::Failed,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    error: Some(sanitize_agent_dispatch_error(&e)),
                    result_ref,
                };
            }
        };
        let coding_session = match build_agent_session(&config, None) {
            Ok(coding_session) => coding_session,
            Err(e) => {
                tracing::warn!(
                    "[job_executor] agent session construction failed (session={}, agent={}): {}",
                    session.id,
                    agent_id,
                    e
                );
                return DispatchOutcome {
                    status: ExecutionStatus::Failed,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    error: Some(sanitize_agent_dispatch_error(&e)),
                    result_ref,
                };
            }
        };

        // Build a oneshot-signalling sink and drive ONE run to completion. The
        // sink's `on_closed` fires the oneshot AFTER the driver's single closed
        // emit (which fires exactly once for both Ok and Err); `on_error`
        // captures the run-level envelope (already sanitized by the driver).
        let (sink, signal) = oneshot_run_sink();
        let handles = drive_agent_run(
            coding_session,
            session.id.clone(),
            initial_message.to_string(),
            Vec::new(),
            sink,
        );

        // Block until the turn ends. The oneshot resolves from `on_closed`,
        // which the driver fires exactly once. A `RecvError` means the sink was
        // dropped without closing (should not happen given closed-once); treat
        // it as a completed-but-unknown run and fall through to transcript
        // classification.
        //
        // Under a timeout bound the wait is capped at the threshold: on elapse
        // the cooperative `abort_run` flips the same cancel token the driving
        // `send_message` is on, so the agent loop unwinds at its next await
        // point, synthesizes a `stopReason=aborted` terminal turn, and the
        // driver fires the single `closed` (closed-once holds on the abort path
        // too) — no orphan running turn, the job can fire again. The minted
        // session stays referenced (VAL-ROBUST-006 / VAL-ROBUST-022).
        let run_error = match timeout {
            Some(dur) => match tokio::time::timeout(dur, signal).await {
                Ok(result) => result.unwrap_or(None),
                Err(_) => {
                    crate::services::abort_run(&session.id);
                    return DispatchOutcome {
                        status: ExecutionStatus::Timeout,
                        stdout: None,
                        stderr: None,
                        exit_code: None,
                        error: Some(timeout_error_message(timeout_secs_of(dur))),
                        result_ref,
                    };
                }
            },
            None => signal.await.unwrap_or(None),
        };
        // The driver task owns the run; dropping its handle detaches the (now
        // finished) task without aborting it.
        drop(handles);

        // 6. Classify the terminal outcome.
        if let Some(envelope_error) = run_error {
            // Run-level error envelope (e.g. provider/model removed). Already
            // sanitized by the driver (`sanitize_coding_agent_error`).
            return DispatchOutcome {
                status: ExecutionStatus::Failed,
                stdout: None,
                stderr: None,
                exit_code: None,
                error: Some(envelope_error),
                result_ref,
            };
        }

        // Read the persisted JSONL transcript to detect an in-band error
        // terminal turn (VAL-TARGET-024): the run returns `Ok` but the final
        // assistant message carries `stopReason == "error"`. The transcript
        // lives under the session's `base_dir` (app_data_dir) keyed by its cwd
        // (the writer side, `config_from_rows`), so we resolve the same cwd here.
        let transcript = self.read_agent_transcript(services, &session);
        match classify_agent_transcript(transcript.as_ref()) {
            AgentRunResult::Success => DispatchOutcome {
                status: ExecutionStatus::Success,
                stdout: None,
                stderr: None,
                exit_code: None,
                error: None,
                result_ref,
            },
            AgentRunResult::InBandError => DispatchOutcome {
                status: ExecutionStatus::Failed,
                stdout: None,
                stderr: None,
                exit_code: None,
                error: Some(AGENT_IN_BAND_ERROR.to_string()),
                result_ref,
            },
        }
    }

    /// Read a minted session's persisted JSONL transcript for terminal-outcome
    /// classification.
    ///
    /// M3 made JSONL the authoritative transcript store; the coding-agent session
    /// the run drove appends its turns to `<app_data_dir>/sessions/
    /// <flattened-cwd>/<session_id>.jsonl`. The cwd is resolved exactly as the
    /// writer side does ([`config_from_rows`] roots a session with no working dir
    /// at `app_data_dir`), so the reader looks in the same `<flattened-cwd>`
    /// directory the writer used. A read failure (or an absent file) yields an
    /// `Err`/empty list that [`classify_agent_transcript`] treats as a non-error
    /// completion — a run-level failure would have surfaced through the sink
    /// envelope and never reached here.
    fn read_agent_transcript(
        &self,
        services: &AgentServices,
        session: &crate::storage::types::AgentSession,
    ) -> Result<Vec<crate::storage::types::AgentSessionMessage>, AppError> {
        let cwd =
            agent_jsonl_store::session_cwd(session.working_dir.as_deref(), &services.app_data_dir);
        agent_jsonl_store::load_transcript(&services.app_data_dir, &cwd, &session.id)
            .map(|opt| opt.unwrap_or_default())
    }

    /// Resolve a usable provider id for an agent template's `model`.
    ///
    /// The agent template stores only a model id (no provider). We find an
    /// enabled provider whose model catalog still serves that exact id, matching
    /// how an agent session is launched from the UI (a model is always picked
    /// together with its provider). Resolution is offline (DB catalog only):
    /// `Err` carries the precise failure class for VAL-TARGET-021 — `NoModel`
    /// when the template has no model set, `ModelRemoved` when no provider serves
    /// it. An enabled provider is preferred; a match found only under a disabled
    /// provider still surfaces as a config failure (the run could not proceed).
    async fn resolve_agent_provider(&self, model: &Option<String>) -> Result<String, AgentFailure> {
        let Some(services) = self.agent_services.as_ref() else {
            return Err(AgentFailure::ConfigError);
        };
        let Some(model_id) = model.as_deref().map(str::trim).filter(|m| !m.is_empty()) else {
            return Err(AgentFailure::NoModel);
        };

        let providers = match services.providers.list_providers().await {
            Ok(providers) => providers,
            Err(e) => {
                tracing::warn!("[job_executor] agent provider listing failed: {}", e);
                return Err(AgentFailure::ConfigError);
            }
        };

        // Prefer an enabled provider that serves the model; fall back to noting a
        // disabled-only match so the failure stays a config class rather than a
        // generic "model removed".
        let mut disabled_match = false;
        for provider in &providers {
            match services.providers.get_model(&provider.id, model_id).await {
                Ok(Some(_)) if provider.enabled => return Ok(provider.id.clone()),
                Ok(Some(_)) => disabled_match = true,
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(
                        "[job_executor] agent model lookup failed (provider={}, model={}): {}",
                        provider.id,
                        model_id,
                        e
                    );
                }
            }
        }

        if disabled_match {
            Err(AgentFailure::ConfigError)
        } else {
            Err(AgentFailure::ModelRemoved)
        }
    }

    /// Test-only view of the shared in-flight set size, so the scheduler and
    /// run-now tests can assert a slot is reserved / released.
    #[cfg(test)]
    pub(crate) async fn in_flight_len(&self) -> usize {
        self.in_flight.lock().await.len()
    }
}

impl DispatchOutcome {
    /// A failed outcome carrying only an `error` message (no process output).
    fn failed(error: String) -> Self {
        Self {
            status: ExecutionStatus::Failed,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some(error),
            result_ref: None,
        }
    }

    /// A `timeout` outcome for a dispatch that exceeded the job's
    /// `exec_timeout_secs` bound. Carries a stable timeout `error` naming the
    /// threshold; no process output. Used by the artifact and prompt paths
    /// (the agent path builds its own timeout outcome so it can attach the
    /// minted session as `result_ref`).
    fn timeout(timeout_secs: i64) -> Self {
        Self {
            status: ExecutionStatus::Timeout,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some(timeout_error_message(timeout_secs)),
            result_ref: None,
        }
    }
}

/// Convert a job's `exec_timeout_secs` into an enforcement `Duration`: `0` (or
/// negative, defensively) means no bound (`None`); any positive value is a
/// `Duration` of that many seconds.
fn timeout_duration(exec_timeout_secs: i64) -> Option<Duration> {
    if exec_timeout_secs > 0 {
        Some(Duration::from_secs(exec_timeout_secs as u64))
    } else {
        None
    }
}

/// Exponential backoff before the retry that follows `attempt`: the gap before
/// attempt `k + 1` is `retry_delay_secs * 2^(k-1)` seconds (so base, 2·base,
/// 4·base, … between successive attempts). `attempt` is the 1-based number of
/// the attempt that just failed.
///
/// A `retry_delay_secs <= 0` yields a zero delay (no inter-attempt wait), so a
/// `retry_delay_secs = 0` job still retries — back to back — and converges in a
/// bounded number of attempts rather than busy-looping forever
/// (VAL-ROBUST-012). The exponent is saturated so a large `attempt` cannot
/// overflow the shift; the resulting seconds are saturated into the `Duration`.
fn backoff_delay(retry_delay_secs: i64, attempt: i32) -> Duration {
    if retry_delay_secs <= 0 || attempt < 1 {
        return Duration::ZERO;
    }
    let base = retry_delay_secs as u64;
    // 2^(attempt-1); clamp the exponent so the shift never overflows.
    let exponent = (attempt - 1).clamp(0, 62) as u32;
    let multiplier = 1u64 << exponent;
    let secs = base.saturating_mul(multiplier);
    Duration::from_secs(secs)
}

/// Number of CONSECUTIVE scheduled failures that arms a single desktop
/// notification. A job's `failure_count` (the continuous-failure counter)
/// crossing exactly this value raises one banner; subsequent failures in the
/// same chain stay silent until a success resets the counter and the chain
/// climbs back across the threshold (VAL-ROBUST-019/020).
const FAILURE_NOTIFY_THRESHOLD: i32 = 3;

/// Whether a finalized envelope should raise the continuous-failure desktop
/// notification, given the job's NEW (post-update) `failure_count`.
///
/// Fires exactly once per failure chain: a scheduled failure increments
/// `failure_count` by exactly 1, so the new value equals `threshold` on one and
/// only one envelope — the moment the chain first crosses it. The 4th, 5th, …
/// failures land on `threshold + 1`, `threshold + 2`, … and stay silent
/// (VAL-ROBUST-019). A terminal success resets the counter to 0, so a fresh
/// chain climbing back to `threshold` fires again — the throttle re-arms with
/// the reset, no extra state required (VAL-ROBUST-020).
///
/// A MANUAL trigger never participates: it leaves `failure_count` untouched, so
/// it can never produce the `== threshold` crossing. The explicit trigger guard
/// makes that intent unmistakable rather than relying on the counter alone.
fn should_notify_failure(new_failure_count: i32, threshold: i32, trigger: Trigger) -> bool {
    matches!(trigger, Trigger::Schedule) && new_failure_count == threshold
}

/// Decide how a finalized envelope should move the job's CONTINUOUS-failure
/// counter (`failure_count`), which is distinct from the cumulative trigger
/// counter (`run_count`):
/// - a MANUAL trigger never participates — its outcome leaves `failure_count`
///   untouched (VAL-ROBUST-024);
/// - a SCHEDULED success resets the counter to 0 (the failure chain is broken,
///   even though the row keeps its `attempt > 1` trail, VAL-ROBUST-016);
/// - a SCHEDULED failure/timeout increments it (VAL-ROBUST-017/018).
fn failure_count_update(trigger: Trigger, status: ExecutionStatus) -> FailureCountUpdate {
    match trigger {
        Trigger::Manual => FailureCountUpdate::Unchanged,
        Trigger::Schedule => {
            if matches!(status, ExecutionStatus::Success) {
                FailureCountUpdate::Reset
            } else {
                FailureCountUpdate::Increment
            }
        }
    }
}

/// Title of the continuous-failure desktop notification. Generic (no job
/// specifics) — the job is named in the body.
const FAILURE_NOTIFY_TITLE: &str = "定时任务连续失败";

/// Body of the continuous-failure desktop notification, naming the offending
/// job and its consecutive-failure count. Carries no command output or secret
/// material — just the job name and the count.
fn failure_notify_body(job_name: &str, failure_count: i32) -> String {
    format!("任务「{}」已连续失败 {} 次", job_name, failure_count)
}

/// The whole-seconds view of a `Duration`, for naming the threshold in a
/// timeout error message. The agent path holds only the `Duration` at the point
/// it builds its outcome, so this recovers the second count without re-threading
/// the raw config value.
fn timeout_secs_of(dur: Duration) -> i64 {
    dur.as_secs() as i64
}

/// Stable error message persisted on a `timeout` execution row, naming the
/// configured threshold. No secret material; safe for the UI.
fn timeout_error_message(timeout_secs: i64) -> String {
    format!(
        "execution exceeded the configured timeout of {}s and was interrupted",
        timeout_secs
    )
}

/// Stable failure message when the executor was built without the prompt
/// collaborators (the artifact-only unit harness). Never carries provider
/// detail.
const PROMPT_NOT_CONFIGURED: &str =
    "Prompt execution is not available (chat services are not configured)";

/// A pre-flight provider check failure, before any prompt is sent. Each variant
/// is something we can name precisely from the provider record alone, with no
/// network round-trip and no secret material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderFailure {
    /// `get_provider` returned `Err` — the provider id no longer resolves
    /// (deleted, or otherwise not found).
    Deleted,
    /// The provider exists but is toggled off (`enabled = false`).
    Disabled,
    /// The provider exists and is enabled but has no API key configured.
    MissingKey,
}

/// Classify the provider pre-flight: `None` means the provider is usable for a
/// send (exists, enabled, non-empty key); `Some(_)` is a named failure.
///
/// Pure over the `get_provider` result so it is unit-testable without a DB:
/// `Err` → [`ProviderFailure::Deleted`]; `!enabled` →
/// [`ProviderFailure::Disabled`]; blank `api_key` → [`ProviderFailure::MissingKey`].
/// The key is only checked for emptiness — its value is never read or logged.
fn classify_provider(result: Result<&Provider, &AppError>) -> Option<ProviderFailure> {
    match result {
        Err(_) => Some(ProviderFailure::Deleted),
        Ok(p) if !p.enabled => Some(ProviderFailure::Disabled),
        Ok(p) if p.api_key.trim().is_empty() => Some(ProviderFailure::MissingKey),
        Ok(_) => None,
    }
}

/// Map a [`ProviderFailure`] to a stable, user-facing message. Contains no
/// secret material and no raw provider/transport detail.
fn provider_failure_message(failure: ProviderFailure) -> String {
    match failure {
        ProviderFailure::Deleted => {
            "the configured provider no longer exists (it may have been deleted)".to_string()
        }
        ProviderFailure::Disabled => "the configured provider is disabled".to_string(),
        ProviderFailure::MissingKey => "the configured provider has no API key set".to_string(),
    }
}

/// Sanitize an `AppError` from chat creation / `send_user_message` into a stable
/// message safe to persist in `job_executions.error` and surface in the UI.
///
/// SECURITY (mirrors `sanitize_agent_error`): the raw `AppError.message` can
/// carry an upstream URL, an `Authorization: Bearer …` header, or a raw provider
/// payload (chat_engine's `client_err_to_app_err` forwards `Display` verbatim),
/// so it is NEVER echoed. We key off the stable `AppError.code` plus a narrow
/// content sniff to tell a *model-resolution* failure (the model id is not
/// registered under the provider) apart from a generic config failure
/// (VAL-TARGET-035). The raw message is the caller's responsibility to `tracing`
/// — it never flows through this function's output.
fn sanitize_send_error(err: &AppError) -> String {
    match err.code.as_str() {
        "AUTH_ERROR" => "the provider rejected the request (authentication failed)".to_string(),
        "NETWORK_ERROR" => "the request to the provider failed (network error)".to_string(),
        "RATE_LIMIT" => "the provider rate-limited the request".to_string(),
        // A post-provider-validation `VALIDATION_ERROR` is, in practice, a
        // model-resolution failure: the model id is not registered under the
        // provider (chat_engine's `resolve_model`). Sniff the known marker so
        // the model class is distinct from the provider class (VAL-TARGET-035).
        "VALIDATION_ERROR" if is_model_resolution_error(&err.message) => {
            "the selected model is not available for this provider".to_string()
        }
        "VALIDATION_ERROR" => {
            "the prompt could not be sent (invalid provider or model configuration)".to_string()
        }
        // INTERNAL_ERROR (incl. empty/aborted stream) and anything else.
        _ => "the prompt run failed to complete".to_string(),
    }
}

/// Whether a `VALIDATION_ERROR` message describes a model-resolution failure
/// (model id not registered under the provider). Matches the markers emitted by
/// `chat_engine::resolve_model_template` and hand-ai's `ProviderNotFound`. Only
/// the *shape* is inspected; the message itself is never propagated.
fn is_model_resolution_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    // `resolve_model_template`: "model '<id>' not registered under provider …".
    // hand-ai `ProviderNotFound` via `client_err_to_app_err`: "… not found …".
    lower.contains("not registered")
        || (lower.contains("model") && lower.contains("not found"))
        || lower.contains("no provider is configured for model")
}

/// Name for the fresh chat created per prompt run: the job name plus a unix-ms
/// suffix so concurrent runs of the same job stay visually distinct.
fn prompt_chat_name(job_name: &str) -> String {
    format!("{} · {}", job_name, current_timestamp())
}

/// Stable failure message when the executor was built without the agent
/// collaborators (the artifact-only unit harness). Never carries provider or
/// template detail.
const AGENT_NOT_CONFIGURED: &str =
    "Agent execution is not available (agent services are not configured)";

/// Stable message persisted when an agent run finishes with an in-band error
/// (the terminal assistant turn has `stopReason == "error"`). The raw upstream
/// `errorMessage` is NOT echoed — it can carry provider detail; the partial
/// transcript (which the run already persisted) is reachable via `result_ref`.
const AGENT_IN_BAND_ERROR: &str = "the agent run ended with an error";

/// A pre-flight agent failure, raised before any run is started. Each variant is
/// something we can name precisely from the template / catalog alone, with no
/// network round-trip and no secret material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentFailure {
    /// The agent template id no longer resolves (deleted, or not found).
    TemplateMissing,
    /// The template exists but has no model selected.
    NoModel,
    /// The template's model is no longer served by any provider (the model — or
    /// every provider that served it — was removed).
    ModelRemoved,
    /// A provider/model configuration problem that is neither a clean
    /// "no model" nor a clean "model removed" (e.g. the model survives only
    /// under a disabled provider, or the provider catalog could not be read).
    ConfigError,
}

/// Map an [`AgentFailure`] to a stable, user-facing message. Carries no secret
/// material and no raw provider/template detail. The "template missing" message
/// is deliberately distinct from the model/config-class ones so the two are
/// distinguishable (VAL-TARGET-020 vs VAL-TARGET-021).
fn agent_failure_message(failure: AgentFailure) -> String {
    match failure {
        AgentFailure::TemplateMissing => {
            "the configured agent no longer exists (it may have been deleted)".to_string()
        }
        AgentFailure::NoModel => "the configured agent has no model selected".to_string(),
        AgentFailure::ModelRemoved => {
            "the agent's model is no longer available for any provider".to_string()
        }
        AgentFailure::ConfigError => {
            "the agent could not be run (invalid provider or model configuration)".to_string()
        }
    }
}

/// Sanitize an `AppError` from agent session creation / `start_run` into a
/// stable message safe to persist and surface. Mirrors [`sanitize_send_error`]:
/// the raw `AppError.message` can carry an upstream URL, an `Authorization`
/// header, or a raw provider payload, so it is NEVER echoed. We key off the
/// stable `AppError.code` plus a narrow content sniff to keep a model-resolution
/// failure distinct from a generic config failure. Raw detail is the caller's
/// responsibility to `tracing` — it never flows through this output.
fn sanitize_agent_dispatch_error(err: &AppError) -> String {
    match err.code.as_str() {
        "AUTH_ERROR" => "the provider rejected the request (authentication failed)".to_string(),
        "NETWORK_ERROR" => "the request to the provider failed (network error)".to_string(),
        "RATE_LIMIT" => "the provider rate-limited the request".to_string(),
        "VALIDATION_ERROR" if is_model_resolution_error(&err.message) => {
            "the selected model is not available for this provider".to_string()
        }
        "VALIDATION_ERROR" => {
            "the agent could not be run (invalid provider or model configuration)".to_string()
        }
        _ => "the agent run failed to complete".to_string(),
    }
}

/// Name for the fresh session minted per agent run: the template name plus a
/// unix-ms suffix so concurrent runs of the same agent stay visually distinct.
fn agent_session_name(agent_name: &str) -> String {
    format!("{} · {}", agent_name, current_timestamp())
}

/// The terminal classification of an agent run, derived from its persisted
/// transcript AFTER the run closed (no run-level error envelope was raised).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentRunResult {
    /// The run completed without an in-band error terminal turn.
    Success,
    /// The final assistant turn carries `stopReason == "error"` — the run ended
    /// with an in-band error but the transcript is persisted (VAL-TARGET-024).
    InBandError,
}

/// Classify a run from its persisted transcript. The final assistant message's
/// `stopReason` is the source of truth: `"error"` is an in-band error; anything
/// else (including no assistant turn at all, or a transcript read failure) is
/// treated as a non-error completion — a run-level failure would instead have
/// surfaced through the sink's error envelope and never reached here.
///
/// Pure over the transcript so it is unit-testable without a runtime. Messages
/// are persisted hand-agent `Message`s (tagged by `role`); the assistant
/// payload carries a camelCase `stopReason` field.
fn classify_agent_transcript(
    transcript: Result<&Vec<crate::storage::types::AgentSessionMessage>, &AppError>,
) -> AgentRunResult {
    let messages = match transcript {
        Ok(messages) => messages,
        // A transcript read failure cannot prove an in-band error; default to a
        // non-error completion (the run already closed without an envelope).
        Err(_) => return AgentRunResult::Success,
    };

    let last_assistant = messages.iter().rev().find(|m| m.role == "assistant");

    match last_assistant {
        Some(message) => {
            let is_error = message
                .payload
                .get("stopReason")
                .and_then(|v| v.as_str())
                .map(|s| s == "error")
                .unwrap_or(false);
            if is_error {
                AgentRunResult::InBandError
            } else {
                AgentRunResult::Success
            }
        }
        None => AgentRunResult::Success,
    }
}

/// Type alias for a sink callback (`Arc<dyn Fn(Value) + Send + Sync>`), matching
/// [`CodingRunSink`]'s constructor parameters.
type SinkCallback = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// The three sink callbacks plus the close receiver. Built by
/// [`build_oneshot_signal`] and assembled into a [`CodingRunSink`] by
/// [`oneshot_run_sink`]; returned separately so the signal wiring can be driven
/// directly in tests without reaching into `CodingRunSink`'s private fields.
struct OneshotSignal {
    on_event: SinkCallback,
    on_error: SinkCallback,
    on_closed: SinkCallback,
    rx: oneshot::Receiver<Option<String>>,
}

/// Build the oneshot-signal callbacks: `on_closed` fires the `oneshot` so a
/// background dispatch can `.await` the turn's completion.
///
/// The oneshot carries `Option<String>`: a run-level error envelope (captured
/// from `on_error`, already sanitized by the runtime) is forwarded as
/// `Some(message)` when the run closes; a clean close sends `None`. `on_event`
/// is dropped — the executor classifies the outcome from the persisted
/// transcript, not the live event stream. The runtime fires `on_closed` exactly
/// once, AFTER the transcript is fully persisted, so awaiting the oneshot blocks
/// precisely until the turn has ended and is durable. `on_error` fires (at most
/// once) BEFORE `on_closed` on the same runtime task, so the captured value is
/// set before it is read on close.
fn build_oneshot_signal() -> OneshotSignal {
    let (tx, rx) = oneshot::channel::<Option<String>>();
    // The last run-level error envelope message, captured for the close signal.
    // A `std::sync::Mutex` is correct here: the callbacks are synchronous and
    // never hold the lock across an await.
    let captured_error: Arc<std::sync::Mutex<Option<String>>> =
        Arc::new(std::sync::Mutex::new(None));

    let on_event: SinkCallback = Arc::new(|_event: serde_json::Value| {
        // Dropped: outcome is read from the persisted transcript, not events.
    });

    let on_error_slot = captured_error.clone();
    let on_error: SinkCallback = Arc::new(move |envelope: serde_json::Value| {
        // The envelope is `{ sessionId, error: { code, message, hint } }` with
        // `error.message` already sanitized by the runtime. Capture only the
        // message; raw transport detail never reaches it.
        let message = envelope
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .map(str::to_string);
        if let Ok(mut slot) = on_error_slot.lock() {
            *slot = message;
        }
    });

    // The oneshot sender is single-use; wrap it so the `Fn` close callback can
    // take it on first (and only) invocation.
    let tx_slot: Arc<std::sync::Mutex<Option<oneshot::Sender<Option<String>>>>> =
        Arc::new(std::sync::Mutex::new(Some(tx)));
    let on_closed_error = captured_error;
    let on_closed: SinkCallback = Arc::new(move |_payload: serde_json::Value| {
        let captured = on_closed_error.lock().ok().and_then(|slot| slot.clone());
        if let Some(sender) = tx_slot.lock().ok().and_then(|mut slot| slot.take()) {
            // The receiver may have been dropped if the dispatch was cancelled;
            // a send failure is then benign.
            let _ = sender.send(captured);
        }
    });

    OneshotSignal {
        on_event,
        on_error,
        on_closed,
        rx,
    }
}

/// Build a [`CodingRunSink`] whose `on_closed` fires a `oneshot` so a background
/// dispatch can `.await` the turn's completion, paired with the receiver. See
/// [`build_oneshot_signal`] for the signal semantics. The sink shape mirrors the
/// foreground `agent_run_stream` command's, reduced to a single completion
/// signal: `on_event` is dropped (the outcome is read from the persisted
/// transcript, not the live stream) and `on_error` captures the sanitized
/// run-level envelope so a clean/error close is distinguishable on the receiver.
fn oneshot_run_sink() -> (CodingRunSink, oneshot::Receiver<Option<String>>) {
    let signal = build_oneshot_signal();
    let sink = CodingRunSink::new(signal.on_event, signal.on_closed).with_error(signal.on_error);
    (sink, signal.rx)
}

/// RAII release of an in-flight job slot.
///
/// Holds the shared in-flight set and the job id; on `Drop` (whether the run
/// returns normally or unwinds from a panic) it removes the id, freeing the slot
/// for a later dispatch. This is what makes re-entrancy protection survive a
/// panicking / crashing target, and what releases a manual run's claim once
/// `run_now` returns (success OR error).
///
/// Owned by the executor (the single in-flight set lives here); the scheduler
/// and `run_now` obtain a guard via [`JobExecutor::try_claim`].
pub struct InFlightGuard {
    set: Arc<Mutex<HashSet<String>>>,
    job_id: String,
}

impl InFlightGuard {
    fn new(set: Arc<Mutex<HashSet<String>>>, job_id: String) -> Self {
        Self { set, job_id }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        // `Drop` is sync but the set is an async `Mutex`; take it without
        // blocking a runtime worker. The uncontended path (the common case —
        // the set is held only for the O(1) insert/remove around each dispatch)
        // is synchronous via `try_lock`; a contended drop hands the removal to a
        // detached task so `Drop` never blocks.
        let set = self.set.clone();
        let job_id = self.job_id.clone();
        if let Ok(mut guard) = set.try_lock() {
            guard.remove(&job_id);
            return;
        }
        tauri::async_runtime::spawn(async move {
            set.lock().await.remove(&job_id);
        });
    }
}

fn current_timestamp() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::types::{
        ArtifactType, CreateArtifactRequest, ExecutionConfig, InstallArtifactRequest,
        SessionStrategy,
    };
    use crate::storage::{ArtifactRepository, Database};
    use sqlx::Row;
    use std::collections::HashMap;
    use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
    use tempfile::tempdir;

    /// A row read straight from `job_executions` for assertions, decoded with
    /// `Option` for every nullable column so a NULL never panics.
    struct ExecutionRow {
        id: String,
        status: String,
        stdout: Option<String>,
        stderr: Option<String>,
        exit_code: Option<i32>,
        error: Option<String>,
        attempt: i32,
        ended_at: Option<i64>,
    }

    struct TestEnv {
        executor: JobExecutor<MockRuntime>,
        db: Arc<Database>,
        artifact_service: Arc<ArtifactService<MockRuntime>>,
        // Keep the temp dirs alive for the duration of the test (the app data
        // dir backs the artifact sandbox).
        _temp_dir: tempfile::TempDir,
    }

    /// Build an executor wired to a fresh temp DB and a shared `MockRuntime`
    /// `ArtifactService`. The DB runs all migrations (incl. 049/050). The
    /// executor and the test share the SAME `ArtifactService` so artifacts the
    /// test installs are visible to the executor.
    async fn setup() -> TestEnv {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(Database::new(&db_path).await.unwrap());

        let artifact_repo = Arc::new(ArtifactRepository::new(db.clone()));
        let context = mock_context::<MockRuntime, _>(noop_assets());
        let app = mock_builder()
            .build(context)
            .expect("failed to build app for tests");
        let artifact_service = Arc::new(ArtifactService::new(artifact_repo, app.handle().clone()));

        let executor = JobExecutor::from_db(db.clone(), artifact_service.clone());

        TestEnv {
            executor,
            db,
            artifact_service,
            _temp_dir: temp_dir,
        }
    }

    /// Wire REAL prompt collaborators (SessionService / MessageService /
    /// ProviderService) onto the env's executor, all sharing the env's temp DB.
    /// `StorageService` is rooted at the env temp dir. Returns a NEW `TestEnv`
    /// whose executor can dispatch `prompt` targets end-to-end (offline: a model
    /// that does not resolve fails the send AFTER the user message is saved).
    fn with_prompt_services(env: TestEnv) -> TestEnv {
        use crate::services::{McpService, ProviderService, SessionService, StorageService};

        let provider_service = Arc::new(ProviderService::new(env.db.clone()));
        let session_service = Arc::new(SessionService::new(
            env.db.clone(),
            provider_service.clone(),
        ));
        let mcp_service = Arc::new(McpService::new(env.db.clone()));
        let storage_service = Arc::new(
            StorageService::new(env._temp_dir.path().to_path_buf()).expect("storage service"),
        );
        let message_service = Arc::new(crate::services::MessageService::new(
            env.db.clone(),
            provider_service.clone(),
            session_service.clone(),
            mcp_service,
            storage_service,
        ));

        let executor = env.executor.clone().with_prompt_services(
            session_service,
            message_service,
            provider_service,
        );

        TestEnv { executor, ..env }
    }

    /// Seed an enabled provider with a non-empty key directly into the DB so the
    /// executor's pre-flight passes and the send reaches the model-resolution
    /// step. `provider_type` controls catalog resolution.
    async fn seed_provider(env: &TestEnv, id: &str, provider_type: &str, enabled: bool, key: &str) {
        let repo = crate::storage::ProviderRepository::new(env.db.clone());
        let provider = Provider {
            id: id.to_string(),
            name: format!("prov-{}", id),
            provider_type: provider_type.to_string(),
            base_url: "https://api.example.invalid".to_string(),
            api_key: key.to_string(),
            enabled,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };
        repo.create_provider(&provider)
            .await
            .expect("seed provider");
    }

    /// Read the (role, content) of every `messages` row for a chat, oldest-first.
    async fn read_chat_messages(env: &TestEnv, chat_id: &str) -> Vec<(String, String)> {
        let rows = sqlx::query(
            "SELECT role, content FROM messages WHERE session_id = $1 ORDER BY created_at ASC, id ASC",
        )
        .bind(chat_id)
        .fetch_all(env.db.pool())
        .await
        .unwrap();
        rows.into_iter()
            .map(|r| {
                (
                    r.try_get::<String, _>("role").unwrap(),
                    r.try_get::<String, _>("content").unwrap(),
                )
            })
            .collect()
    }

    /// Create + install a shell artifact whose `main.sh` is `script`. Returns the
    /// installed artifact id. Installation copies the script into the sandbox
    /// (`app_data_dir/artifacts/<id>/main.sh`) where `execute_artifact` runs it.
    async fn install_shell_artifact(env: &TestEnv, script: &str) -> String {
        let src_dir = tempdir().unwrap();
        let entry = src_dir.path().join("main.sh");
        tokio::fs::write(&entry, script).await.unwrap();

        let artifact = env
            .artifact_service
            .create_artifact(CreateArtifactRequest {
                name: format!("shell-{}", uuid::Uuid::new_v4()),
                description: None,
                artifact_type: ArtifactType::Shell,
                entry_file: "main.sh".to_string(),
                source_path: Some(src_dir.path().to_string_lossy().to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: Some(ExecutionConfig {
                    args: vec![],
                    env: HashMap::new(),
                    permissions: vec![],
                    timeout: 5000,
                }),
                tags: None,
                icon: None,
            })
            .await
            .expect("create artifact");

        env.artifact_service
            .install_artifact(InstallArtifactRequest {
                artifact_id: artifact.id.clone(),
                model_id: None,
                provider_id: None,
            })
            .await
            .expect("install artifact");

        // Keep the source dir alive until after install copied the files.
        drop(src_dir);
        artifact.id
    }

    /// Build an enabled `Job` with the given target and a future `next_run_at`.
    async fn make_job(id: &str, target: JobTarget) -> Job {
        let now = current_timestamp();
        Job {
            id: id.to_string(),
            name: format!("job-{}", id),
            description: None,
            target,
            cron_expr: "0 9 * * *".to_string(),
            timezone: "local".to_string(),
            enabled: true,
            last_run_at: None,
            next_run_at: Some(now + 10_000),
            last_status: None,
            run_count: 0,
            failure_count: 0,
            exec_timeout_secs: 0,
            max_retries: 0,
            retry_delay_secs: 60,
            created_at: now,
            updated_at: now,
        }
    }

    /// Seed a `jobs` row so foreign-key-backed execution rows can be inserted
    /// and the post-run statistics update has a row to touch.
    async fn seed_job(env: &TestEnv, job: &Job) {
        env.executor.jobs.create(job).await.expect("seed job");
    }

    /// Read all `job_executions` rows for a job, newest-first.
    async fn read_rows(env: &TestEnv, job_id: &str) -> Vec<ExecutionRow> {
        let rows = sqlx::query(
            "SELECT id, status, stdout, stderr, exit_code, error, attempt, ended_at \
             FROM job_executions WHERE job_id = $1 ORDER BY started_at DESC, id DESC",
        )
        .bind(job_id)
        .fetch_all(env.db.pool())
        .await
        .unwrap();

        rows.into_iter()
            .map(|r| ExecutionRow {
                id: r.try_get("id").unwrap(),
                status: r.try_get("status").unwrap(),
                stdout: r.try_get("stdout").unwrap(),
                stderr: r.try_get("stderr").unwrap(),
                exit_code: r.try_get("exit_code").unwrap(),
                error: r.try_get("error").unwrap(),
                attempt: r.try_get("attempt").unwrap(),
                ended_at: r.try_get("ended_at").unwrap(),
            })
            .collect()
    }

    fn artifact_target(artifact_id: &str) -> JobTarget {
        JobTarget::Artifact {
            artifact_id: artifact_id.to_string(),
            args: vec![],
            env: HashMap::new(),
        }
    }

    // VAL-TARGET-001 / VAL-HISTORY-010: a successful artifact run records
    // stdout/stderr/exit_code and produces exactly one row.
    #[tokio::test]
    async fn artifact_success_records_output_in_single_row() {
        let env = setup().await;
        let artifact_id =
            install_shell_artifact(&env, "echo hello-out\necho oops-err 1>&2\nexit 0\n").await;
        let job = make_job("job_ok", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let exec = env
            .executor
            .execute(&job, Trigger::Schedule)
            .await
            .expect("execute");

        assert_eq!(exec.status, ExecutionStatus::Success);
        assert_eq!(exec.exit_code, Some(0));
        assert!(exec.stdout.as_deref().unwrap().contains("hello-out"));
        assert!(exec.stderr.as_deref().unwrap().contains("oops-err"));

        let rows = read_rows(&env, "job_ok").await;
        assert_eq!(rows.len(), 1, "one trigger => exactly one row");
        assert_eq!(rows[0].id, exec.id);
        assert_eq!(rows[0].status, "success");
        assert_eq!(rows[0].exit_code, Some(0));
        assert_eq!(rows[0].attempt, 1);
        assert!(rows[0].stdout.as_deref().unwrap().contains("hello-out"));
        assert!(rows[0].stderr.as_deref().unwrap().contains("oops-err"));
        assert!(rows[0].ended_at.is_some());
    }

    // VAL-TARGET-002: exit 0 with non-empty stderr is still success.
    #[tokio::test]
    async fn exit_zero_with_stderr_is_success() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo to-stderr 1>&2\nexit 0\n").await;
        let job = make_job("job_warn", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Success);
        assert_eq!(exec.exit_code, Some(0));
        assert!(exec.stderr.as_deref().unwrap().contains("to-stderr"));
    }

    // VAL-TARGET-003: a non-zero exit is failed, with all three of
    // stdout/stderr/exit_code visible.
    #[tokio::test]
    async fn non_zero_exit_is_failed_with_all_fields() {
        let env = setup().await;
        let artifact_id =
            install_shell_artifact(&env, "echo partial-out\necho boom 1>&2\nexit 3\n").await;
        let job = make_job("job_fail", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.exit_code, Some(3));

        let rows = read_rows(&env, "job_fail").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
        assert_eq!(rows[0].exit_code, Some(3));
        assert!(rows[0].stdout.as_deref().unwrap().contains("partial-out"));
        assert!(rows[0].stderr.as_deref().unwrap().contains("boom"));
    }

    // VAL-TARGET-004: a process that cannot start (interpreter not found on the
    // child's PATH) is failed, with exit_code NULL and a non-empty error. We
    // install a Python artifact and override the child's PATH to a directory
    // with no `python3`, so the exec of the interpreter itself fails (a genuine
    // spawn failure, distinct from a script exiting non-zero).
    #[tokio::test]
    async fn spawn_failure_is_failed_with_null_exit_and_error() {
        let env = setup().await;

        let src_dir = tempdir().unwrap();
        let entry = src_dir.path().join("main.py");
        tokio::fs::write(&entry, "print('never runs')\n")
            .await
            .unwrap();

        let artifact = env
            .artifact_service
            .create_artifact(CreateArtifactRequest {
                name: format!("py-{}", uuid::Uuid::new_v4()),
                description: None,
                artifact_type: ArtifactType::Python,
                entry_file: "main.py".to_string(),
                source_path: Some(src_dir.path().to_string_lossy().to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: Some(ExecutionConfig {
                    args: vec![],
                    env: HashMap::new(),
                    permissions: vec![],
                    timeout: 5000,
                }),
                tags: None,
                icon: None,
            })
            .await
            .unwrap();
        env.artifact_service
            .install_artifact(InstallArtifactRequest {
                artifact_id: artifact.id.clone(),
                model_id: None,
                provider_id: None,
            })
            .await
            .unwrap();

        // Override the child's PATH to a guaranteed-empty location so `python3`
        // cannot be resolved; the interpreter exec fails rather than the script
        // exiting non-zero.
        let mut bad_env = HashMap::new();
        bad_env.insert("PATH".to_string(), "/nonexistent-handbox-path".to_string());
        let target = JobTarget::Artifact {
            artifact_id: artifact.id.clone(),
            args: vec![],
            env: bad_env,
        };
        let job = make_job("job_spawn", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.exit_code, None, "spawn failure => no exit code");
        assert!(
            exec.error
                .as_deref()
                .map(|e| !e.is_empty())
                .unwrap_or(false),
            "spawn failure must carry a non-empty error, got {:?}",
            exec.error
        );

        let rows = read_rows(&env, "job_spawn").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
        assert_eq!(rows[0].exit_code, None);
        assert!(rows[0].error.is_some());
    }

    // VAL-TARGET-016: a never-installed artifact target is failed with an
    // "not installed" error.
    #[tokio::test]
    async fn never_installed_artifact_is_failed_with_error() {
        let env = setup().await;
        // Create but do NOT install.
        let artifact = env
            .artifact_service
            .create_artifact(CreateArtifactRequest {
                name: format!("uninstalled-{}", uuid::Uuid::new_v4()),
                description: None,
                artifact_type: ArtifactType::Shell,
                entry_file: "main.sh".to_string(),
                source_path: None,
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: Some(ExecutionConfig::default()),
                tags: None,
                icon: None,
            })
            .await
            .unwrap();

        let job = make_job("job_uninstalled", artifact_target(&artifact.id)).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.exit_code, None);
        let err = exec.error.as_deref().unwrap_or_default();
        assert!(
            err.to_lowercase().contains("install"),
            "error should mention installation, got: {}",
            err
        );

        let rows = read_rows(&env, "job_uninstalled").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
        assert!(rows[0].error.is_some());
    }

    // VAL-TARGET-015: a target referencing a completely unknown artifact id is
    // failed with a non-empty error.
    #[tokio::test]
    async fn missing_artifact_is_failed_with_error() {
        let env = setup().await;
        let job = make_job("job_missing", artifact_target("does-not-exist")).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert!(exec
            .error
            .as_deref()
            .map(|e| !e.is_empty())
            .unwrap_or(false));

        let rows = read_rows(&env, "job_missing").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
        assert!(rows[0].error.is_some());
    }

    // VAL-TARGET-028: args containing spaces/quotes are passed as a single argv
    // each (no shell), so they reach the program verbatim with no injection.
    #[tokio::test]
    async fn args_with_spaces_and_quotes_passed_as_single_argv() {
        let env = setup().await;
        // Echo each positional arg on its own line so we can count and inspect.
        let artifact_id =
            install_shell_artifact(&env, "for a in \"$@\"; do echo \"ARG:$a\"; done\nexit 0\n")
                .await;

        let injected = "; touch /tmp/handbox_pwned";
        let target = JobTarget::Artifact {
            artifact_id: artifact_id.clone(),
            args: vec![
                "hello world".to_string(),
                "with \"quotes\"".to_string(),
                injected.to_string(),
            ],
            env: HashMap::new(),
        };
        let job = make_job("job_args", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Success);
        let stdout = exec.stdout.as_deref().unwrap();
        // Exactly three args arrived, each intact (spaces/quotes preserved).
        assert!(stdout.contains("ARG:hello world"));
        assert!(stdout.contains("ARG:with \"quotes\""));
        assert!(stdout.contains(&format!("ARG:{}", injected)));
        // The injection string was treated as data, not executed: it appears as
        // a single argument, and the shell never created the marker file.
        assert!(!std::path::Path::new("/tmp/handbox_pwned").exists());
        let arg_lines = stdout.lines().filter(|l| l.starts_with("ARG:")).count();
        assert_eq!(arg_lines, 3, "exactly three argv entries");
    }

    // VAL-TARGET-029: env with an empty value and (effectively) overlapping keys
    // does not crash; execution still completes.
    #[tokio::test]
    async fn env_with_empty_value_does_not_crash() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(
            &env,
            "echo \"EMPTY=[$EMPTY_VAR]\"\necho \"SET=[$SET_VAR]\"\nexit 0\n",
        )
        .await;

        // HashMap cannot hold duplicate keys, but an empty value is the boundary
        // we can express through the typed target; assert it round-trips and the
        // run still succeeds.
        let mut env_map = HashMap::new();
        env_map.insert("EMPTY_VAR".to_string(), String::new());
        env_map.insert("SET_VAR".to_string(), "present".to_string());
        let target = JobTarget::Artifact {
            artifact_id: artifact_id.clone(),
            args: vec![],
            env: env_map,
        };
        let job = make_job("job_env", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(
            exec.status,
            ExecutionStatus::Success,
            "error: {:?}",
            exec.error
        );
        let stdout = exec.stdout.as_deref().unwrap();
        assert!(stdout.contains("EMPTY=[]"));
        assert!(stdout.contains("SET=[present]"));
    }

    // VAL-HISTORY-009 / VAL-HISTORY-010: the running row is inserted up front
    // (ended_at NULL) and then the SAME row id flips to terminal in place — one
    // row total. Verified by inspecting the row mid-flight via a slow script.
    #[tokio::test]
    async fn running_row_starts_then_updates_same_id() {
        let env = setup().await;
        // A script slow enough to observe the running row before it finalizes.
        let artifact_id = install_shell_artifact(&env, "sleep 0.3\necho done\nexit 0\n").await;
        let job = make_job("job_running", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        // Run the executor concurrently; poll the DB for the running row.
        let job_clone = job.clone();
        let executor = env.executor.clone();
        let handle =
            tokio::spawn(async move { executor.execute(&job_clone, Trigger::Schedule).await });

        // Poll until a running row appears (ended_at NULL).
        let mut running_id = None;
        for _ in 0..50 {
            let rows = read_rows(&env, "job_running").await;
            if let Some(row) = rows.iter().find(|r| r.status == "running") {
                assert_eq!(row.ended_at, None, "running row has NULL ended_at");
                running_id = Some(row.id.clone());
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let running_id = running_id.expect("a running row should appear mid-flight");

        let exec = handle.await.unwrap().expect("execute");

        // The finalized execution reuses the SAME id, and exactly one row exists.
        assert_eq!(exec.id, running_id, "finalize updates the same row id");
        let rows = read_rows(&env, "job_running").await;
        assert_eq!(rows.len(), 1, "start-then-update, never a second row");
        assert_eq!(rows[0].id, running_id);
        assert_eq!(rows[0].status, "success");
        assert!(rows[0].ended_at.is_some());
    }

    // Without the agent collaborators wired (the artifact-only unit harness),
    // an agent target fails cleanly with a stable "not configured" message — it
    // never panics and never leaks any template / provider detail.
    #[tokio::test]
    async fn agent_target_without_services_fails_cleanly() {
        let env = setup().await;
        // `setup` builds the executor via `from_db` — no agent services.
        let target = JobTarget::Agent {
            agent_id: "agent_1".to_string(),
            initial_message: "go".to_string(),
            project_id: None,
        };
        let job = make_job("job_agent", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.error.as_deref(), Some(AGENT_NOT_CONFIGURED));
        // No raw agent id leaks into the persisted error.
        let err = exec.error.unwrap_or_default();
        assert!(!err.contains("agent_1"));
        assert!(exec.result_ref.is_none(), "no session created when unwired");

        let rows = read_rows(&env, "job_agent").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
    }

    // Without the prompt collaborators wired (the artifact-only unit harness),
    // a prompt target fails cleanly with a stable "not configured" message —
    // it never panics and never leaks any provider detail.
    #[tokio::test]
    async fn prompt_target_without_services_fails_cleanly() {
        let env = setup().await;
        // `setup` builds the executor via `from_db` — no prompt services.
        let target = JobTarget::Prompt {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            prompt: "summarize".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        let job = make_job("job_prompt", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.error.as_deref(), Some(PROMPT_NOT_CONFIGURED));
        // No raw provider id or model id leaks into the persisted error.
        let err = exec.error.unwrap_or_default();
        assert!(!err.contains("openai"));
        assert!(!err.contains("gpt-4"));

        let rows = read_rows(&env, "job_prompt").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
    }

    // VAL-TARGET-019 (end-to-end): a prompt whose provider does not exist in the
    // DB is failed with the deleted-provider message — and NO chat is created
    // (the pre-flight short-circuits before chat creation), so no result_ref.
    #[tokio::test]
    async fn prompt_missing_provider_fails_before_chat() {
        let env = with_prompt_services(setup().await);
        // No provider seeded → get_provider returns Err.
        let target = JobTarget::Prompt {
            provider_id: "ghost".to_string(),
            model_id: "gpt-4o".to_string(),
            prompt: "hi".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        let job = make_job("job_p_missing", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(provider_failure_message(ProviderFailure::Deleted).as_str())
        );
        assert!(
            exec.result_ref.is_none(),
            "no chat created on pre-flight fail"
        );
        // No chat row leaked into the DB (the `chats` table is named `sessions`).
        let chats: i64 = sqlx::query("SELECT COUNT(*) FROM sessions")
            .fetch_one(env.db.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(chats, 0);
    }

    // VAL-TARGET-018 (end-to-end): a disabled provider is failed before the send,
    // with the disabled message and no chat.
    #[tokio::test]
    async fn prompt_disabled_provider_fails_before_chat() {
        let env = with_prompt_services(setup().await);
        seed_provider(&env, "prov_off", "openai", false, "sk-live-abcd").await;
        let target = JobTarget::Prompt {
            provider_id: "prov_off".to_string(),
            model_id: "gpt-4o".to_string(),
            prompt: "hi".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        let job = make_job("job_p_off", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(provider_failure_message(ProviderFailure::Disabled).as_str())
        );
        assert!(exec.result_ref.is_none());
    }

    // VAL-TARGET-017 (end-to-end): an enabled provider with a blank key is failed
    // before the send with the missing-key message and no chat.
    #[tokio::test]
    async fn prompt_keyless_provider_fails_before_chat() {
        let env = with_prompt_services(setup().await);
        seed_provider(&env, "prov_nokey", "openai", true, "").await;
        let target = JobTarget::Prompt {
            provider_id: "prov_nokey".to_string(),
            model_id: "gpt-4o".to_string(),
            prompt: "hi".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        let job = make_job("job_p_nokey", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(provider_failure_message(ProviderFailure::MissingKey).as_str())
        );
        assert!(exec.result_ref.is_none());
    }

    // VAL-TARGET-023 + VAL-TARGET-030 + VAL-TARGET-035 (end-to-end, offline): a
    // provider that passes the pre-flight but whose model does NOT resolve under
    // the catalog provider type fails the send AFTER the user message is saved.
    // The outcome is `failed` with the model-class sanitized error, and
    // `result_ref` points at the PARTIAL chat — which holds exactly the user
    // message (unicode preserved) and NO assistant message.
    #[tokio::test]
    async fn prompt_model_unresolvable_leaves_partial_chat_with_user_message() {
        let env = with_prompt_services(setup().await);
        // Enabled, keyed, but `bogus-model` is not in the openai catalog → the
        // model-resolution step fails offline, after save_user_message.
        seed_provider(&env, "prov_ok", "openai", true, "sk-live-abcd").await;
        let unicode_prompt = "你好，请总结今日要点 🌟 — résumé";
        let target = JobTarget::Prompt {
            provider_id: "prov_ok".to_string(),
            model_id: "bogus-model-not-in-catalog".to_string(),
            prompt: unicode_prompt.to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        let job = make_job("job_p_model", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        // Model-class sanitized error (distinct from provider pre-flight).
        let err = exec.error.clone().unwrap_or_default();
        assert!(
            err.to_lowercase().contains("model"),
            "model-class error: {}",
            err
        );
        // No raw model id / provider id / key leaks into the persisted error.
        assert!(!err.contains("bogus-model-not-in-catalog"));
        assert!(!err.contains("prov_ok"));
        assert!(!err.contains("sk-live"));

        // result_ref points at the partial chat (VAL-TARGET-023).
        let chat_id = exec
            .result_ref
            .expect("result_ref points at the partial chat");
        let messages = read_chat_messages(&env, &chat_id).await;
        // Exactly the user message, no assistant message.
        assert_eq!(messages.len(), 1, "partial chat: user only, no assistant");
        assert_eq!(messages[0].0, "user");
        // VAL-TARGET-030: unicode preserved byte-for-byte.
        assert_eq!(messages[0].1, unicode_prompt);
    }

    // ---- prompt dispatch pure helpers (VAL-TARGET-017/018/019/026/027/035) ----

    fn sample_provider(enabled: bool, api_key: &str) -> Provider {
        Provider {
            id: "prov_1".to_string(),
            name: "Test".to_string(),
            provider_type: "openai".to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: api_key.to_string(),
            enabled,
            created_at: 0,
            updated_at: 0,
        }
    }

    // VAL-TARGET-019: a deleted provider (get_provider => Err) is classified as
    // a deleted-provider failure.
    #[test]
    fn classify_provider_err_is_deleted() {
        let err = AppError::validation_error("Provider not found");
        assert_eq!(
            classify_provider(Err::<&Provider, _>(&err)),
            Some(ProviderFailure::Deleted)
        );
    }

    // VAL-TARGET-018: an existing-but-disabled provider is classified as
    // disabled (checked before the key, so a disabled keyless provider is still
    // reported as disabled — the first actionable problem).
    #[test]
    fn classify_provider_disabled() {
        let p = sample_provider(false, "sk-live-123");
        assert_eq!(classify_provider(Ok(&p)), Some(ProviderFailure::Disabled));
        // Disabled wins even with no key.
        let p_no_key = sample_provider(false, "");
        assert_eq!(
            classify_provider(Ok(&p_no_key)),
            Some(ProviderFailure::Disabled)
        );
    }

    // VAL-TARGET-017: an enabled provider with a blank/whitespace key is a
    // missing-key failure.
    #[test]
    fn classify_provider_missing_key() {
        let p = sample_provider(true, "");
        assert_eq!(classify_provider(Ok(&p)), Some(ProviderFailure::MissingKey));
        let p_ws = sample_provider(true, "   ");
        assert_eq!(
            classify_provider(Ok(&p_ws)),
            Some(ProviderFailure::MissingKey)
        );
    }

    // A usable provider (enabled + non-empty key) passes the pre-flight.
    #[test]
    fn classify_provider_ok() {
        let p = sample_provider(true, "sk-live-123");
        assert_eq!(classify_provider(Ok(&p)), None);
    }

    // VAL-TARGET-026: classifying a provider never reads the key value — the
    // three pre-flight failure messages carry no key material whatsoever.
    #[test]
    fn provider_failure_messages_carry_no_secret() {
        for failure in [
            ProviderFailure::Deleted,
            ProviderFailure::Disabled,
            ProviderFailure::MissingKey,
        ] {
            let msg = provider_failure_message(failure);
            assert!(!msg.is_empty());
            assert!(!msg.contains("sk-"));
        }
    }

    // VAL-TARGET-027: each error code maps to a stable message that contains no
    // raw URL, Bearer token, or key fragment from the underlying AppError.
    #[test]
    fn sanitize_send_error_drops_raw_detail() {
        // An AUTH_ERROR whose raw message embeds a URL + Bearer header (exactly
        // the kind of leak chat_engine's Display passthrough can produce).
        let leaky = AppError::auth_error(
            "POST https://api.openai.com/v1/chat failed: Authorization: Bearer sk-live-SECRET",
        );
        let sanitized = sanitize_send_error(&leaky);
        assert!(!sanitized.contains("sk-live-SECRET"));
        assert!(!sanitized.contains("Bearer"));
        assert!(!sanitized.contains("https://"));
        assert!(!sanitized.contains("api.openai.com"));
        // It is still a meaningful auth message.
        assert!(sanitized.to_lowercase().contains("authentication"));
    }

    #[test]
    fn sanitize_send_error_maps_each_code() {
        assert!(sanitize_send_error(&AppError::auth_error("x"))
            .to_lowercase()
            .contains("authentication"));
        assert!(sanitize_send_error(&AppError::network_error("x"))
            .to_lowercase()
            .contains("network"));
        assert!(sanitize_send_error(&AppError::rate_limit_error())
            .to_lowercase()
            .contains("rate"));
        assert!(!sanitize_send_error(&AppError::internal_error("x")).is_empty());
    }

    // VAL-TARGET-035: a model-resolution VALIDATION_ERROR is mapped to a model
    // class message, distinct from a generic config failure — and distinct from
    // the provider pre-flight failures.
    #[test]
    fn sanitize_send_error_distinguishes_model_resolution() {
        let model_err = AppError::validation_error(
            "chat_engine: model 'gpt-4' not registered under provider 'openai'",
        );
        let model_msg = sanitize_send_error(&model_err);
        assert!(model_msg.to_lowercase().contains("model"));

        let generic = AppError::validation_error("Chat ID is required");
        let generic_msg = sanitize_send_error(&generic);
        // The two validation sub-cases yield different messages.
        assert_ne!(model_msg, generic_msg);

        // hand-ai ProviderNotFound shape also classifies as model.
        let pnf = AppError::validation_error("no provider is configured for model \"claude-3\"");
        assert!(sanitize_send_error(&pnf).to_lowercase().contains("model"));
    }

    // The per-run chat name embeds the job name so a human can spot which job a
    // chat came from; it is regenerated each run so two runs are distinct.
    #[test]
    fn prompt_chat_name_includes_job_name() {
        let name = prompt_chat_name("Daily digest");
        assert!(name.starts_with("Daily digest"));
    }

    // Job-level statistics are updated after a run: run_count increments,
    // last_status/last_run_at are set, failure_count tracks failures, and
    // next_run_at is preserved (the executor does NOT recompute cron).
    #[tokio::test]
    async fn updates_job_statistics_after_run() {
        let env = setup().await;
        let ok_id = install_shell_artifact(&env, "echo ok\nexit 0\n").await;
        let fail_id = install_shell_artifact(&env, "exit 1\n").await;

        // Success run.
        let mut success_job = make_job("job_stats", artifact_target(&ok_id)).await;
        success_job.next_run_at = Some(123_456);
        seed_job(&env, &success_job).await;

        env.executor
            .execute(&success_job, Trigger::Schedule)
            .await
            .unwrap();

        let after = env.executor.jobs.get("job_stats").await.unwrap().unwrap();
        assert_eq!(after.run_count, 1);
        assert_eq!(after.failure_count, 0);
        assert_eq!(after.last_status, Some(ExecutionStatus::Success));
        assert!(after.last_run_at.is_some());
        assert_eq!(
            after.next_run_at,
            Some(123_456),
            "executor preserves next_run_at; scheduler owns cron recompute"
        );

        // Failure run: failure_count increments.
        let fail_job = make_job("job_stats_fail", artifact_target(&fail_id)).await;
        seed_job(&env, &fail_job).await;
        env.executor
            .execute(&fail_job, Trigger::Schedule)
            .await
            .unwrap();
        let after_fail = env
            .executor
            .jobs
            .get("job_stats_fail")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after_fail.run_count, 1);
        assert_eq!(after_fail.failure_count, 1);
        assert_eq!(after_fail.last_status, Some(ExecutionStatus::Failed));
    }

    // ---- Manual run-now + shared in-flight re-entrancy (M2) ----

    /// `try_claim` is the single re-entrancy gate: the first claim succeeds, a
    /// second for the same id while the first guard is held is rejected, and
    /// dropping the guard frees the slot for re-claiming.
    #[tokio::test]
    async fn try_claim_claims_once_then_releases() {
        let env = setup().await;

        let first = env.executor.try_claim("job_x").await;
        assert!(first.is_some(), "first claim succeeds");
        assert_eq!(env.executor.in_flight_len().await, 1);

        let second = env.executor.try_claim("job_x").await;
        assert!(
            second.is_none(),
            "second claim for the same id is rejected while the first is held"
        );

        // A different id is independently claimable.
        let other = env.executor.try_claim("job_y").await;
        assert!(
            other.is_some(),
            "a different job is independently claimable"
        );
        drop(other);

        // Drop the first guard; the slot frees and is reusable.
        drop(first);
        for _ in 0..50 {
            if env.executor.in_flight_len().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(
            env.executor.in_flight_len().await,
            0,
            "guard drop frees the slot"
        );
        assert!(
            env.executor.try_claim("job_x").await.is_some(),
            "slot is reusable after release"
        );
    }

    // VAL-HISTORY-004 / VAL-HISTORY-013: a manual run produces exactly one
    // execution row with trigger = manual.
    #[tokio::test]
    async fn run_now_records_a_manual_execution_row() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo manual-ran\nexit 0\n").await;
        let job = make_job("job_manual", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let exec = env.executor.run_now(&job).await.expect("manual run");

        assert_eq!(exec.trigger, Trigger::Manual, "trigger is manual");
        assert_eq!(exec.status, ExecutionStatus::Success);
        assert!(exec.stdout.as_deref().unwrap().contains("manual-ran"));

        // Exactly one row, stamped manual on the wire ('manual').
        let rows =
            sqlx::query("SELECT trigger, status FROM job_executions WHERE job_id = 'job_manual'")
                .fetch_all(env.db.pool())
                .await
                .unwrap();
        assert_eq!(rows.len(), 1, "one manual trigger => exactly one row");
        let trigger: String = rows[0].try_get("trigger").unwrap();
        let status: String = rows[0].try_get("status").unwrap();
        assert_eq!(trigger, "manual");
        assert_eq!(status, "success");

        // The slot is released after the run.
        for _ in 0..50 {
            if env.executor.in_flight_len().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(env.executor.in_flight_len().await, 0);
    }

    // VAL-HISTORY-027: a disabled job (enabled = 0) still runs manually and
    // writes a manual row — disabling only stops automatic scheduling.
    #[tokio::test]
    async fn run_now_runs_disabled_job() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo disabled-but-ran\nexit 0\n").await;
        let mut job = make_job("job_disabled", artifact_target(&artifact_id)).await;
        job.enabled = false;
        seed_job(&env, &job).await;

        let exec = env
            .executor
            .run_now(&job)
            .await
            .expect("disabled job still runs manually");

        assert_eq!(exec.status, ExecutionStatus::Success);
        assert_eq!(exec.trigger, Trigger::Manual);

        let rows = read_rows(&env, "job_disabled").await;
        assert_eq!(rows.len(), 1, "disabled job's manual run writes one row");
        assert_eq!(rows[0].status, "success");
    }

    // VAL-HISTORY-028: while an execution is in flight, a second run-now is
    // rejected with a CONFLICT and writes NO second row (no concurrent running
    // rows). We hold the slot by claiming it directly (the guard simulates an
    // active run), then assert the second run-now bounces without persisting.
    #[tokio::test]
    async fn run_now_rejected_while_in_flight_writes_no_second_row() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo ran\nexit 0\n").await;
        let job = make_job("job_busy", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        // Simulate an execution already in flight by holding the job's slot.
        let _claim = env
            .executor
            .try_claim("job_busy")
            .await
            .expect("first claim succeeds");

        let err = env
            .executor
            .run_now(&job)
            .await
            .expect_err("run-now must be rejected while a run is in flight");
        assert_eq!(err.code, "CONFLICT");

        // No row was written by the rejected run (no concurrent running row).
        let rows = read_rows(&env, "job_busy").await;
        assert_eq!(rows.len(), 0, "a rejected run-now writes no execution row");

        // Releasing the held slot lets a subsequent run-now succeed (the gate is
        // not permanently stuck).
        drop(_claim);
        for _ in 0..50 {
            if env.executor.in_flight_len().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let exec = env
            .executor
            .run_now(&job)
            .await
            .expect("run-now succeeds once the slot is free");
        assert_eq!(exec.status, ExecutionStatus::Success);
    }

    // ---- History pruning wired into the execution write path (M2) ----

    /// Seed `count` already-finalized executions for a job WITHOUT going through
    /// a child process, with ascending `started_at` so FIFO order is well
    /// defined. Returns their ids oldest-first. Used to position a job's history
    /// right at the N boundary cheaply, then `execute` drives the wired prune.
    async fn seed_finalized(env: &TestEnv, job_id: &str, count: i64, base: i64) -> Vec<String> {
        let mut ids = Vec::new();
        for i in 0..count {
            let id = format!("{}_seed{}", job_id, i);
            let started = base + i;
            env.executor
                .executions
                .insert_running(&id, job_id, Trigger::Schedule, 1, started, started)
                .await
                .unwrap();
            env.executor
                .executions
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

    // VAL-HISTORY-021: a job sitting at exactly N executions is NOT pruned —
    // running `execute` once more would push it to N+1, but the wired prune
    // (after finalize) trims back to N, and the row count stays at N. Here we
    // seed N-1 then let `execute` write the Nth row through the real path; the
    // count is exactly N with nothing dropped (the oldest seed survives).
    #[tokio::test]
    async fn execute_at_exactly_n_keeps_all_rows() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo run\nexit 0\n").await;
        let job = make_job("job_exact", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        // Seed N-1 finalized rows in the past, then execute once to reach N.
        let limit = DEFAULT_EXECUTION_HISTORY_LIMIT;
        let oldest = seed_finalized(&env, "job_exact", limit - 1, 1).await;

        env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        let rows = read_rows(&env, "job_exact").await;
        assert_eq!(
            rows.len() as i64,
            limit,
            "exactly N rows: the wired prune keeps all when count == N"
        );
        // The oldest seeded row is still present (nothing pruned at exactly N).
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert!(
            ids.contains(&oldest[0].as_str()),
            "the oldest row must remain when the job is exactly at N"
        );
    }

    // VAL-HISTORY-022 / VAL-HISTORY-023: the (N+1)th execution drops exactly the
    // oldest row (FIFO by started_at) and the persisted count stabilizes at N —
    // no transient N+1 is left behind. We seed N rows, then execute once: the
    // wired prune (after finalize) trims the oldest back to N.
    #[tokio::test]
    async fn execute_n_plus_one_drops_oldest_and_stays_at_n() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo run\nexit 0\n").await;
        let job = make_job("job_fifo", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let limit = DEFAULT_EXECUTION_HISTORY_LIMIT;
        // Seed exactly N rows in the past; the next execute is the (N+1)th.
        let seeded = seed_finalized(&env, "job_fifo", limit, 1).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        let rows = read_rows(&env, "job_fifo").await;
        assert_eq!(
            rows.len() as i64,
            limit,
            "count stabilizes at N after the (N+1)th execution (no transient N+1)"
        );
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        // The oldest seeded row (FIFO) is gone; the newest execution remains.
        assert!(
            !ids.contains(&seeded[0].as_str()),
            "the oldest row (lowest started_at) must be pruned first (FIFO)"
        );
        assert!(
            ids.contains(&exec.id.as_str()),
            "the just-written newest execution must remain"
        );
    }

    // VAL-HISTORY-023 (count guard): even across several executions past the
    // limit, the persisted row count for the job never exceeds N at any point a
    // caller could observe it — the prune runs inside each `execute`.
    #[tokio::test]
    async fn execute_repeatedly_never_exceeds_n() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo run\nexit 0\n").await;
        let job = make_job("job_cap", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let limit = DEFAULT_EXECUTION_HISTORY_LIMIT;
        // Start one short of the limit, then drive several real executions over it.
        seed_finalized(&env, "job_cap", limit - 1, 1).await;

        for _ in 0..5 {
            env.executor.execute(&job, Trigger::Schedule).await.unwrap();
            let rows = read_rows(&env, "job_cap").await;
            assert!(
                rows.len() as i64 <= limit,
                "persisted count must never exceed N (got {})",
                rows.len()
            );
        }
        // After settling, it sits exactly at the cap.
        assert_eq!(read_rows(&env, "job_cap").await.len() as i64, limit);
    }

    // VAL-HISTORY-024: pruning is per-job — driving job A over the limit does not
    // touch job B's history. We park B at N rows, push A past the limit via
    // `execute`, and assert B is untouched.
    #[tokio::test]
    async fn execute_prune_is_per_job() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo run\nexit 0\n").await;
        let job_a = make_job("job_a", artifact_target(&artifact_id)).await;
        let job_b = make_job("job_b", artifact_target(&artifact_id)).await;
        seed_job(&env, &job_a).await;
        seed_job(&env, &job_b).await;

        let limit = DEFAULT_EXECUTION_HISTORY_LIMIT;
        // A is at N; B is at N. Executing A once must prune only A.
        seed_finalized(&env, "job_a", limit, 1).await;
        let b_ids = seed_finalized(&env, "job_b", limit, 1).await;

        env.executor
            .execute(&job_a, Trigger::Schedule)
            .await
            .unwrap();

        assert_eq!(
            read_rows(&env, "job_a").await.len() as i64,
            limit,
            "job A is pruned back to N"
        );
        let b_rows = read_rows(&env, "job_b").await;
        assert_eq!(
            b_rows.len() as i64,
            limit,
            "job B is untouched by job A's prune"
        );
        let b_remaining: Vec<&str> = b_rows.iter().map(|r| r.id.as_str()).collect();
        assert!(
            b_remaining.contains(&b_ids[0].as_str()),
            "job B's oldest row survives — prune is isolated per job"
        );
    }

    // VAL-HISTORY-025: a still-running row for the same job is never pruned by an
    // execute that overflows the finalized history. We inject an oldest running
    // row directly, fill the finalized history to N, then execute once more; the
    // running row must survive (prune only trims finalized rows).
    #[tokio::test]
    async fn execute_prune_never_drops_running_row() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo run\nexit 0\n").await;
        let job = make_job("job_keep_running", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let limit = DEFAULT_EXECUTION_HISTORY_LIMIT;
        // An in-flight (running) row with the OLDEST started_at — the prime
        // candidate for FIFO removal, which must nonetheless survive.
        env.executor
            .executions
            .insert_running(
                "inflight_old",
                "job_keep_running",
                Trigger::Schedule,
                1,
                0,
                0,
            )
            .await
            .unwrap();
        // Fill finalized history to the cap, then push one more through execute.
        seed_finalized(&env, "job_keep_running", limit, 1).await;

        env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        let rows = read_rows(&env, "job_keep_running").await;
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert!(
            ids.contains(&"inflight_old"),
            "the oldest still-running row must never be pruned"
        );
        // N finalized rows + 1 surviving running row.
        assert_eq!(
            rows.len() as i64,
            limit + 1,
            "running rows are not counted against the finalized cap"
        );
    }

    // VAL-HISTORY-026: deleting a job cascades to its job_executions rows
    // (FK ON DELETE CASCADE; sqlx keeps foreign_keys = ON). We run the job once
    // through the real path, assert a row exists, delete the job, and assert the
    // raw execution count is zero.
    #[tokio::test]
    async fn delete_job_cascades_executions() {
        let env = setup().await;

        // Confirm FK enforcement is ON for this connection (cascade depends on it).
        let fk: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(env.db.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(fk, 1, "FK enforcement must be ON for cascade delete");

        let artifact_id = install_shell_artifact(&env, "echo run\nexit 0\n").await;
        let job = make_job("job_cascade", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        assert_eq!(
            read_rows(&env, "job_cascade").await.len(),
            1,
            "the run wrote one execution row"
        );

        env.executor.jobs.delete("job_cascade").await.unwrap();

        let count: i64 = sqlx::query("SELECT COUNT(*) FROM job_executions WHERE job_id = $1")
            .bind("job_cascade")
            .fetch_one(env.db.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(count, 0, "deleting the job cascades away its executions");
    }

    // ---- Realtime `job_executed` event contract (M2) ----

    // The event channel name and payload shape are the wire contract the
    // frontend listens on; a typo or a casing drift silently breaks the
    // realtime detail/list refresh. Pin both: channel name and the camelCase
    // payload keys with a snake_case `status` value.
    #[test]
    fn job_executed_event_name_is_pinned() {
        assert_eq!(JOB_EXECUTED_EVENT, "job_executed");
    }

    #[test]
    fn job_executed_payload_serializes_camel_case_with_snake_status() {
        let payload = JobExecutedEvent {
            job_id: "job_1".to_string(),
            execution_id: "exec_1".to_string(),
            status: ExecutionStatus::Running,
        };
        let value = serde_json::to_value(&payload).unwrap();
        assert_eq!(value["jobId"], "job_1");
        assert_eq!(value["executionId"], "exec_1");
        assert_eq!(value["status"], "running");

        let terminal = JobExecutedEvent {
            job_id: "job_1".to_string(),
            execution_id: "exec_1".to_string(),
            status: ExecutionStatus::Success,
        };
        assert_eq!(
            serde_json::to_value(&terminal).unwrap()["status"],
            "success"
        );
    }

    // Without an `AppHandle` (the unit-test wiring), `execute` still runs and
    // persists exactly one terminal row — emit is a clean no-op, so the existing
    // executor tests are never destabilized by the new event path.
    #[tokio::test]
    async fn execute_without_app_handle_emits_nothing_and_still_records() {
        let env = setup().await;
        // `setup` builds the executor via `from_db` — no AppHandle attached.
        assert!(
            env.executor.app_handle.is_none(),
            "the test executor has no AppHandle, so emit must be a no-op"
        );
        let artifact_id = install_shell_artifact(&env, "echo ran\nexit 0\n").await;
        let job = make_job("job_noemit", artifact_target(&artifact_id)).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Success);
        let rows = read_rows(&env, "job_noemit").await;
        assert_eq!(rows.len(), 1, "one terminal row even with emit as no-op");
        assert_eq!(rows[0].status, "success");
    }

    // The in-flight set is SHARED across every clone of an executor (it lives
    // behind an `Arc`): a slot claimed on one handle is seen as occupied by a
    // clone. This is what makes the scheduler (which clones the executor) and
    // the run-now command (a separate State handle, also a clone) share ONE
    // gate — the core of VAL-HISTORY-028.
    #[tokio::test]
    async fn in_flight_set_is_shared_across_clones() {
        let env = setup().await;
        let clone = env.executor.clone();

        let claim = env.executor.try_claim("job_shared").await;
        assert!(claim.is_some(), "claimed on the original handle");

        // The clone sees the same job as already in flight.
        assert!(
            clone.try_claim("job_shared").await.is_none(),
            "a clone shares the same in-flight set (claim is visible across handles)"
        );
        assert_eq!(clone.in_flight_len().await, 1, "one shared set, one entry");

        drop(claim);
        for _ in 0..50 {
            if clone.in_flight_len().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert!(
            clone.try_claim("job_shared").await.is_some(),
            "releasing on one handle frees the slot for the other"
        );
    }

    // ---- agent dispatch (VAL-TARGET-006 / 020 / 021 / 024 / 031) ----

    use crate::services::{AgentService, AgentSessionService};
    use crate::storage::types::AgentSessionMessage;

    /// Wire REAL agent collaborators onto the env's executor, all sharing the
    /// env's temp DB. `app_data_dir` is the env's temp dir (the coding-agent
    /// session's `base_dir`), so the offline pre-flight + transcript-
    /// classification paths exercised here stay inside the test sandbox. The
    /// real LLM run (`drive_agent_run` + `send_message`) is not driven offline —
    /// it needs a live model — so these tests cover everything up to and
    /// including session construction plus the pure classification seam.
    fn with_agent_services(env: TestEnv) -> TestEnv {
        let provider_service = Arc::new(ProviderService::new(env.db.clone()));
        let agent_service = Arc::new(AgentService::new(env.db.clone()));
        let agent_session_service = Arc::new(AgentSessionService::new(env.db.clone()));
        let app_data_dir = env._temp_dir.path().to_path_buf();

        let executor = env.executor.clone().with_agent_services(
            agent_service,
            agent_session_service,
            provider_service,
            app_data_dir,
        );

        TestEnv { executor, ..env }
    }

    /// Seed an `agents` template row (the thing `JobTarget::Agent.agent_id`
    /// references). Returns the agent id.
    async fn seed_agent(env: &TestEnv, model: Option<&str>) -> String {
        let service = AgentService::new(env.db.clone());
        let agent = service
            .create_agent(
                format!("agent-{}", uuid::Uuid::new_v4()),
                model.map(str::to_string),
                Some(0.5),
                None,
                None,
                None,
                Some(1024),
                Some("You are a helpful agent.".to_string()),
                None,
                None,
            )
            .await
            .expect("seed agent");
        agent.id
    }

    /// Seed a `models` catalog row so the resolver can find a provider for a
    /// model id. `seed_provider` already created the provider row.
    async fn seed_model(env: &TestEnv, provider_id: &str, model_id: &str) {
        let repo = crate::storage::ModelRepository::new(env.db.clone());
        let model = crate::storage::types::Model {
            id: model_id.to_string(),
            provider_id: provider_id.to_string(),
            name: model_id.to_string(),
            context_length: Some(4096),
            output_max_tokens: Some(2048),
            supported_features: None,
            description: None,
            input_modalities: None,
            output_modalities: None,
            metadata: None,
            pricing: None,
            url: None,
            supported_parameters: None,
            default_parameters: None,
            max_parameters: None,
            supported_methods: Some(vec!["completions".to_string()]),
            model_created_at: None,
            enabled: true,
            favorite: false,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };
        repo.create_models(&[model]).await.expect("seed model");
    }

    fn agent_target(agent_id: &str, initial_message: &str) -> JobTarget {
        JobTarget::Agent {
            agent_id: agent_id.to_string(),
            initial_message: initial_message.to_string(),
            project_id: None,
        }
    }

    // VAL-TARGET-020 (end-to-end): an agent target whose template id does not
    // resolve is failed with the distinct "template missing" message — and NO
    // session is created (the pre-flight short-circuits), so no result_ref.
    #[tokio::test]
    async fn agent_missing_template_fails_before_session() {
        let env = with_agent_services(setup().await);
        let target = agent_target("ghost-agent", "go");
        let job = make_job("job_a_missing", target).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(agent_failure_message(AgentFailure::TemplateMissing).as_str())
        );
        assert!(
            exec.result_ref.is_none(),
            "no session created on template-missing pre-flight fail"
        );
        // No agent session row leaked into the DB.
        let sessions: i64 = sqlx::query("SELECT COUNT(*) FROM agent_sessions")
            .fetch_one(env.db.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(sessions, 0);
    }

    // VAL-TARGET-021 (end-to-end): a template that exists but has no model
    // selected is failed with a model-class message — distinct from the
    // template-missing message — and no session is created.
    #[tokio::test]
    async fn agent_template_without_model_fails_with_model_class_error() {
        let env = with_agent_services(setup().await);
        let agent_id = seed_agent(&env, None).await;
        let job = make_job("job_a_nomodel", agent_target(&agent_id, "go")).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(agent_failure_message(AgentFailure::NoModel).as_str())
        );
        // Distinct from the template-missing class (VAL-TARGET-020 vs 021).
        assert_ne!(
            exec.error.as_deref(),
            Some(agent_failure_message(AgentFailure::TemplateMissing).as_str())
        );
        assert!(exec.result_ref.is_none());
    }

    // VAL-TARGET-021 (end-to-end): a template whose model is served by no
    // provider (provider/model removed) is failed with the model-removed class
    // — distinct from the template-missing class — and no session is created.
    #[tokio::test]
    async fn agent_model_served_by_no_provider_fails_with_model_class_error() {
        let env = with_agent_services(setup().await);
        // A model id that no provider in the (empty) catalog serves.
        let agent_id = seed_agent(&env, Some("gone-model")).await;
        let job = make_job("job_a_modelgone", agent_target(&agent_id, "go")).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(agent_failure_message(AgentFailure::ModelRemoved).as_str())
        );
        assert_ne!(
            exec.error.as_deref(),
            Some(agent_failure_message(AgentFailure::TemplateMissing).as_str())
        );
        assert!(exec.result_ref.is_none());
    }

    // VAL-TARGET-021 (resolution): a model that survives ONLY under a DISABLED
    // provider is a config-class failure (the run could not proceed), distinct
    // from a clean "model removed". No enabled provider serves the model.
    #[tokio::test]
    async fn agent_resolver_disabled_only_match_is_config_error() {
        let env = with_agent_services(setup().await);
        seed_provider(&env, "prov_off", "openai", false, "sk-live-abcd").await;
        seed_model(&env, "prov_off", "shared-model").await;
        let agent_id = seed_agent(&env, Some("shared-model")).await;
        let job = make_job("job_a_off", agent_target(&agent_id, "go")).await;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(
            exec.error.as_deref(),
            Some(agent_failure_message(AgentFailure::ConfigError).as_str())
        );
        assert!(exec.result_ref.is_none());
    }

    // VAL-TARGET-006 (resolution): an enabled provider serving the template's
    // model is resolved as the run's provider. Exercised directly through the
    // pre-flight resolver (a real run needs an LLM).
    #[tokio::test]
    async fn agent_resolver_prefers_enabled_provider_serving_model() {
        let env = with_agent_services(setup().await);
        seed_provider(&env, "prov_on", "openai", true, "sk-live-abcd").await;
        seed_model(&env, "prov_on", "live-model").await;

        let resolved = env
            .executor
            .resolve_agent_provider(&Some("live-model".to_string()))
            .await
            .expect("an enabled provider serving the model resolves");
        assert_eq!(resolved, "prov_on");
    }

    // Without the agent collaborators, the resolver reports a config error
    // rather than panicking.
    #[tokio::test]
    async fn agent_resolver_unwired_is_config_error() {
        let env = setup().await;
        let err = env
            .executor
            .resolve_agent_provider(&Some("m".to_string()))
            .await
            .expect_err("unwired resolver fails");
        assert_eq!(err, AgentFailure::ConfigError);
    }

    // VAL-TARGET-020 / 021: each agent failure class maps to a stable, distinct,
    // secret-free message. Template-missing is distinguishable from every
    // model/config class.
    #[test]
    fn agent_failure_messages_are_distinct_and_carry_no_secret() {
        let classes = [
            AgentFailure::TemplateMissing,
            AgentFailure::NoModel,
            AgentFailure::ModelRemoved,
            AgentFailure::ConfigError,
        ];
        let messages: Vec<String> = classes.iter().map(|c| agent_failure_message(*c)).collect();
        for msg in &messages {
            assert!(!msg.is_empty());
            assert!(!msg.contains("sk-"));
        }
        // Template-missing is distinct from each model/config class (020 vs 021).
        let template_missing = agent_failure_message(AgentFailure::TemplateMissing);
        for other in [
            AgentFailure::NoModel,
            AgentFailure::ModelRemoved,
            AgentFailure::ConfigError,
        ] {
            assert_ne!(template_missing, agent_failure_message(other));
        }
    }

    // VAL-TARGET-027 (agent path): each AppError code maps to a stable message
    // that contains no raw URL, Bearer token, or key fragment.
    #[test]
    fn sanitize_agent_dispatch_error_drops_raw_detail() {
        let leaky = AppError::auth_error(
            "POST https://api.openai.com/v1 failed: Authorization: Bearer sk-live-SECRET",
        );
        let sanitized = sanitize_agent_dispatch_error(&leaky);
        assert!(!sanitized.contains("sk-live-SECRET"));
        assert!(!sanitized.contains("Bearer"));
        assert!(!sanitized.contains("https://"));
        assert!(sanitized.to_lowercase().contains("authentication"));
    }

    #[test]
    fn sanitize_agent_dispatch_error_distinguishes_model_resolution() {
        let model_err = AppError::validation_error(
            "chat_engine: model 'gpt-4' not registered under provider 'openai'",
        );
        let model_msg = sanitize_agent_dispatch_error(&model_err);
        assert!(model_msg.to_lowercase().contains("model"));

        let generic = AppError::validation_error("agent session has no model_id selected");
        assert_ne!(model_msg, sanitize_agent_dispatch_error(&generic));
    }

    // ---- transcript classification (VAL-TARGET-024 / 031) ----

    /// Build a persisted assistant transcript row with the given `stopReason`.
    fn assistant_row(seq: i64, stop_reason: &str) -> AgentSessionMessage {
        AgentSessionMessage {
            id: format!("m{}", seq),
            session_id: "s".to_string(),
            seq,
            role: "assistant".to_string(),
            payload: serde_json::json!({
                "role": "assistant",
                "content": [],
                "stopReason": stop_reason,
            }),
            created_at: 0,
        }
    }

    /// Build a persisted user transcript row carrying `content` verbatim.
    fn user_row(seq: i64, content: &str) -> AgentSessionMessage {
        AgentSessionMessage {
            id: format!("m{}", seq),
            session_id: "s".to_string(),
            seq,
            role: "user".to_string(),
            payload: serde_json::json!({
                "role": "user",
                "content": content,
            }),
            created_at: 0,
        }
    }

    // VAL-TARGET-024: a transcript whose terminal assistant turn carries
    // `stopReason == "error"` classifies as an in-band error (failed), even
    // though the run returned Ok and the transcript is persisted.
    #[test]
    fn classify_transcript_in_band_error_is_failed() {
        let transcript = vec![user_row(0, "hi"), assistant_row(1, "error")];
        assert_eq!(
            classify_agent_transcript(Ok(&transcript)),
            AgentRunResult::InBandError
        );
    }

    // A normal terminal assistant turn (stopReason "stop") is a success.
    #[test]
    fn classify_transcript_normal_stop_is_success() {
        let transcript = vec![user_row(0, "hi"), assistant_row(1, "stop")];
        assert_eq!(
            classify_agent_transcript(Ok(&transcript)),
            AgentRunResult::Success
        );
    }

    // A user-only partial transcript (no assistant turn) is NOT an in-band
    // error: a run-level failure would have surfaced via the error envelope.
    #[test]
    fn classify_transcript_user_only_is_success() {
        let transcript = vec![user_row(0, "hi")];
        assert_eq!(
            classify_agent_transcript(Ok(&transcript)),
            AgentRunResult::Success
        );
    }

    // The LAST assistant turn wins: an earlier error followed by a normal turn
    // (e.g. a recovered multi-turn run) classifies on the terminal turn.
    #[test]
    fn classify_transcript_uses_last_assistant_turn() {
        let transcript = vec![
            user_row(0, "hi"),
            assistant_row(1, "error"),
            assistant_row(2, "stop"),
        ];
        assert_eq!(
            classify_agent_transcript(Ok(&transcript)),
            AgentRunResult::Success
        );
    }

    // A transcript read failure cannot prove an in-band error; default to a
    // non-error completion (the run already closed without an envelope).
    #[test]
    fn classify_transcript_read_error_defaults_to_success() {
        let err = AppError::internal_error("db read failed");
        assert_eq!(
            classify_agent_transcript(Err::<&Vec<AgentSessionMessage>, _>(&err)),
            AgentRunResult::Success
        );
    }

    // VAL-TARGET-031: the unicode initial instruction is preserved byte-for-byte
    // in a persisted user turn — classification reads stopReason, never mutating
    // the user content. (The runtime persists the user turn verbatim; here we
    // assert the transcript shape the executor reads back is unicode-safe.)
    #[test]
    fn classify_transcript_preserves_unicode_user_content() {
        let unicode = "你好，请总结今日要点 🌟 — résumé";
        let transcript = vec![user_row(0, unicode), assistant_row(1, "stop")];
        // The first user turn carries the instruction verbatim.
        assert_eq!(
            transcript[0]
                .payload
                .get("content")
                .and_then(|c| c.as_str()),
            Some(unicode)
        );
        assert_eq!(
            classify_agent_transcript(Ok(&transcript)),
            AgentRunResult::Success
        );
    }

    // ---- oneshot run sink (the executor-side CodingRunSink) ----

    // A clean close fires the oneshot with `None` (no run-level error). Drives
    // the signal callbacks directly, exactly as the driver invokes the sink.
    #[tokio::test]
    async fn oneshot_signal_closed_carries_none() {
        let signal = build_oneshot_signal();
        // Drive the callbacks as the driver would: emit an event, then close.
        (signal.on_event)(serde_json::json!({ "sessionId": "s", "event": {} }));
        (signal.on_closed)(serde_json::json!({ "sessionId": "s" }));

        let result = signal.rx.await.expect("oneshot resolves on close");
        assert_eq!(result, None, "a clean close carries no error");
    }

    // A run-level error envelope (on_error) before close is forwarded as the
    // close signal's payload (its sanitized message), so the dispatch can record
    // a failed outcome from it.
    #[tokio::test]
    async fn oneshot_signal_forwards_error_envelope_on_close() {
        let signal = build_oneshot_signal();
        (signal.on_error)(serde_json::json!({
            "sessionId": "s",
            "error": { "code": "AUTH_ERROR", "message": "the provider rejected the request", "hint": null },
        }));
        (signal.on_closed)(serde_json::json!({ "sessionId": "s" }));

        let result = signal.rx.await.expect("oneshot resolves on close");
        assert_eq!(
            result.as_deref(),
            Some("the provider rejected the request"),
            "the sanitized envelope message reaches the close signal"
        );
    }

    // `oneshot_run_sink` assembles the same signal into a real `CodingRunSink`
    // (the shape `drive_agent_run` consumes) and still yields a usable receiver —
    // proving the production builder wires the signal through unchanged.
    #[tokio::test]
    async fn oneshot_run_sink_produces_a_usable_runsink() {
        let (_sink, _rx) = oneshot_run_sink();
        // Constructing the production pair must not panic; the signal semantics
        // are covered by the `build_oneshot_signal` tests above.
    }

    // ---- exec timeout (VAL-ROBUST-004/005/006/007/008/022, VAL-TARGET-036) ----

    // `exec_timeout_secs == 0` (or, defensively, negative) means no bound — no
    // `tokio::time::timeout` wrapper is applied. `> 0` maps to a `Duration` of
    // that many seconds. This is the single switch that decides whether a
    // dispatch is bounded (VAL-ROBUST-008).
    #[test]
    fn timeout_duration_zero_is_unbounded_positive_is_bounded() {
        assert_eq!(timeout_duration(0), None, "0 => no timeout bound");
        assert_eq!(timeout_duration(-5), None, "negative => no timeout bound");
        assert_eq!(
            timeout_duration(7),
            Some(Duration::from_secs(7)),
            "positive => a Duration of that many seconds"
        );
    }

    // The timeout error message names the configured threshold and carries no
    // secret material — it is persisted on the row and shown in the UI.
    #[test]
    fn timeout_error_message_names_threshold() {
        let msg = timeout_error_message(30);
        assert!(msg.to_lowercase().contains("timeout"));
        assert!(msg.contains("30"));
        assert!(!msg.contains("sk-"));
    }

    // `timeout_secs_of` recovers the whole-seconds view of a `Duration` (the
    // agent path holds only the Duration when it builds its timeout outcome).
    #[test]
    fn timeout_secs_of_recovers_whole_seconds() {
        assert_eq!(timeout_secs_of(Duration::from_secs(12)), 12);
        assert_eq!(timeout_secs_of(Duration::from_millis(3500)), 3);
    }

    // `DispatchOutcome::timeout` is a terminal `timeout` outcome carrying the
    // threshold-naming error and no process output / result ref.
    #[test]
    fn dispatch_outcome_timeout_shape() {
        let outcome = DispatchOutcome::timeout(15);
        assert_eq!(outcome.status, ExecutionStatus::Timeout);
        assert!(outcome.stdout.is_none());
        assert!(outcome.stderr.is_none());
        assert!(outcome.exit_code.is_none());
        assert!(outcome.result_ref.is_none());
        assert!(outcome.error.as_deref().unwrap().contains("15"));
    }

    // VAL-ROBUST-004 + VAL-ROBUST-005 + VAL-ROBUST-007 + VAL-TARGET-036: an
    // artifact that runs far longer than the job's `exec_timeout_secs` is
    // interrupted near the threshold and recorded as `timeout` (NOT the
    // artifact's generalized `failed`), with a duration close to the bound and a
    // timeout-naming error. The bound (1s) is enforced by the execution-side
    // timer; the spawned `sleep 30` child is reaped by `kill_on_drop(true)`, so
    // the call returns promptly rather than hanging for the full sleep — that
    // prompt return is the observable proof the OS child did not keep the future
    // alive (VAL-ROBUST-005). Because the threshold (1s) is unrelated to any
    // 30s scheduler tick, this also pins the execution-side timing
    // (VAL-ROBUST-007).
    #[tokio::test]
    async fn artifact_exceeding_timeout_records_timeout_not_failed() {
        let env = setup().await;
        // Sleeps far past the 1s bound; if the timer or kill failed this test
        // would hang for ~30s instead of returning near the threshold.
        let artifact_id = install_shell_artifact(&env, "sleep 30\necho done\nexit 0\n").await;
        let mut job = make_job("job_timeout", artifact_target(&artifact_id)).await;
        job.exec_timeout_secs = 1;
        seed_job(&env, &job).await;

        let start = std::time::Instant::now();
        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        let wall = start.elapsed();

        assert_eq!(
            exec.status,
            ExecutionStatus::Timeout,
            "an over-budget artifact is recorded as timeout, not the artifact's failed"
        );
        // Duration is recorded and close to the 1s threshold (well under the
        // 30s sleep), proving interruption near the bound.
        let duration = exec.duration.expect("timeout rows record a duration");
        assert!(
            (800..3_000).contains(&duration),
            "duration should be near the 1s threshold, got {duration}ms"
        );
        // The future returned promptly — the killed child did not hold it open
        // for the full sleep.
        assert!(
            wall < Duration::from_secs(10),
            "the timed-out dispatch must return near the threshold, took {wall:?}"
        );
        let err = exec.error.as_deref().unwrap_or_default();
        assert!(
            err.to_lowercase().contains("timeout"),
            "error explains the timeout, got: {err}"
        );

        let rows = read_rows(&env, "job_timeout").await;
        assert_eq!(rows.len(), 1, "one trigger => exactly one row");
        assert_eq!(rows[0].status, "timeout");
        assert!(rows[0].ended_at.is_some());
    }

    // VAL-ROBUST-008: with `exec_timeout_secs == 0` the dispatch is NOT wrapped
    // in a timeout and runs to its natural end — a brief `sleep` artifact
    // completes successfully rather than being interrupted.
    #[tokio::test]
    async fn artifact_with_zero_timeout_runs_to_completion() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "sleep 0.3\necho slept\nexit 0\n").await;
        let job = make_job("job_unbounded", artifact_target(&artifact_id)).await;
        // make_job defaults exec_timeout_secs to 0 — assert it explicitly so the
        // intent of this test is unmistakable.
        assert_eq!(job.exec_timeout_secs, 0, "0 = unbounded");
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(
            exec.status,
            ExecutionStatus::Success,
            "an unbounded run completes naturally, error: {:?}",
            exec.error
        );
        assert!(exec.stdout.as_deref().unwrap().contains("slept"));
    }

    // A fast artifact under a generous timeout still completes normally — the
    // timeout wrapper does not perturb a run that finishes well inside the bound.
    #[tokio::test]
    async fn artifact_within_timeout_completes_normally() {
        let env = setup().await;
        let artifact_id = install_shell_artifact(&env, "echo quick\nexit 0\n").await;
        let mut job = make_job("job_under", artifact_target(&artifact_id)).await;
        job.exec_timeout_secs = 30;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Success);
        assert!(exec.stdout.as_deref().unwrap().contains("quick"));
    }

    /// Bind a local TCP listener that ACCEPTS connections but never writes a
    /// byte, so any HTTP client awaiting a response hangs deterministically.
    /// Returns its `http://addr` base url plus the acceptor task handle (kept
    /// alive so accepted sockets are not RST'd for the test's duration).
    async fn spawn_hanging_http_server() -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let acceptor = tokio::spawn(async move {
            let mut held = Vec::new();
            loop {
                match listener.accept().await {
                    Ok((sock, _)) => held.push(sock),
                    Err(_) => break,
                }
            }
        });
        (format!("http://{addr}"), acceptor)
    }

    // VAL-ROBUST-022 (prompt leg): a prompt whose LLM call does not return within
    // the job's `exec_timeout_secs` is interrupted at the threshold and recorded
    // as `timeout` (not `failed`). The send is made to hang deterministically by
    // pointing an openai-compatible provider at a TCP server that never responds;
    // the executor's short bound fires first. Dropping the timed-out future
    // cancels the in-flight send, and the chat + persisted user message stay
    // reachable via `result_ref` — no orphan running session (VAL-ROBUST-006).
    #[tokio::test]
    async fn prompt_exceeding_timeout_records_timeout_not_failed() {
        let (base_url, _acceptor) = spawn_hanging_http_server().await;

        let env = with_prompt_services(setup().await);
        seed_provider(
            &env,
            "prov_phang",
            "openai-compatible",
            true,
            "sk-live-abcd",
        )
        .await;
        sqlx::query("UPDATE providers SET base_url = $1 WHERE id = $2")
            .bind(&base_url)
            .bind("prov_phang")
            .execute(env.db.pool())
            .await
            .unwrap();

        let target = JobTarget::Prompt {
            provider_id: "prov_phang".to_string(),
            model_id: "hang-model".to_string(),
            prompt: "summarize".to_string(),
            session_strategy: SessionStrategy::NewSession,
        };
        let mut job = make_job("job_prompt_timeout", target).await;
        job.exec_timeout_secs = 1;
        seed_job(&env, &job).await;

        let start = std::time::Instant::now();
        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        let wall = start.elapsed();

        assert_eq!(
            exec.status,
            ExecutionStatus::Timeout,
            "an over-budget prompt send is recorded as timeout, error: {:?}",
            exec.error
        );
        assert!(
            wall < Duration::from_secs(10),
            "the timed-out prompt dispatch must return near the threshold, took {wall:?}"
        );
        let err = exec.error.as_deref().unwrap_or_default();
        assert!(
            err.to_lowercase().contains("timeout"),
            "error explains the timeout, got: {err}"
        );

        let rows = read_rows(&env, "job_prompt_timeout").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "timeout");
    }

    // VAL-ROBUST-006 + VAL-ROBUST-022: an agent run that does not close within
    // the job's `exec_timeout_secs` is interrupted via the cooperative
    // `abort_run` and recorded as `timeout`. The abort flips the same cancel
    // token the driving coding-agent `send_message` is on, so the agent loop
    // unwinds at its next await point and the driver's `send_message` resolves —
    // proving no orphan running turn is left behind (the dispatch returns
    // promptly near the threshold rather than hanging for the full network
    // stall). The minted session stays referenced so its (partial) transcript is
    // reachable.
    //
    // The run is driven through the REAL executor `execute` path (which builds a
    // genuine coding-agent session and calls `drive_agent_run`); it is made to
    // hang deterministically by pointing the provider at a local TCP listener
    // that ACCEPTS connections but never responds, so the real client blocks
    // awaiting the SSE response until the executor's short bound fires.
    #[tokio::test]
    async fn agent_exceeding_timeout_aborts_and_records_timeout() {
        let (base_url, _acceptor) = spawn_hanging_http_server().await;

        let env = with_agent_services(setup().await);
        // An openai-compatible provider whose model synthesizes to an OpenAI
        // completions template; the base_url override points at the hanging TCP
        // server so the run blocks on the network.
        seed_provider(&env, "prov_hang", "openai-compatible", true, "sk-live-abcd").await;
        // Override the seeded provider's base_url to our hanging listener.
        sqlx::query("UPDATE providers SET base_url = $1 WHERE id = $2")
            .bind(&base_url)
            .bind("prov_hang")
            .execute(env.db.pool())
            .await
            .unwrap();
        seed_model(&env, "prov_hang", "hang-model").await;

        let agent_id = seed_agent(&env, Some("hang-model")).await;

        let mut job = make_job("job_agent_timeout", agent_target(&agent_id, "go")).await;
        job.exec_timeout_secs = 1;
        seed_job(&env, &job).await;

        let start = std::time::Instant::now();
        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        let wall = start.elapsed();

        assert_eq!(
            exec.status,
            ExecutionStatus::Timeout,
            "an over-budget agent run is recorded as timeout, error: {:?}",
            exec.error
        );
        let err = exec.error.as_deref().unwrap_or_default();
        assert!(
            err.to_lowercase().contains("timeout"),
            "error explains the timeout, got: {err}"
        );
        // The timed-out dispatch returns near the 1s bound — the cooperative
        // abort unwound the driving `send_message` rather than leaving an orphan
        // turn that would have kept the call blocked on the network stall
        // (VAL-ROBUST-006 / VAL-ROBUST-022).
        assert!(
            wall < Duration::from_secs(10),
            "the timed-out agent dispatch must return near the threshold, took {wall:?}"
        );
        // The minted session stays referenced so its (partial) transcript is
        // reachable.
        assert!(
            exec.result_ref.is_some(),
            "a timed-out agent run still references its session"
        );

        // The persisted execution row is terminal `timeout`.
        let rows = read_rows(&env, "job_agent_timeout").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "timeout");
    }

    // ---- retry backoff (VAL-ROBUST-009..018/023..025, VAL-HISTORY-032) ----

    /// Install a shell artifact that fails its first `fail_count` invocations and
    /// succeeds thereafter, using a counter file in a temp dir to persist the
    /// invocation count ACROSS retries (each attempt is a fresh child process).
    /// A huge `fail_count` makes it always fail. Returns the installed artifact
    /// id; the counter file path is caller-chosen so distinct jobs never collide.
    async fn install_fail_then_succeed_artifact(
        env: &TestEnv,
        counter_path: &std::path::Path,
        fail_count: i64,
    ) -> String {
        // The script reads/increments a counter; while count <= fail_count it
        // exits non-zero, otherwise it prints and exits 0. No locking is needed:
        // the executor never runs two attempts of one envelope concurrently.
        let script = format!(
            r#"COUNTER="{}"
n=0
if [ -f "$COUNTER" ]; then n=$(cat "$COUNTER"); fi
n=$((n + 1))
echo "$n" > "$COUNTER"
echo "attempt-$n"
if [ "$n" -le {} ]; then
  echo "fail-$n" 1>&2
  exit 1
fi
exit 0
"#,
            counter_path.display(),
            fail_count
        );
        install_shell_artifact(env, &script).await
    }

    /// Read the integer in a counter file, or 0 if it does not exist.
    fn read_counter(path: &std::path::Path) -> i64 {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok())
            .unwrap_or(0)
    }

    // ---- backoff_delay pure law (exponential, base 2; ROBUST-009/012) ----

    // The gap before the k-th retry is base * 2^(k-1): base, 2·base, 4·base, …
    // (`attempt` is the 1-based number of the attempt that just failed).
    #[test]
    fn backoff_delay_is_exponential_base_two() {
        let base = 4i64;
        assert_eq!(backoff_delay(base, 1), Duration::from_secs(4)); // before retry 1
        assert_eq!(backoff_delay(base, 2), Duration::from_secs(8)); // before retry 2
        assert_eq!(backoff_delay(base, 3), Duration::from_secs(16)); // before retry 3
        assert_eq!(backoff_delay(base, 4), Duration::from_secs(32));

        // Adjacent backoffs grow by exactly a factor of 2.
        let d1 = backoff_delay(base, 1).as_secs();
        let d2 = backoff_delay(base, 2).as_secs();
        let d3 = backoff_delay(base, 3).as_secs();
        assert_eq!(d2, d1 * 2);
        assert_eq!(d3, d2 * 2);
    }

    // A zero (or negative) base collapses every backoff to zero: retries run
    // back-to-back, with no inter-attempt wait (VAL-ROBUST-012).
    #[test]
    fn backoff_delay_zero_base_is_zero() {
        for attempt in 1..=5 {
            assert_eq!(backoff_delay(0, attempt), Duration::ZERO);
            assert_eq!(backoff_delay(-10, attempt), Duration::ZERO);
        }
    }

    // A large attempt count cannot overflow the shift; the delay saturates
    // rather than panicking.
    #[test]
    fn backoff_delay_saturates_on_large_attempt() {
        let d = backoff_delay(i64::MAX, 100);
        assert!(d.as_secs() > 0, "saturated delay is still positive");
    }

    // ---- failure_count_update mapping (ROBUST-016/017/018/024) ----

    // Scheduled success resets the continuous-failure chain; scheduled
    // failure/timeout increments it; a manual run never touches it.
    #[test]
    fn failure_count_update_maps_trigger_and_status() {
        assert_eq!(
            failure_count_update(Trigger::Schedule, ExecutionStatus::Success),
            FailureCountUpdate::Reset
        );
        assert_eq!(
            failure_count_update(Trigger::Schedule, ExecutionStatus::Failed),
            FailureCountUpdate::Increment
        );
        assert_eq!(
            failure_count_update(Trigger::Schedule, ExecutionStatus::Timeout),
            FailureCountUpdate::Increment
        );
        // Manual never participates, whatever the outcome.
        for status in [
            ExecutionStatus::Success,
            ExecutionStatus::Failed,
            ExecutionStatus::Timeout,
        ] {
            assert_eq!(
                failure_count_update(Trigger::Manual, status),
                FailureCountUpdate::Unchanged
            );
        }
    }

    // ---- continuous-failure notification decision (ROBUST-019/020/021) ----

    // The threshold constant is the documented value (3): the body / tests and
    // the validator all key off this number.
    #[test]
    fn failure_notify_threshold_is_three() {
        assert_eq!(FAILURE_NOTIFY_THRESHOLD, 3);
    }

    // VAL-ROBUST-019: a scheduled failure chain fires EXACTLY once — at the
    // envelope whose new failure_count equals the threshold. Counts below the
    // threshold are silent, and the 4th, 5th, … failures (count > threshold)
    // stay silent too. Because each scheduled failure increments by exactly 1,
    // `== threshold` is hit on one and only one envelope.
    #[test]
    fn should_notify_fires_once_exactly_at_threshold() {
        let t = FAILURE_NOTIFY_THRESHOLD;
        // Below the threshold: silent.
        assert!(!should_notify_failure(1, t, Trigger::Schedule));
        assert!(!should_notify_failure(2, t, Trigger::Schedule));
        // The single crossing: fire.
        assert!(should_notify_failure(3, t, Trigger::Schedule));
        // Past the threshold (4th, 5th failure): silent.
        assert!(!should_notify_failure(4, t, Trigger::Schedule));
        assert!(!should_notify_failure(5, t, Trigger::Schedule));
        assert!(!should_notify_failure(100, t, Trigger::Schedule));
    }

    // VAL-ROBUST-020: a success resets failure_count to 0, so the counter climbs
    // from scratch and crosses the threshold again on a fresh chain — the
    // throttle re-arms with the reset, with no extra state. Counting up 0,1,2,3
    // a SECOND time still yields exactly one `== threshold` crossing.
    #[test]
    fn should_notify_re_arms_after_reset_to_zero() {
        let t = FAILURE_NOTIFY_THRESHOLD;
        // A reset lands the counter at 0 — never a crossing on its own.
        assert!(!should_notify_failure(0, t, Trigger::Schedule));
        // The fresh chain climbing back to the threshold fires once more.
        assert!(should_notify_failure(t, t, Trigger::Schedule));
    }

    // A MANUAL trigger never raises the banner, whatever the count: manual runs
    // do not touch failure_count, so they can never legitimately reach the
    // crossing — and the explicit trigger guard makes that unmistakable even if
    // a stale count were passed in.
    #[test]
    fn should_notify_never_fires_for_manual_trigger() {
        let t = FAILURE_NOTIFY_THRESHOLD;
        for count in 0..=10 {
            assert!(
                !should_notify_failure(count, t, Trigger::Manual),
                "manual trigger must never notify (count {count})"
            );
        }
    }

    // The notification body names the offending job and its consecutive-failure
    // count — and nothing else (no command output / secret material).
    #[test]
    fn failure_notify_body_names_job_and_count() {
        let body = failure_notify_body("nightly-backup", 3);
        assert!(body.contains("nightly-backup"), "body names the job");
        assert!(body.contains('3'), "body carries the failure count");
        assert!(
            body.contains("连续失败"),
            "body conveys the continuous-failure semantics"
        );
    }

    // VAL-ROBUST-019 (end-to-end persisted view): driving an always-failing
    // SCHEDULED job through three envelopes climbs failure_count 1 → 2 → 3, and
    // the third envelope is the single one whose persisted count equals the
    // threshold (the crossing). The 4th and 5th envelopes land on 4 and 5, past
    // the threshold. The job is re-read each envelope so the persisted counter
    // (the source of truth the executor reads back) drives the decision.
    #[tokio::test]
    async fn scheduled_failures_cross_threshold_once_in_persisted_count() {
        let env = setup().await;
        // No AppHandle in the unit wiring, so the banner is a no-op — but the
        // failure_count / history accounting (and the crossing) is unaffected,
        // which is exactly the graceful-degradation guarantee (ROBUST-021).
        assert!(
            env.executor.app_handle.is_none(),
            "unit executor has no AppHandle; notifications are a clean no-op here"
        );

        let counter = env._temp_dir.path().join("c_notify_chain");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_notify", artifact_target(&artifact_id)).await;
        job.max_retries = 0; // each envelope is a single attempt
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        // Five scheduled failures; record the persisted count after each, and
        // whether it would have armed the banner.
        let mut counts = Vec::new();
        let mut crossings = 0;
        for _ in 0..5 {
            // Re-read so the in-memory snapshot tracks the persisted chain (the
            // real scheduler reloads via `list_due` every tick).
            let current = env.executor.jobs.get("job_notify").await.unwrap().unwrap();
            let exec = env
                .executor
                .execute(&current, Trigger::Schedule)
                .await
                .expect("scheduled execute should not fail the run");
            assert_eq!(exec.status, ExecutionStatus::Failed);

            let after = env.executor.jobs.get("job_notify").await.unwrap().unwrap();
            counts.push(after.failure_count);
            if should_notify_failure(
                after.failure_count,
                FAILURE_NOTIFY_THRESHOLD,
                Trigger::Schedule,
            ) {
                crossings += 1;
            }
        }

        // The chain climbs 1,2,3,4,5 — monotonic, never reset (no success).
        assert_eq!(counts, vec![1, 2, 3, 4, 5], "continuous chain climbs by 1");
        // Exactly ONE crossing across the whole chain (at == threshold).
        assert_eq!(
            crossings, 1,
            "the banner arms exactly once per failure chain"
        );
        // History and accounting survived even though no banner was shown.
        let rows = read_rows(&env, "job_notify").await;
        assert_eq!(rows.len(), 5, "every failed envelope is still recorded");
        assert!(rows.iter().all(|r| r.status == "failed"));
    }

    // VAL-ROBUST-020 (end-to-end persisted view): a success between two failure
    // chains resets failure_count to 0, so the SECOND chain crosses the
    // threshold again — two crossings across fail×3, success, fail×3.
    #[tokio::test]
    async fn success_reset_re_arms_threshold_crossing() {
        let env = setup().await;
        let fail_counter = env._temp_dir.path().join("c_notify_rearm");
        let fail_id = install_fail_then_succeed_artifact(&env, &fail_counter, 1_000).await;
        let ok_id = install_shell_artifact(&env, "echo ok\nexit 0\n").await;

        let mut fail_job = make_job("job_rearm", artifact_target(&fail_id)).await;
        fail_job.max_retries = 0;
        fail_job.retry_delay_secs = 0;
        seed_job(&env, &fail_job).await;

        // Build the run sequence: fail, fail, fail, success, fail, fail, fail.
        let ok_target = artifact_target(&ok_id);
        let sequence = [
            (false,),
            (false,),
            (false,),
            (true,),
            (false,),
            (false,),
            (false,),
        ];

        let mut crossings = 0;
        let mut counts = Vec::new();
        for (succeed,) in sequence {
            let mut current = env.executor.jobs.get("job_rearm").await.unwrap().unwrap();
            // Swap the target per step (success vs failure) without disturbing
            // the persisted counter the executor reads back.
            if succeed {
                current.target = ok_target.clone();
            } else {
                current.target = artifact_target(&fail_id);
            }
            env.executor
                .execute(&current, Trigger::Schedule)
                .await
                .unwrap();

            let after = env.executor.jobs.get("job_rearm").await.unwrap().unwrap();
            counts.push(after.failure_count);
            if should_notify_failure(
                after.failure_count,
                FAILURE_NOTIFY_THRESHOLD,
                Trigger::Schedule,
            ) {
                crossings += 1;
            }
        }

        // Counter: 1,2,3 (first chain), 0 (success reset), 1,2,3 (second chain).
        assert_eq!(counts, vec![1, 2, 3, 0, 1, 2, 3]);
        // Two crossings: once per chain. The success between them re-armed the
        // throttle without any extra state.
        assert_eq!(crossings, 2, "the throttle re-arms after a success reset");
    }

    // VAL-ROBUST-021: the banner path is graceful-degradation by construction —
    // `notify_failure_threshold` is a clean no-op without an AppHandle and never
    // panics or blocks, so a scheduled failure chain crossing the threshold
    // still finalizes the row and bumps failure_count exactly as it would with a
    // working desktop. (A REAL macOS permission-denied banner is probed by the
    // milestone validator with computer-use; here we prove the executor never
    // depends on the banner reaching the screen.)
    #[tokio::test]
    async fn notification_degradation_never_blocks_execution() {
        let env = setup().await;
        assert!(
            env.executor.app_handle.is_none(),
            "no AppHandle => the banner is a no-op (the degraded path)"
        );

        let counter = env._temp_dir.path().join("c_notify_degrade");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_degrade", artifact_target(&artifact_id)).await;
        job.max_retries = 0;
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        // Drive failures right up to and past the threshold; calling
        // `notify_failure_threshold` (a no-op here) must not panic or stall.
        for _ in 0..FAILURE_NOTIFY_THRESHOLD + 1 {
            let current = env.executor.jobs.get("job_degrade").await.unwrap().unwrap();
            let exec = env
                .executor
                .execute(&current, Trigger::Schedule)
                .await
                .expect("execute completes even though no banner can be shown");
            assert_eq!(exec.status, ExecutionStatus::Failed);
        }

        // The threshold-crossing did NOT depend on a banner: the row and the
        // continuous-failure count are persisted exactly as expected.
        let after = env.executor.jobs.get("job_degrade").await.unwrap().unwrap();
        assert_eq!(
            after.failure_count,
            FAILURE_NOTIFY_THRESHOLD + 1,
            "failure_count accrues normally despite no banner"
        );
        let rows = read_rows(&env, "job_degrade").await;
        assert_eq!(
            rows.len(),
            (FAILURE_NOTIFY_THRESHOLD + 1) as usize,
            "every failure is recorded despite the silent banner"
        );
        assert!(rows.iter().all(|r| r.status == "failed"));

        // Calling the notifier directly with no handle is a safe no-op too.
        env.executor
            .notify_failure_threshold("job_degrade", FAILURE_NOTIFY_THRESHOLD);
    }

    // A MANUAL run never crosses the threshold: it leaves failure_count
    // untouched, so even a job already AT the threshold via prior scheduled
    // failures does not re-arm or re-fire on a manual failure (sticks at 3, and
    // `should_notify_failure` is gated to Schedule anyway).
    #[tokio::test]
    async fn manual_failure_does_not_arm_notification() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_notify_manual");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_manual_notify", artifact_target(&artifact_id)).await;
        job.max_retries = 0;
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        // Seed the counter exactly AT the threshold via three scheduled failures.
        for _ in 0..FAILURE_NOTIFY_THRESHOLD {
            let current = env
                .executor
                .jobs
                .get("job_manual_notify")
                .await
                .unwrap()
                .unwrap();
            env.executor
                .execute(&current, Trigger::Schedule)
                .await
                .unwrap();
        }
        let at_threshold = env
            .executor
            .jobs
            .get("job_manual_notify")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(at_threshold.failure_count, FAILURE_NOTIFY_THRESHOLD);

        // A manual failure leaves the count untouched and never notifies.
        let manual = env.executor.run_now(&at_threshold).await.unwrap();
        assert_eq!(manual.status, ExecutionStatus::Failed);
        let after = env
            .executor
            .jobs
            .get("job_manual_notify")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            after.failure_count, FAILURE_NOTIFY_THRESHOLD,
            "manual failure does not touch the continuous counter"
        );
        assert!(
            !should_notify_failure(
                after.failure_count,
                FAILURE_NOTIFY_THRESHOLD,
                Trigger::Manual
            ),
            "a manual trigger never arms the banner"
        );
    }

    // VAL-ROBUST-009 + VAL-HISTORY-032: a job that fails every attempt is retried
    // up to max_retries+1 times, the dispatch is invoked exactly that many times,
    // and the WHOLE envelope is ONE row finalized to `failed` with the final
    // attempt recorded — never one row per attempt.
    #[tokio::test]
    async fn always_failing_job_retries_to_max_then_one_failed_row() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_always_fail");
        // fail_count huge => always fails.
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_retry_fail", artifact_target(&artifact_id)).await;
        job.max_retries = 3; // up to 4 attempts
        job.retry_delay_secs = 0; // no inter-attempt wait, keep the test fast
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.attempt, 4, "max_retries=3 => terminal attempt 4");
        // The dispatch actually ran 4 times.
        assert_eq!(read_counter(&counter), 4, "exactly 4 dispatches");

        // ONE row for the whole envelope (HISTORY-032), recording attempt 4.
        let rows = read_rows(&env, "job_retry_fail").await;
        assert_eq!(rows.len(), 1, "attempt=4 envelope is still ONE history row");
        assert_eq!(rows[0].status, "failed");
        assert_eq!(rows[0].attempt, 4);

        // run_count +1 (one trigger), failure_count +1 (terminal scheduled fail).
        let after = env
            .executor
            .jobs
            .get("job_retry_fail")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.run_count, 1, "one envelope => run_count +1");
        assert_eq!(after.failure_count, 1, "terminal failure increments");
    }

    // VAL-ROBUST-010: max_retries=0 makes the first failure terminal — exactly
    // one dispatch, one attempt=1 failed row, no retry.
    #[tokio::test]
    async fn zero_retries_fails_on_first_attempt() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_zero_retry");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_no_retry", artifact_target(&artifact_id)).await;
        job.max_retries = 0;
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.attempt, 1, "no retries => terminal attempt 1");
        assert_eq!(read_counter(&counter), 1, "exactly one dispatch, no retry");

        let rows = read_rows(&env, "job_no_retry").await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "failed");
        assert_eq!(rows[0].attempt, 1);
    }

    // VAL-ROBUST-016: a job that fails then succeeds on a retry finalizes to
    // `success`, resets failure_count to 0, but keeps the attempt trail
    // (attempt > 1) so the earlier failures remain visible.
    #[tokio::test]
    async fn fail_then_succeed_records_success_with_attempt_trail() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_fail_then_ok");
        // Fail the first 2 attempts, succeed on the 3rd.
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 2).await;
        let mut job = make_job("job_recover", artifact_target(&artifact_id)).await;
        job.max_retries = 3; // up to 4 attempts; success arrives on attempt 3
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;
        // Pre-load a non-zero failure_count to prove success RESETS it.
        env.executor
            .jobs
            .update_after_run(
                "job_recover",
                current_timestamp(),
                ExecutionStatus::Failed,
                FailureCountUpdate::Increment,
                job.next_run_at,
                current_timestamp(),
            )
            .await
            .unwrap();
        let before = env.executor.jobs.get("job_recover").await.unwrap().unwrap();
        assert_eq!(before.failure_count, 1, "seeded a prior failure");

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();

        assert_eq!(exec.status, ExecutionStatus::Success);
        assert_eq!(exec.attempt, 3, "succeeded on the 3rd attempt");
        assert_eq!(
            read_counter(&counter),
            3,
            "stopped dispatching once it succeeded"
        );

        let rows = read_rows(&env, "job_recover").await;
        assert_eq!(rows.len(), 1, "one envelope => one row even after retries");
        assert_eq!(rows[0].status, "success");
        assert_eq!(
            rows[0].attempt, 3,
            "the failure trail (attempt>1) is preserved"
        );

        let after = env.executor.jobs.get("job_recover").await.unwrap().unwrap();
        assert_eq!(
            after.failure_count, 0,
            "a terminal success resets the chain"
        );
    }

    // VAL-ROBUST-017: all retries fail => terminal `failed` and failure_count +1.
    #[tokio::test]
    async fn all_retries_fail_increments_failure_count() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_all_fail");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_all_fail", artifact_target(&artifact_id)).await;
        job.max_retries = 2;
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.attempt, 3);

        let after = env
            .executor
            .jobs
            .get("job_all_fail")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.run_count, 1);
        assert_eq!(after.failure_count, 1);
    }

    // VAL-ROBUST-018: a "fail -> fail -> success -> fail" SEQUENCE of four
    // envelopes leaves run_count=4 (cumulative triggers) and failure_count=1
    // (continuous failures, reset by the intervening success), and the count of
    // persisted FAILED history rows does not shrink when the success resets the
    // continuous counter — continuous vs cumulative semantics are distinct.
    #[tokio::test]
    async fn continuous_vs_cumulative_failure_semantics() {
        let env = setup().await;
        // Two targets: one that always fails, one that always succeeds, run in
        // the order fail, fail, success, fail.
        let fail_counter = env._temp_dir.path().join("c_seq_fail");
        let fail_id = install_fail_then_succeed_artifact(&env, &fail_counter, 1_000).await;
        let ok_id = install_shell_artifact(&env, "echo ok\nexit 0\n").await;

        let mut fail_job = make_job("job_seq", artifact_target(&fail_id)).await;
        fail_job.max_retries = 0; // each envelope is a single attempt, keep it fast
        fail_job.retry_delay_secs = 0;
        seed_job(&env, &fail_job).await;

        // envelope 1: fail
        env.executor
            .execute(&fail_job, Trigger::Schedule)
            .await
            .unwrap();
        // envelope 2: fail
        env.executor
            .execute(&fail_job, Trigger::Schedule)
            .await
            .unwrap();
        // envelope 3: success (failure_count resets to 0)
        let mut ok_job = fail_job.clone();
        ok_job.target = artifact_target(&ok_id);
        env.executor
            .execute(&ok_job, Trigger::Schedule)
            .await
            .unwrap();
        // envelope 4: fail
        env.executor
            .execute(&fail_job, Trigger::Schedule)
            .await
            .unwrap();

        let after = env.executor.jobs.get("job_seq").await.unwrap().unwrap();
        assert_eq!(
            after.run_count, 4,
            "four triggers => run_count 4 (cumulative)"
        );
        assert_eq!(
            after.failure_count, 1,
            "continuous failures: reset by the success, then the final fail"
        );

        // The persisted FAILED history rows are not reduced by the reset: three
        // failed envelopes => three failed rows (plus one success row).
        let rows = read_rows(&env, "job_seq").await;
        let failed = rows.iter().filter(|r| r.status == "failed").count();
        let succeeded = rows.iter().filter(|r| r.status == "success").count();
        assert_eq!(
            failed, 3,
            "failed history rows survive the failure_count reset"
        );
        assert_eq!(succeeded, 1);
        assert_eq!(rows.len(), 4, "four envelopes => four rows");
    }

    // VAL-ROBUST-023 + VAL-ROBUST-024: a manual run_now also retries on failure,
    // but a terminal manual failure leaves failure_count untouched (manual runs
    // do not participate in continuous-failure tracking).
    #[tokio::test]
    async fn manual_run_retries_but_does_not_touch_failure_count() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_manual");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_manual_retry", artifact_target(&artifact_id)).await;
        job.max_retries = 2;
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;
        // Seed a prior failure_count so we can prove the manual run leaves it.
        env.executor
            .jobs
            .update_after_run(
                "job_manual_retry",
                current_timestamp(),
                ExecutionStatus::Failed,
                FailureCountUpdate::Increment,
                job.next_run_at,
                current_timestamp(),
            )
            .await
            .unwrap();

        let exec = env.executor.run_now(&job).await.expect("manual run");

        assert_eq!(exec.trigger, Trigger::Manual);
        assert_eq!(exec.status, ExecutionStatus::Failed);
        // The manual run still RETRIED (ROBUST-023): 3 dispatches for 2 retries.
        assert_eq!(exec.attempt, 3, "manual run retries up to max_retries+1");
        assert_eq!(read_counter(&counter), 3);

        let after = env
            .executor
            .jobs
            .get("job_manual_retry")
            .await
            .unwrap()
            .unwrap();
        // failure_count is untouched by the manual failure (ROBUST-024).
        assert_eq!(
            after.failure_count, 1,
            "manual failure does not bump the chain"
        );
    }

    // VAL-ROBUST-011: a timeout-class failure also triggers retry, and an
    // envelope whose every attempt times out finalizes to a `timeout` terminal
    // state with the final attempt recorded. The bound is short (1s) and there
    // is one retry, so the test runs in ~2s.
    #[tokio::test]
    async fn timeout_failure_triggers_retry_and_ends_as_timeout() {
        let env = setup().await;
        // Sleeps far past the 1s bound on every attempt => every attempt times
        // out; kill_on_drop reaps the child so each attempt returns near 1s.
        let artifact_id = install_shell_artifact(&env, "sleep 30\nexit 0\n").await;
        let mut job = make_job("job_retry_timeout", artifact_target(&artifact_id)).await;
        job.exec_timeout_secs = 1;
        job.max_retries = 1; // 2 attempts, both time out
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        let start = std::time::Instant::now();
        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        let wall = start.elapsed();

        assert_eq!(
            exec.status,
            ExecutionStatus::Timeout,
            "terminal state is timeout"
        );
        assert_eq!(exec.attempt, 2, "the timed-out envelope retried once");
        // Two ~1s attempts, no inter-attempt wait => well under the 30s sleep.
        assert!(
            wall < Duration::from_secs(15),
            "both attempts were interrupted near the 1s bound, took {wall:?}"
        );

        let rows = read_rows(&env, "job_retry_timeout").await;
        assert_eq!(rows.len(), 1, "one envelope => one row");
        assert_eq!(rows[0].status, "timeout");
        assert_eq!(rows[0].attempt, 2);

        let after = env
            .executor
            .jobs
            .get("job_retry_timeout")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            after.failure_count, 1,
            "a terminal timeout is a continuous failure"
        );
    }

    // VAL-ROBUST-012: retry_delay_secs=0 converges in a bounded number of
    // attempts (no busy-loop / no infinite retries) and returns promptly. With
    // max_retries=5 a back-to-back failing envelope must run exactly 6 attempts
    // and finish far faster than any backoff would allow.
    #[tokio::test]
    async fn zero_delay_converges_without_busy_loop() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_zero_delay");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_zero_delay", artifact_target(&artifact_id)).await;
        job.max_retries = 5;
        job.retry_delay_secs = 0;
        seed_job(&env, &job).await;

        let start = std::time::Instant::now();
        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        let wall = start.elapsed();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.attempt, 6, "max_retries=5 => bounded at 6 attempts");
        assert_eq!(
            read_counter(&counter),
            6,
            "exactly 6 dispatches, then it stops"
        );
        // Pure convergence proof: 6 fast shell runs with zero backoff complete
        // quickly — if the loop were unbounded this would never return.
        assert!(
            wall < Duration::from_secs(20),
            "zero-delay retries converge promptly, took {wall:?}"
        );
    }

    // VAL-ROBUST-013/015 (in-flight, retry leg): the WHOLE retry envelope is held
    // under one in-flight claim, so a concurrent run_now (the scheduler tick uses
    // the same gate) fired WHILE the envelope is mid-backoff is rejected with a
    // CONFLICT and opens NO second row. We use a non-zero base so the envelope
    // dwells in a backoff sleep, fire a competing run_now during that window, and
    // assert there is never more than one row and only one running row.
    #[tokio::test]
    async fn retry_envelope_holds_in_flight_across_backoff() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_inflight");
        // Always fails; the first failure triggers a backoff sleep we can race.
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_inflight_retry", artifact_target(&artifact_id)).await;
        job.max_retries = 1; // one retry: a single backoff window to race
        job.retry_delay_secs = 1; // ~1s backoff before the retry — wide enough to race
        seed_job(&env, &job).await;

        // Drive the envelope under a held claim, exactly as run_now/scheduler do.
        let executor = env.executor.clone();
        let job_clone = job.clone();
        let handle = tokio::spawn(async move {
            let _guard = executor
                .try_claim(&job_clone.id)
                .await
                .expect("first claim succeeds");
            executor.execute(&job_clone, Trigger::Schedule).await
        });

        // Wait until the first attempt has run (counter == 1) — the envelope is
        // now in its backoff window, still holding the slot.
        let mut entered_backoff = false;
        for _ in 0..200 {
            if read_counter(&counter) >= 1 {
                entered_backoff = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(entered_backoff, "the first attempt should have dispatched");

        // A competing run_now during the backoff must be rejected (the envelope
        // still owns the in-flight slot), writing NO second row.
        let conflict = env.executor.run_now(&job).await;
        assert!(
            conflict.is_err(),
            "a run fired mid-backoff must be rejected while the envelope holds the slot"
        );
        assert_eq!(conflict.unwrap_err().code, "CONFLICT");

        // At most one running row exists during the envelope.
        let mid = read_rows(&env, "job_inflight_retry").await;
        let running = mid.iter().filter(|r| r.status == "running").count();
        assert_eq!(running, 1, "exactly one running row during the envelope");
        assert_eq!(mid.len(), 1, "no second row opened by the rejected run");

        let exec = handle.await.unwrap().expect("envelope completes");
        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.attempt, 2);

        // After completion: still exactly one (now terminal) row.
        let rows = read_rows(&env, "job_inflight_retry").await;
        assert_eq!(rows.len(), 1, "the whole envelope is a single row");
        assert_eq!(rows[0].status, "failed");
    }

    // VAL-ROBUST-025: deleting the job mid-envelope (during a backoff) aborts the
    // retry cleanly — no further attempt runs, the cascade removes the running
    // row (no leftover running row), and no new execution row appears after the
    // delete. The envelope surfaces the deletion as a not_found Err.
    #[tokio::test]
    async fn delete_during_retry_aborts_cleanly() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_delete");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_del_retry", artifact_target(&artifact_id)).await;
        job.max_retries = 3;
        job.retry_delay_secs = 1; // a backoff window during which we delete the job
        seed_job(&env, &job).await;

        let executor = env.executor.clone();
        let job_clone = job.clone();
        let handle =
            tokio::spawn(async move { executor.execute(&job_clone, Trigger::Schedule).await });

        // Wait for the first attempt to have run, then delete the job while the
        // envelope is backing off before attempt 2.
        for _ in 0..200 {
            if read_counter(&counter) >= 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        env.executor.jobs.delete("job_del_retry").await.unwrap();

        let result = handle.await.unwrap();
        assert!(
            result.is_err(),
            "a job deleted mid-envelope aborts with an error, not a finalized row"
        );
        assert_eq!(result.unwrap_err().code, "NOT_FOUND");

        // Only the first attempt ran; no further attempts after the delete.
        let dispatches = read_counter(&counter);
        assert!(
            (1..=2).contains(&dispatches),
            "no new attempts after delete (got {dispatches} dispatches)"
        );

        // The cascade removed the running row; no execution rows linger.
        let count: i64 = sqlx::query("SELECT COUNT(*) FROM job_executions WHERE job_id = $1")
            .bind("job_del_retry")
            .fetch_one(env.db.pool())
            .await
            .unwrap()
            .try_get(0)
            .unwrap();
        assert_eq!(count, 0, "the deleted job leaves no running/terminal rows");

        // The in-flight gate is free (execute does not itself claim).
        assert!(env.executor.try_claim("job_del_retry").await.is_some());
    }

    // A real-timing anchor for the exponential law (small base): with base=1s and
    // two retries, the two backoffs are ~1s then ~2s, so the failing envelope
    // takes at least ~3s of cumulative backoff (plus three fast dispatches). This
    // ties the wall-clock behaviour to `backoff_delay` without a multi-minute
    // wait; the precise per-gap ratio is pinned by the pure `backoff_delay`
    // tests above.
    #[tokio::test]
    async fn real_backoff_accumulates_exponential_gaps() {
        let env = setup().await;
        let counter = env._temp_dir.path().join("c_real_backoff");
        let artifact_id = install_fail_then_succeed_artifact(&env, &counter, 1_000).await;
        let mut job = make_job("job_real_backoff", artifact_target(&artifact_id)).await;
        job.max_retries = 2; // 3 attempts, 2 backoffs: ~1s + ~2s
        job.retry_delay_secs = 1; // base = 1s
        seed_job(&env, &job).await;

        let start = std::time::Instant::now();
        let exec = env.executor.execute(&job, Trigger::Schedule).await.unwrap();
        let wall = start.elapsed();

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.attempt, 3);
        assert_eq!(read_counter(&counter), 3);
        // Cumulative backoff is 1s + 2s = 3s; allow the lower bound a small
        // margin for timer granularity. Upper bound guards against accidental
        // extra backoff (e.g. a wrong exponent producing 1+2+4).
        assert!(
            wall >= Duration::from_millis(2_800),
            "two backoffs (~1s + ~2s) must accumulate, took {wall:?}"
        );
        assert!(
            wall < Duration::from_secs(10),
            "but only TWO backoffs, not more, took {wall:?}"
        );
    }
}
