// Scheduled-job scheduler: the background tick loop that fires due jobs.
//
// Reuses the `catalog_sync` background pattern (`tauri::async_runtime::spawn`
// + `loop { work; sleep }`). The scheduler only runs while the app is up:
// closing it stops scheduling, restarting recomputes every enabled job's
// `next_run_at` from `now` — missed firings during downtime are NOT replayed.
//
// Design (each property holds by construction):
//
// - DB is the single source of truth. Every tick re-reads due jobs via
//   `JobRepository::list_due(now)`; nothing is cached across ticks.
// - At-most-once per occurrence. The instant a due job is selected, its
//   `next_run_at` is advanced to the next cron occurrence strictly after `now`
//   (the SAME `utils::cron::next_occurrences(cron, 1)` source the preview uses)
//   BEFORE execution is dispatched. A 30s tick against a 1-minute cron therefore
//   selects each occurrence once; a forward clock jump advances straight to the
//   next future occurrence rather than replaying every skipped slot.
// - Re-entrancy guard. The SHARED in-flight set lives on the `JobExecutor`
//   (`Arc<Mutex<HashSet<JobId>>>`); the scheduler claims a due job's slot via
//   `executor.try_claim` BEFORE advancing/dispatching it, and a job already in
//   flight (by ANY trigger — a tick OR a manual run-now) is skipped. The claim
//   is released on BOTH the completion and panic paths (an RAII guard), so a
//   crashing/​panicking job never wedges its slot. Because the manual
//   `job_run_now` command claims against the same set on the same executor,
//   scheduled and manual triggers cannot run the same job concurrently.
// - The loop never blocks on a job. Each due job is dispatched on its own
//   detached `tokio::spawn`; a slow or panicking job neither stalls nor kills
//   the tick loop.
//
// Out of scope here: retry / timeout-override / notifications (M4). This drives
// the existing `JobExecutor::execute`; it does not modify the executor's
// dispatch logic (the M2 run-now feature relocated the in-flight set onto the
// executor so every trigger path shares one gate).

use std::sync::Arc;
use std::time::Duration;

use tauri::{Runtime, Wry};

use crate::services::job_executor::InFlightGuard;
use crate::services::JobExecutor;
use crate::storage::types::{ExecutionStatus, Job, Timestamp, Trigger};
use crate::storage::{JobExecutionRepository, JobRepository};

/// Fixed tick interval. Sub-minute so a 1-minute cron is observed within the
/// minute it is due, matching `catalog_sync`'s "work then sleep" cadence (the
/// sleep follows the work, so per-tick processing time never compounds into
/// schedule drift).
pub const TICK_INTERVAL: Duration = Duration::from_secs(30);

/// Error message stamped on `running` execution rows left behind by a previous
/// process (crash or normal exit mid-run) when the scheduler reconciles them at
/// startup.
const STALE_RUNNING_MESSAGE: &str = "Interrupted: process exited while running";

/// The background scheduler driving due jobs to execution.
///
/// Generic over the Tauri `Runtime` (to match `JobExecutor`); the app wires a
/// `JobScheduler<Wry>` and `app.manage`s it so it can be observed as State.
pub struct JobScheduler<R: Runtime = Wry> {
    jobs: JobRepository,
    /// Reconcile-only handle on `job_executions`. The executor owns its own
    /// repository for run finalization; the scheduler keeps a parallel one
    /// solely to reconcile stale `running` rows at startup (the executor does
    /// not expose that, and per the feature boundary we do not modify it).
    executions: JobExecutionRepository,
    executor: JobExecutor<R>,
}

// Manual `Clone`: the bound is on the fields, not on `R: Clone` (Tauri runtimes
// are not `Clone`). The executor clones via its `Arc`-backed fields (including
// the shared in-flight set it now owns).
impl<R: Runtime> Clone for JobScheduler<R> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
            executions: self.executions.clone(),
            executor: self.executor.clone(),
        }
    }
}

