/**
 * Mirrors the Rust serde shapes in `storage/types/job.rs`: camelCase fields;
 * `JobTarget` is a discriminated union on `kind` (snake_case tag values).
 */

import type { BaseEntity, UUID, Timestamp } from "./index";

export type ExecutionStatus = "running" | "success" | "failed" | "timeout";

export type Trigger = "schedule" | "manual";

// Currently the only strategy: a fresh session per run.
export type SessionStrategy = "new_session";

export interface AgentTarget {
  kind: "agent";
  agentId: UUID;
  // Agent definitions carry no model; each job selects its own.
  modelId: string;
  initialMessage: string;
  projectId?: UUID;
}

export interface PromptTarget {
  kind: "prompt";
  providerId: string;
  modelId: string;
  prompt: string;
  sessionStrategy?: SessionStrategy;
}

export type JobTarget = AgentTarget | PromptTarget;

// Named defaults for the robustness settings, kept in sync with the constants
// in `storage/types/job.rs` (0 = unlimited timeout / no retries). Used to
// backfill blank form fields.
export const DEFAULT_EXEC_TIMEOUT_SECS = 0;
export const DEFAULT_MAX_RETRIES = 0;
export const DEFAULT_RETRY_DELAY_SECS = 60;

export interface Job extends BaseEntity {
  name: string;
  description?: string;
  target: JobTarget;
  cronExpr: string;
  timezone: string;
  enabled: boolean;
  lastRunAt?: Timestamp;
  nextRunAt?: Timestamp;
  lastStatus?: ExecutionStatus;
  runCount: number;
  failureCount: number;
  // Per-run timeout in seconds; 0 = unlimited.
  execTimeoutSecs: number;
  // Max retries after failure; 0 = no retries.
  maxRetries: number;
  // Retry delay in seconds.
  retryDelaySecs: number;
}

export interface JobExecution {
  id: UUID;
  jobId: UUID;
  status: ExecutionStatus;
  trigger: Trigger;
  attempt: number;
  stdout?: string;
  stderr?: string;
  exitCode?: number;
  error?: string;
  resultRef?: string;
  startedAt: Timestamp;
  endedAt?: Timestamp;
  duration?: number; // milliseconds
  createdAt: Timestamp;
}
