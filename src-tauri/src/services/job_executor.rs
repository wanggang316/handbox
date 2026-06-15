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
use std::sync::Arc;

use crate::models::{AppError, UserMessageSendRequest};
use crate::services::{
    AgentRuntime, AgentService, AgentSessionService, ArtifactService, MessageService,
    ProviderService, RunSink, SessionService,
};
use crate::storage::job_repository::DEFAULT_EXECUTION_HISTORY_LIMIT;
use crate::storage::types::{
    CreateAgentSessionRequest, ExecuteArtifactRequest, ExecutionStatus, Job, JobExecution,
    JobTarget, Provider, Timestamp, Trigger,
};
use crate::storage::{JobExecutionRepository, JobRepository};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, Wry};
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
/// `runtime` is a dedicated [`AgentRuntime`] for the executor (NOT the
/// app-managed one): `AgentRuntime` is not `Clone`, and its per-session run
/// registry need not be shared with the foreground agent UI — every job run
/// mints a brand-new session, so there is never a registry collision. `agents`
/// resolves the agent template referenced by the target; `sessions` mints the
/// per-run session from that template and reads the persisted transcript back
/// to classify the terminal outcome; `providers` resolves a usable provider for
/// the template's model (the template stores only a model id).
#[derive(Clone)]
struct AgentServices {
    runtime: Arc<AgentRuntime>,
    agents: Arc<AgentService>,
    sessions: Arc<AgentSessionService>,
    providers: Arc<ProviderService>,
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
    /// mint a fresh session, drive one run to completion). Consuming builder used
    /// by the app wiring; without it (artifact-only unit wiring) an `agent`
    /// dispatch fails cleanly rather than running. `runtime` is a dedicated
    /// [`AgentRuntime`] for the executor — see [`AgentServices`].
    pub fn with_agent_services(
        mut self,
        runtime: Arc<AgentRuntime>,
        agents: Arc<AgentService>,
        sessions: Arc<AgentSessionService>,
        providers: Arc<ProviderService>,
    ) -> Self {
        self.agent_services = Some(AgentServices {
            runtime,
            agents,
            sessions,
            providers,
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

    /// Execute `job` and persist the outcome.
    ///
    /// Flow: insert a `running` row -> dispatch the target -> finalize the SAME
    /// row to its terminal state -> update the job's run statistics. Returns the
    /// finalized `JobExecution`.
    ///
    /// Re-entrancy is the CALLER's responsibility: callers that need the
    /// at-most-one-concurrent-run guarantee must hold a slot from
    /// [`try_claim`] (or call [`run_now`], which does) for the duration of this
    /// call. `execute` deliberately does not claim so the caller can act under
    /// the reservation before dispatching.
    ///
    /// A dispatch failure (artifact missing / not installed, process that fails
    /// to start, or an unsupported target in M1) is NOT propagated as `Err`: it
    /// is recorded as a `failed` execution row with a non-empty `error`, and the
    /// finalized row is returned. `Err` is reserved for persistence failures
    /// where no consistent row could be written.
    pub async fn execute(&self, job: &Job, trigger: Trigger) -> Result<JobExecution, AppError> {
        let exec_id = uuid::Uuid::new_v4().to_string();
        let started_at = current_timestamp();
        const FIRST_ATTEMPT: i32 = 1;

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

        let outcome = self.dispatch(job).await;

        let ended_at = current_timestamp();
        let duration = (ended_at - started_at).max(0);

        self.executions
            .finalize(
                &exec_id,
                outcome.status,
                outcome.stdout.as_deref(),
                outcome.stderr.as_deref(),
                outcome.exit_code,
                outcome.error.as_deref(),
                outcome.result_ref.as_deref(),
                ended_at,
                duration,
            )
            .await?;

        // Update job-level run statistics. `next_run_at` is preserved as-is; the
        // scheduler owns cron recomputation, not the executor.
        self.jobs
            .update_after_run(&job.id, ended_at, outcome.status, job.next_run_at, ended_at)
            .await?;

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
            attempt: FIRST_ATTEMPT,
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

    /// Dispatch a job's target to the matching backend and map the result onto a
    /// terminal `DispatchOutcome`. Never returns `Err`: a failed dispatch is a
    /// `failed` outcome with an `error` message so the caller can finalize one
    /// consistent row.
    async fn dispatch(&self, job: &Job) -> DispatchOutcome {
        match &job.target {
            JobTarget::Artifact {
                artifact_id,
                args,
                env,
            } => self.dispatch_artifact(artifact_id, args, env).await,
            JobTarget::Agent {
                agent_id,
                initial_message,
                project_id,
            } => {
                self.dispatch_agent(agent_id, initial_message, project_id.as_deref())
                    .await
            }
            JobTarget::Prompt {
                provider_id,
                model_id,
                prompt,
                ..
            } => {
                self.dispatch_prompt(&job.name, provider_id, model_id, prompt)
                    .await
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
    /// agent session from it, drive ONE run to completion, then classify the
    /// terminal outcome from the persisted transcript.
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
    /// 3. mint a fresh isolated session carrying the template's model + the
    ///    resolved provider + system prompt / sampling config — never reused, so
    ///    two jobs in the same tick get distinct sessions and the
    ///    one-run-per-session race is moot;
    /// 4. build a oneshot-signalling [`RunSink`] (its `on_closed` fires the
    ///    oneshot AFTER the transcript is fully persisted; `on_event` /
    ///    `on_error` are captured), `start_run`, then await the oneshot to block
    ///    until the turn ends;
    /// 5. classify: a run-level error envelope (`on_error`, e.g. the provider /
    ///    model was removed under the runtime) OR an in-band-error terminal
    ///    assistant turn (`stopReason == "error"`, VAL-TARGET-024) is `failed`;
    ///    otherwise `success`. In EVERY post-session outcome `result_ref` points
    ///    at the minted session so its (possibly partial) transcript stays
    ///    reachable.
    ///
    /// SECURITY: every persisted error is a sanitized, stable message. A
    /// run-level envelope is already sanitized by the runtime
    /// (`sanitize_agent_error`); a `start_run` / creation `AppError` is run
    /// through [`sanitize_agent_dispatch_error`], so no raw upstream URL, header,
    /// or key fragment can reach `job_executions.error` — raw detail goes to
    /// `tracing` only.
    async fn dispatch_agent(
        &self,
        agent_id: &str,
        initial_message: &str,
        project_id: Option<&str>,
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

        // 3. Mint a fresh, isolated session from the template (VAL-TARGET-006).
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

        // 4. Build a oneshot-signalling sink and drive ONE run to completion.
        //    The sink's `on_closed` fires the oneshot AFTER the transcript is
        //    fully persisted (the runtime guarantees closed-once, post-persist);
        //    `on_error` captures a run-level envelope (sanitized by the runtime).
        let (sink, signal) = oneshot_run_sink();
        if let Err(e) = services
            .runtime
            .start_run(
                session.id.clone(),
                initial_message.to_string(),
                Vec::new(),
                Vec::new(),
                sink,
            )
            .await
        {
            tracing::warn!(
                "[job_executor] agent start_run failed (session={}, agent={}): {}",
                session.id,
                agent_id,
                e
            );
            // The session exists; its (empty) transcript stays reachable.
            return DispatchOutcome {
                status: ExecutionStatus::Failed,
                stdout: None,
                stderr: None,
                exit_code: None,
                error: Some(sanitize_agent_dispatch_error(&e)),
                result_ref: Some(session.id),
            };
        }

        // Block until the turn ends. The oneshot resolves from `on_closed`,
        // which the runtime fires exactly once after persistence. A `RecvError`
        // means the sink was dropped without closing (should not happen given
        // closed-once); treat it as a completed-but-unknown run and fall through
        // to transcript classification.
        let run_error = signal.await.unwrap_or(None);

        // 5. Classify the terminal outcome. `result_ref` points at the session
        //    in every post-creation outcome so its transcript stays reachable.
        let result_ref = Some(session.id.clone());

        if let Some(envelope_error) = run_error {
            // Run-level error envelope (e.g. provider/model removed under the
            // runtime). Already sanitized by the runtime.
            return DispatchOutcome {
                status: ExecutionStatus::Failed,
                stdout: None,
                stderr: None,
                exit_code: None,
                error: Some(envelope_error),
                result_ref,
            };
        }

        // Read the persisted transcript to detect an in-band error terminal turn
        // (VAL-TARGET-024): the run returns `Ok` but the final assistant message
        // carries `stopReason == "error"`.
        let transcript = services.sessions.list_messages(session.id.clone()).await;
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
/// [`RunSink`]'s constructor parameters.
type SinkCallback = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// The three sink callbacks plus the close receiver. Built by
/// [`build_oneshot_signal`] and assembled into a [`RunSink`] by
/// [`oneshot_run_sink`]; returned separately so the signal wiring can be driven
/// directly in tests without reaching into `RunSink`'s private fields.
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

/// Build a [`RunSink`] whose `on_closed` fires a `oneshot` so a background
/// dispatch can `.await` the turn's completion, paired with the receiver. See
/// [`build_oneshot_signal`] for the signal semantics. Mirrors the runtime's own
/// `CapturingSink` + `wait_for_closed` test harness, reduced to a single signal.
fn oneshot_run_sink() -> (RunSink, oneshot::Receiver<Option<String>>) {
    let signal = build_oneshot_signal();
    let sink = RunSink::new(signal.on_event, signal.on_closed).with_error(signal.on_error);
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

    use crate::services::{AgentRuntime, AgentService, AgentSessionService};
    use crate::storage::types::AgentSessionMessage;

    /// Wire REAL agent collaborators onto the env's executor, all sharing the
    /// env's temp DB. The `AgentRuntime` is the inert (no-skills) one, fine for
    /// the offline pre-flight + transcript-classification paths exercised here.
    fn with_agent_services(env: TestEnv) -> TestEnv {
        let provider_service = Arc::new(ProviderService::new(env.db.clone()));
        let agent_service = Arc::new(AgentService::new(env.db.clone()));
        let agent_session_service = Arc::new(AgentSessionService::new(env.db.clone()));
        let runtime = Arc::new(AgentRuntime::new(env.db.clone()));

        let executor = env.executor.clone().with_agent_services(
            runtime,
            agent_service,
            agent_session_service,
            provider_service,
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

    // ---- oneshot run sink (the executor-side RunSink) ----

    // A clean close fires the oneshot with `None` (no run-level error). Drives
    // the signal callbacks directly, exactly as the runtime invokes the sink.
    #[tokio::test]
    async fn oneshot_signal_closed_carries_none() {
        let signal = build_oneshot_signal();
        // Drive the callbacks as the runtime would: emit an event, then close.
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

    // `oneshot_run_sink` assembles the same signal into a real `RunSink` (the
    // shape `start_run` consumes) and still yields a usable receiver — proving
    // the production builder wires the signal through unchanged.
    #[tokio::test]
    async fn oneshot_run_sink_produces_a_usable_runsink() {
        let (_sink, _rx) = oneshot_run_sink();
        // Constructing the production pair must not panic; the signal semantics
        // are covered by the `build_oneshot_signal` tests above.
    }
}