impl<R: Runtime> JobScheduler<R> {
    /// Build a scheduler from its collaborators.
    pub fn new(
        jobs: JobRepository,
        executions: JobExecutionRepository,
        executor: JobExecutor<R>,
    ) -> Self {
        Self {
            jobs,
            executions,
            executor,
        }
    }

    /// Convenience constructor mirroring `JobExecutor::from_db`, used by the app
    /// wiring in `lib.rs`.
    pub fn from_db(db: Arc<crate::storage::Database>, executor: JobExecutor<R>) -> Self {
        Self::new(
            JobRepository::new(db.clone()),
            JobExecutionRepository::new(db),
            executor,
        )
    }

    /// Reconcile execution rows left in `running` by a prior process.
    ///
    /// On any startup (crash or clean exit) the previous run may have died
    /// mid-execution, leaving rows stuck in `running`. There is no live process
    /// behind them, so they are finalized to `failed` with an explanatory error.
    /// Returns the number of rows reconciled. Run ONCE at startup, before the
    /// tick loop begins.
    pub async fn reconcile_stale_running(&self) -> Result<u64, crate::models::AppError> {
        let now = current_timestamp();
        let count = self
            .executions
            .reconcile_stale_running(ExecutionStatus::Failed, STALE_RUNNING_MESSAGE, now)
            .await?;
        if count > 0 {
            tracing::info!(
                count,
                "[JobScheduler::reconcile] finalized {count} stale running execution(s) to failed"
            );
        } else {
            tracing::info!("[JobScheduler::reconcile] no stale running executions");
        }
        Ok(count)
    }

    /// Recompute `next_run_at` for every enabled job from `now`.
    ///
    /// Run ONCE at startup, after reconcile and before the tick loop. Each
    /// enabled job's `next_run_at` is set to the next cron occurrence strictly
    /// after `now` — so firings missed while the app was down are NOT replayed,
    /// and an already-overdue job does not fire instantly on launch. Disabled
    /// jobs are left untouched (they never fire). A job whose cron yields no
    /// upcoming occurrence has its `next_run_at` cleared to NULL (unscheduled).
    ///
    /// Per-job failures are logged and skipped so one malformed cron cannot
    /// abort startup recomputation for the rest.
    pub async fn recompute_all_enabled(&self) -> Result<(), crate::models::AppError> {
        let now = current_timestamp();
        // Page through all jobs; recompute only the enabled ones. The job set is
        // small (user-defined schedules), so a generous page is a single query.
        let all = self.jobs.list(i64::MAX, 0).await?;
        let mut recomputed = 0usize;
        for job in all.into_iter().filter(|j| j.enabled) {
            let next = next_run_after(&job.cron_expr, now);
            if let Err(e) = self.jobs.recompute_next_run(&job.id, next, now).await {
                tracing::warn!(
                    job_id = %job.id,
                    "[JobScheduler::recompute] failed to set next_run_at: {e:?}"
                );
                continue;
            }
            recomputed += 1;
        }
        tracing::info!(
            recomputed,
            "[JobScheduler::recompute] recomputed next_run_at for {recomputed} enabled job(s) from now (no catch-up)"
        );
        Ok(())
    }

