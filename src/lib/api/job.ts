
import { apiCall } from "./index";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ExecutionStatus,
  Job,
  JobExecution,
  JobTarget,
  Timestamp,
  UUID,
} from "../types";

/** Mirrors the backend's `JobCreatePayload`. */
export interface JobCreateInput {
  name: string;
  description?: string;
  target: JobTarget;
  cronExpr: string;
  timezone: string;
  enabled?: boolean;
  /** Per-run timeout in seconds; omitted → backend default (0 = unlimited). */
  execTimeoutSecs?: number;
  /** Omitted → backend default (0 = no retries). */
  maxRetries?: number;
  /** Retry delay in seconds; omitted → backend default (60). */
  retryDelaySecs?: number;
}

/** Mirrors the backend's `JobUpdatePayload`. */
export interface JobUpdateInput {
  name: string;
  description?: string;
  target: JobTarget;
  cronExpr: string;
  timezone: string;
  enabled: boolean;
  /** Per-run timeout in seconds; omitted → backend default (0 = unlimited). */
  execTimeoutSecs?: number;
  /** Omitted → backend default (0 = no retries). */
  maxRetries?: number;
  /** Retry delay in seconds; omitted → backend default (60). */
  retryDelaySecs?: number;
}

/**
 * Previews a cron schedule: up to `n` (default 5) local-timezone millisecond
 * timestamps, ascending, first strictly after now. Sparse schedules may yield
 * fewer than `n` entries (possibly none). Invalid cron raises an AppError.
 */
export async function previewSchedule(
  cron: string,
  n?: number,
): Promise<Timestamp[]> {
  return apiCall<Timestamp[]>("job_preview_schedule", { cronExpr: cron, n });
}

export async function createJob(input: JobCreateInput): Promise<Job> {
  return apiCall<Job>("job_create", { request: input });
}

/** Newest first. */
export async function listJobs(
  limit?: number,
  offset?: number,
): Promise<Job[]> {
  return apiCall<Job[]>("job_list", { limit, offset });
}

export async function getJob(jobId: UUID): Promise<Job> {
  return apiCall<Job>("job_get", { jobId });
}

/** Full replacement of the job definition, not a partial update. */
export async function updateJob(
  jobId: UUID,
  input: JobUpdateInput,
): Promise<Job> {
  return apiCall<Job>("job_update", { jobId, request: input });
}

/** Execution history is deleted with the job. */
export async function deleteJob(jobId: UUID): Promise<void> {
  return apiCall<void>("job_delete", { jobId });
}

export async function setJobEnabled(
  jobId: UUID,
  enabled: boolean,
): Promise<Job> {
  return apiCall<Job>("job_set_enabled", { jobId, enabled });
}

/**
 * Newest first. Includes running rows, so the timeline can show in-flight runs
 * without an event subscription. Never-run jobs return an empty array.
 */
export async function listExecutions(
  jobId: UUID,
  limit?: number,
  offset?: number,
): Promise<JobExecution[]> {
  return apiCall<JobExecution[]>("job_execution_list", { jobId, limit, offset });
}

/**
 * Manual run (`trigger = manual`), independent of scheduling — disabled jobs
 * can run too (disabling only stops the scheduler). Resolves with the final
 * execution record; if a run is already in flight the backend returns CONFLICT
 * without writing a second record.
 */
export async function runNow(jobId: UUID): Promise<JobExecution> {
  return apiCall<JobExecution>("job_run_now", { jobId });
}

/**
 * `job_executed` payload (mirrors the backend event). Emitted once when a run
 * starts (`status: "running"`) and once when it reaches a terminal state.
 */
export interface JobExecutedEvent {
  jobId: UUID;
  executionId: UUID;
  status: ExecutionStatus;
}

/**
 * The event is only a refresh trigger — the source of truth is always the
 * `job_execution_list` / `job_get` commands, so a missed event cannot corrupt
 * state. Call the returned `UnlistenFn` on component unmount.
 */
export async function listenJobExecuted(
  handler: (payload: JobExecutedEvent) => void,
): Promise<UnlistenFn> {
  return listen<JobExecutedEvent>("job_executed", (event) => {
    handler(event.payload);
  });
}