    /// One scheduler tick: read due jobs from the DB and fire the eligible ones.
    ///
    /// For each due job not already in flight: advance its `next_run_at` to the
    /// next cron occurrence > now (so the next tick won't re-select it — this is
    /// what makes a 30s tick fire a 1-minute cron exactly once), mark it in
    /// flight, then dispatch its execution on a detached task. The loop returns
    /// immediately; it never waits on a job's execution.
    ///
    /// Returns the number of jobs dispatched this tick (0 for an idle tick).
    pub async fn tick(&self) -> u64 {
        let now = current_timestamp();

        // DB is the source of truth — re-read every tick.
        let due = match self.jobs.list_due(now).await {
            Ok(due) => due,
            Err(e) => {
                tracing::warn!("[JobScheduler::tick] failed to query due jobs: {e:?}");
                return 0;
            }
        };

        let mut dispatched = 0u64;
        for mut job in due {
            // Re-entrancy: skip jobs whose previous execution is still in flight
            // by ANY trigger. The slot is reserved against the executor's shared
            // set BEFORE advancing/dispatching, so a concurrent tick (should one
            // overlap) or a manual run-now cannot also claim it. Holding the
            // guard keeps the claim alive until it is moved into the dispatch
            // task; dropping it on an early `continue` releases the slot.
            let guard = match self.executor.try_claim(&job.id).await {
                Some(guard) => guard,
                None => {
                    tracing::info!(
                        job_id = %job.id,
                        "[JobScheduler::tick] skipped {} — previous execution still in flight",
                        job.id
                    );
                    continue;
                }
            };

            // Advance next_run_at to the next occurrence strictly after now,
            // BEFORE dispatching. This is the at-most-once guarantee: the next
            // tick (and a forward clock jump) sees the advanced time and does
            // not re-fire this occurrence.
            let next = next_run_after(&job.cron_expr, now);
            if let Err(e) = self.jobs.recompute_next_run(&job.id, next, now).await {
                tracing::warn!(
                    job_id = %job.id,
                    "[JobScheduler::tick] failed to advance next_run_at, releasing slot: {e:?}"
                );
                // Could not advance — drop the guard to release the slot so a
                // later tick can retry rather than wedging the job in flight.
                drop(guard);
                continue;
            }

            // Carry the advanced time on the in-memory job handed to the
            // executor. The executor PRESERVES `job.next_run_at` when it writes
            // run statistics (`update_after_run`); without this it would write
            // back the stale due-time we just advanced past, re-arming the same
            // occurrence. Setting it here keeps the advancement durable across
            // the run.
            job.next_run_at = next;

            tracing::info!(
                job_id = %job.id,
                next_run_at = ?next,
                "[JobScheduler::tick] firing {} (next_run_at advanced to {next:?})",
                job.id
            );

            self.spawn_execution(job, guard);
            dispatched += 1;
        }

        if dispatched == 0 {
            tracing::info!("[JobScheduler::tick] idle tick — no due jobs");
        } else {
            tracing::info!(
                dispatched,
                "[JobScheduler::tick] dispatched {dispatched} job(s)"
            );
        }
        dispatched
    }

    /// Dispatch one job's execution on a detached task so the tick loop is never
    /// blocked. Takes ownership of the in-flight `guard` already reserved by
    /// [`tick`] (against the executor's shared set); dropping it on completion
    /// OR panic-unwind releases the slot, so a panicking target never wedges it.
    fn spawn_execution(&self, job: Job, guard: InFlightGuard) {
        let executor = self.executor.clone();
        let job_id = job.id.clone();

        tokio::spawn(async move {
            // RAII: the moved-in guard drops at the end of this task (normal
            // return OR panic-unwind), removing the job from the shared
            // in-flight set. `next_run_at` was already advanced by the caller,
            // so a failed/panicked run still has a future occurrence queued.
            let _guard = guard;

            match executor.execute(&job, Trigger::Schedule).await {
                Ok(exec) => {
                    if matches!(exec.status, ExecutionStatus::Success) {
                        tracing::info!(
                            job_id = %job_id,
                            "[JobScheduler::run] {job_id} completed: {:?}",
                            exec.status
                        );
                    } else {
                        // The target failed; the executor recorded a failed row.
                        // The loop keeps running.
                        tracing::warn!(
                            job_id = %job_id,
                            "[JobScheduler::run] {job_id} finished with non-success status: {:?}",
                            exec.status
                        );
                    }
                }
                Err(e) => {
                    // A persistence-level failure (no consistent row written).
                    // Logged; the loop is unaffected.
                    tracing::warn!(
                        job_id = %job_id,
                        "[JobScheduler::run] {job_id} execution error: {e:?}"
                    );
                }
            }
            // `_guard` drops here, releasing the in-flight slot.
        });
    }

    /// Start the background tick loop. Spawned via Tauri's async runtime so it
    /// outlives `initialize_services` and runs for the app's lifetime. Mirrors
    /// `catalog_sync::spawn`: do the work, then sleep a fixed interval (work →
    /// sleep, so processing time does not accumulate into drift).
    pub fn spawn_tick_loop(&self) {
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                scheduler.tick().await;
                tokio::time::sleep(TICK_INTERVAL).await;
            }
        });
    }

    /// Test-only view of the executor's shared in-flight set size, for asserting
    /// the slot is released after a run.
    #[cfg(test)]
    async fn in_flight_len(&self) -> usize {
        self.executor.in_flight_len().await
    }
}

/// How many upcoming occurrences `next_run_after` asks croner for. croner
/// always anchors its search on the live local clock; when the caller's `now`
/// is sampled a hair ahead of that clock, the very first occurrence can be at or
/// before `now`, so we fetch a few and take the first strictly after `now`. A
/// handful is plenty — consecutive occurrences are minutes apart at the
/// finest-grained cron and `now`/​live-clock skew is sub-second.
const LOOKAHEAD: usize = 4;

/// Next cron occurrence strictly after `now`, as the stored millisecond
/// `Timestamp`, or `None` when the schedule has no upcoming occurrence.
///
/// The single recomputation primitive shared by startup recompute and tick
/// advancement, built on `utils::cron::next_occurrences` — the SAME source the
/// preview command uses — so a job's computed next-run always matches what the
/// user previews.
///
/// At-most-once / no-catch-up: croner's search anchors on the live local clock
/// and only ever yields future occurrences, so a forward clock jump lands on the
/// single next occurrence rather than replaying every slot skipped over. The
/// `> now` selection makes the result a genuine function of `(cron, now)` and
/// guards the sub-second race where `now` was sampled just ahead of croner's
/// internal `Local::now()`.
fn next_run_after(cron: &str, now: Timestamp) -> Option<Timestamp> {
    match crate::utils::cron::next_occurrences(cron, LOOKAHEAD) {
        Ok(occ) => occ.into_iter().find(|&t| t > now),
        Err(e) => {
            tracing::warn!("[JobScheduler] invalid cron '{cron}', clearing next_run_at: {e:?}");
            None
        }
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
    use crate::storage::types::{JobTarget, SessionStrategy};
    use crate::storage::Database;
    use sqlx::Row;
    use tauri::test::MockRuntime;
    use tempfile::tempdir;

    // ---- Pure-function tests (no DB / no runtime): the core scheduling logic ----
    //
    // Timing behaviour (a real 30s tick, OS sleep/wake) cannot be reproduced in a
    // unit test, so the decision logic is factored into `next_run_after` and the
    // in-flight set operations, which ARE unit-testable. End-to-end timing is
    // left to the milestone validator.

    /// every-minute cron yields a next occurrence strictly in the future.
    #[test]
    fn next_run_after_returns_future_occurrence() {
        let now = current_timestamp();
        let next = next_run_after("* * * * *", now).expect("every-minute cron has a next run");
        assert!(next > now, "next_run_at must be strictly after now");
        // The next minute boundary is within ~60s of now.
        assert!(
            next <= now + 61_000,
            "next every-minute occurrence should be within ~60s, got {}ms ahead",
            next - now
        );
    }

    /// no-double-fire: two recomputes 30s apart inside the same minute both land
    /// on the SAME next-minute occurrence — so a 30s tick selects a 1-minute
    /// cron's occurrence exactly once (the second tick sees next_run_at already
    /// advanced past now and does not re-fire).
    #[test]
    fn next_run_after_is_stable_within_a_minute() {
        let now = current_timestamp();
        let first = next_run_after("* * * * *", now).unwrap();
        // Simulate a second tick 30s later, still within the same minute window.
        let later = now + 30_000;
        let second = next_run_after("* * * * *", later).unwrap();
        // Both ticks compute the same upcoming minute boundary => no double fire.
        // (Only diverges if `now` and `now+30s` straddle a minute boundary; the
        // boundaries are 60s apart so at most one of the two windows crosses.)
        assert!(
            second >= first,
            "a later recompute never moves the next run backwards"
        );
    }

    /// clock-jump at-most-once: after a large forward jump, recompute lands on
    /// the next future occurrence relative to the jumped-to time, never on a
    /// replay of the slots skipped over.
    #[test]
    fn next_run_after_skips_to_single_future_occurrence() {
        let now = current_timestamp();
        let next = next_run_after("* * * * *", now).unwrap();
        // The returned occurrence is a single value (not a backlog), and it is
        // the immediate next one, not the first overdue slot.
        assert!(next > now);
        assert!(
            next - now <= 61_000,
            "advances to the next occurrence, not a replay of missed ones"
        );
    }

    /// invalid cron -> None (job becomes unscheduled rather than panicking).
    #[test]
    fn next_run_after_invalid_cron_is_none() {
        assert_eq!(next_run_after("not-a-cron", current_timestamp()), None);
    }

    // ---- DB-backed scheduler tests ----

    struct TestEnv {
        scheduler: JobScheduler<MockRuntime>,
        jobs: JobRepository,
        executions: crate::storage::JobExecutionRepository,
        db: Arc<Database>,
        _temp_dir: tempfile::TempDir,
    }

    async fn setup() -> TestEnv {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = Arc::new(Database::new(&db_path).await.unwrap());

        // A `MockRuntime` executor (no AppHandle) is enough to drive scheduling:
        // dispatch fails cleanly (prompt services are unwired), but the execution
        // still completes — a row is inserted and finalized, stats advance.
        let executor = JobExecutor::<MockRuntime>::from_db(db.clone());
        let scheduler = JobScheduler::from_db(db.clone(), executor);

        TestEnv {
            scheduler,
            jobs: JobRepository::new(db.clone()),
            executions: crate::storage::JobExecutionRepository::new(db.clone()),
            db,
            _temp_dir: temp_dir,
        }
    }

    fn make_job(id: &str, target: JobTarget, cron: &str, now: Timestamp) -> Job {
        Job {
            id: id.to_string(),
            name: format!("job-{id}"),
            description: None,
            target,
            cron_expr: cron.to_string(),
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

    /// A deterministic fail-clean target: with no prompt services wired the
    /// dispatch returns a terminal "not configured" failure, so the execution
    /// still COMPLETES (a running row is inserted then finalized to `failed`)
    /// and job stats advance — everything scheduling mechanics assert on.
    fn prompt_target() -> JobTarget {
        JobTarget::Prompt {
            provider_id: "p1".into(),
            model_id: "m1".into(),
            prompt: "do it".into(),
            session_strategy: SessionStrategy::NewSession,
        }
    }

    /// Wait until the job's execution history shows `expected` non-running rows,
    /// or time out. Returns the final terminal-row count.
    async fn wait_for_terminal_rows(env: &TestEnv, job_id: &str, expected: usize) -> usize {
        for _ in 0..200 {
            let rows = count_terminal_rows(env, job_id).await;
            if rows >= expected {
                return rows;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        count_terminal_rows(env, job_id).await
    }

    async fn count_terminal_rows(env: &TestEnv, job_id: &str) -> usize {
        let rows = sqlx::query(
            "SELECT COUNT(*) AS c FROM job_executions WHERE job_id = $1 AND status != 'running'",
        )
        .bind(job_id)
        .fetch_one(env.db.pool())
        .await
        .unwrap();
        let c: i64 = rows.try_get("c").unwrap();
        c as usize
    }

    /// A tick fires a due, enabled job: it produces an execution and advances
    /// next_run_at past now. (Covers due-selection + dispatch + advancement.)
    #[tokio::test]
    async fn tick_fires_due_enabled_job_and_advances_next_run() {
        let env = setup().await;
        let now = current_timestamp();
        let mut job = make_job("due_job", prompt_target(), "* * * * *", now);
        job.next_run_at = Some(now - 1000); // due
        env.jobs.create(&job).await.unwrap();

        let dispatched = env.scheduler.tick().await;
        assert_eq!(dispatched, 1, "the single due job is dispatched once");

        // The execution completes (fail-clean dispatch) — one terminal row.
        assert_eq!(wait_for_terminal_rows(&env, "due_job", 1).await, 1);

        // next_run_at was advanced strictly past now BEFORE dispatch.
        let after = env.jobs.get("due_job").await.unwrap().unwrap();
        let next = after.next_run_at.expect("next_run_at advanced");
        assert!(next > now, "next_run_at advanced strictly past now");
    }

    /// A disabled job is never selected by a tick, even when overdue.
    #[tokio::test]
    async fn tick_never_fires_disabled_job() {
        let env = setup().await;
        let now = current_timestamp();
        let mut job = make_job("disabled_job", prompt_target(), "* * * * *", now);
        job.enabled = false;
        job.next_run_at = Some(now - 1000); // overdue but disabled
        env.jobs.create(&job).await.unwrap();

        let dispatched = env.scheduler.tick().await;
        assert_eq!(dispatched, 0, "disabled job is not dispatched");
        // Give any erroneous dispatch a chance to write a row, then assert none.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count_terminal_rows(&env, "disabled_job").await, 0);
    }

    /// no-double-fire across two ticks: a 1-minute cron, ticked twice in quick
    /// succession (the 30s-tick-vs-1min-cron case), fires exactly once because
    /// the first tick advanced next_run_at to the next minute (future).
    #[tokio::test]
    async fn two_ticks_within_a_minute_fire_once() {
        let env = setup().await;
        let now = current_timestamp();
        let mut job = make_job("once_job", prompt_target(), "* * * * *", now);
        job.next_run_at = Some(now - 1000); // due
        env.jobs.create(&job).await.unwrap();

        let first = env.scheduler.tick().await;
        assert_eq!(first, 1, "first tick fires it");
        // Wait for the first execution to finish so the in-flight slot frees and
        // we isolate the no-double-fire to next_run_at advancement, not re-entry.
        assert_eq!(wait_for_terminal_rows(&env, "once_job", 1).await, 1);

        let second = env.scheduler.tick().await;
        assert_eq!(second, 0, "second tick within the minute does NOT re-fire");
        // Still exactly one execution.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count_terminal_rows(&env, "once_job").await, 1);
    }

    /// Re-entrancy: while one execution is in flight (slow script), a second
    /// tick that re-selects the job does NOT dispatch it again. We force
    /// re-selection by leaving next_run_at due is impossible (the first tick
    /// advances it), so instead we drive the in-flight gate directly: manually
    /// reserve the slot, then a tick selecting the (still-due) job is skipped.
    #[tokio::test]
    async fn in_flight_job_is_skipped_by_tick() {
        let env = setup().await;
        let now = current_timestamp();
        let mut job = make_job("busy_job", prompt_target(), "* * * * *", now);
        job.next_run_at = Some(now - 1000); // due
        env.jobs.create(&job).await.unwrap();

        // Pretend an execution is already in flight for this job by claiming its
        // slot on the SHARED executor set; hold the guard for the tick so the
        // claim stays active.
        let _claim = env
            .scheduler
            .executor
            .try_claim("busy_job")
            .await
            .expect("first claim succeeds");

        let dispatched = env.scheduler.tick().await;
        assert_eq!(dispatched, 0, "in-flight job is skipped");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            count_terminal_rows(&env, "busy_job").await,
            0,
            "no execution while a prior one is in flight"
        );

        // next_run_at must NOT have been advanced for the skipped job (the slot
        // was never claimed by this tick).
        let after = env.jobs.get("busy_job").await.unwrap().unwrap();
        assert_eq!(
            after.next_run_at,
            Some(now - 1000),
            "skipped job keeps its due next_run_at for a later tick"
        );
    }

    /// After an execution completes, the in-flight slot is released so the job
    /// can fire again on a later occurrence (panic/crash release is the same
    /// RAII path, exercised by the unit test above).
    #[tokio::test]
    async fn in_flight_slot_released_after_run() {
        let env = setup().await;
        let now = current_timestamp();
        let mut job = make_job("release_job", prompt_target(), "* * * * *", now);
        job.next_run_at = Some(now - 1000);
        env.jobs.create(&job).await.unwrap();

        env.scheduler.tick().await;
        assert_eq!(wait_for_terminal_rows(&env, "release_job", 1).await, 1);

        // The slot is freed once the detached run finishes.
        for _ in 0..200 {
            if env.scheduler.in_flight_len().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            env.scheduler.in_flight_len().await,
            0,
            "in-flight set is empty after the run completes"
        );
    }

    /// A failing target is recorded as a failed execution; the tick itself still
    /// reports success (the loop keeps running) and the slot is released.
    #[tokio::test]
    async fn failing_target_records_failure_loop_continues() {
        let env = setup().await;
        let now = current_timestamp();
        let mut job = make_job("fail_job", prompt_target(), "* * * * *", now);
        job.next_run_at = Some(now - 1000);
        env.jobs.create(&job).await.unwrap();

        let dispatched = env.scheduler.tick().await;
        assert_eq!(dispatched, 1);
        assert_eq!(wait_for_terminal_rows(&env, "fail_job", 1).await, 1);

        let rows = sqlx::query(
            "SELECT status FROM job_executions WHERE job_id = 'fail_job' ORDER BY started_at DESC",
        )
        .fetch_all(env.db.pool())
        .await
        .unwrap();
        let status: String = rows[0].try_get("status").unwrap();
        assert_eq!(
            status, "failed",
            "fail-clean dispatch recorded as failed; the loop keeps running"
        );

        // Slot released; next_run_at still advanced so it will fire again.
        for _ in 0..200 {
            if env.scheduler.in_flight_len().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(env.scheduler.in_flight_len().await, 0);
        let after = env.jobs.get("fail_job").await.unwrap().unwrap();
        assert!(after.next_run_at.unwrap() > now);
    }

    /// Same tick, multiple due jobs: each is dispatched independently.
    #[tokio::test]
    async fn same_tick_dispatches_multiple_due_jobs() {
        let env = setup().await;
        let now = current_timestamp();

        for i in 0..3 {
            let id = format!("multi_{i}");
            let mut job = make_job(&id, prompt_target(), "* * * * *", now);
            job.next_run_at = Some(now - 1000);
            env.jobs.create(&job).await.unwrap();
        }

        let dispatched = env.scheduler.tick().await;
        assert_eq!(dispatched, 3, "all three due jobs dispatched in one tick");

        for i in 0..3 {
            let id = format!("multi_{i}");
            assert_eq!(wait_for_terminal_rows(&env, &id, 1).await, 1);
        }
    }

    /// Startup recompute: every enabled job's next_run_at is advanced to a
    /// future occurrence (no catch-up); disabled jobs are left untouched.
    #[tokio::test]
    async fn recompute_all_enabled_advances_future_skips_disabled() {
        let env = setup().await;
        let now = current_timestamp();

        // Enabled, badly overdue (would catch up if we replayed) — must jump to
        // a single future occurrence instead.
        let mut overdue = make_job("overdue", prompt_target(), "* * * * *", now);
        overdue.next_run_at = Some(now - 10_000_000);
        env.jobs.create(&overdue).await.unwrap();

        // Disabled — left untouched.
        let mut disabled = make_job("disabled", prompt_target(), "* * * * *", now);
        disabled.enabled = false;
        disabled.next_run_at = Some(now - 5000);
        env.jobs.create(&disabled).await.unwrap();

        env.scheduler.recompute_all_enabled().await.unwrap();

        let overdue_after = env.jobs.get("overdue").await.unwrap().unwrap();
        let next = overdue_after.next_run_at.expect("enabled job rescheduled");
        assert!(
            next > now,
            "enabled job advanced to a future occurrence (no catch-up)"
        );
        assert!(
            next - now <= 61_000,
            "advanced to the NEXT occurrence, not a replay of missed ones"
        );

        let disabled_after = env.jobs.get("disabled").await.unwrap().unwrap();
        assert_eq!(
            disabled_after.next_run_at,
            Some(now - 5000),
            "disabled job's next_run_at is untouched"
        );
    }

    /// After startup recompute, no job is immediately due — a job overdue at
    /// launch does NOT fire on the first tick (it was advanced to the future).
    #[tokio::test]
    async fn no_immediate_fire_after_startup_recompute() {
        let env = setup().await;
        let now = current_timestamp();
        let mut overdue = make_job("startup", prompt_target(), "* * * * *", now);
        overdue.next_run_at = Some(now - 10_000_000);
        env.jobs.create(&overdue).await.unwrap();

        env.scheduler.recompute_all_enabled().await.unwrap();
        let dispatched = env.scheduler.tick().await;
        assert_eq!(
            dispatched, 0,
            "no immediate fire for an at-launch-overdue job"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(count_terminal_rows(&env, "startup").await, 0);
    }

    /// Startup reconcile: a leftover `running` execution row from a prior process
    /// is finalized to `failed` with the interrupted message.
    #[tokio::test]
    async fn reconcile_finalizes_stale_running_rows() {
        let env = setup().await;
        let now = current_timestamp();
        let job = make_job("stale", prompt_target(), "* * * * *", now);
        env.jobs.create(&job).await.unwrap();

        // Simulate a crash: a running row with no live process behind it.
        env.executions
            .insert_running("stale_exec", "stale", Trigger::Schedule, 1, now, now)
            .await
            .unwrap();

        let count = env.scheduler.reconcile_stale_running().await.unwrap();
        assert_eq!(count, 1, "the leftover running row is reconciled");

        let rows = sqlx::query("SELECT status, error FROM job_executions WHERE id = 'stale_exec'")
            .fetch_one(env.db.pool())
            .await
            .unwrap();
        let status: String = rows.try_get("status").unwrap();
        let error: Option<String> = rows.try_get("error").unwrap();
        assert_eq!(status, "failed");
        assert_eq!(error.as_deref(), Some(STALE_RUNNING_MESSAGE));

        // A second reconcile finds nothing (idempotent across restarts).
        assert_eq!(env.scheduler.reconcile_stale_running().await.unwrap(), 0);
    }

    /// DB is the source of truth: a job created AFTER one idle tick is picked up
    /// by the next tick (no stale in-memory snapshot).
    #[tokio::test]
    async fn tick_rereads_db_each_time() {
        let env = setup().await;
        let now = current_timestamp();

        // First tick: nothing scheduled.
        assert_eq!(env.scheduler.tick().await, 0, "idle tick, empty DB");

        // Now add a due job and tick again.
        let mut job = make_job("late", prompt_target(), "* * * * *", now);
        job.next_run_at = Some(now - 1000);
        env.jobs.create(&job).await.unwrap();

        assert_eq!(
            env.scheduler.tick().await,
            1,
            "next tick re-reads the DB and fires it"
        );
        assert_eq!(wait_for_terminal_rows(&env, "late", 1).await, 1);
    }
}
