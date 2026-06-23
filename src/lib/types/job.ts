/**
 * 定时任务 (Scheduled Job) 相关类型定义。
 *
 * 镜像 Rust 端 `storage/types/job.rs` 的 serde 表示：字段 camelCase，
 * `JobTarget` 为按 `kind` 判别的 discriminated union（tag 值 snake_case）。
 */

import type { BaseEntity, UUID, Timestamp } from "./index";

// 单次运行的结果状态
export type ExecutionStatus = "running" | "success" | "failed" | "timeout";

// 触发来源
export type Trigger = "schedule" | "manual";

// Prompt 目标的会话策略（当前仅「每次新建会话」）
export type SessionStrategy = "new_session";

// 触发 Agent（可选限定 project）
export interface AgentTarget {
  kind: "agent";
  agentId: UUID;
  initialMessage: string;
  projectId?: UUID;
}

// 向 provider/model 发送一次性 prompt
export interface PromptTarget {
  kind: "prompt";
  providerId: string;
  modelId: string;
  prompt: string;
  sessionStrategy?: SessionStrategy;
}

// 任务目标：按 `kind` 判别的联合类型
export type JobTarget = AgentTarget | PromptTarget;

// 健壮性配置的具名默认值（与 Rust 端 storage/types/job.rs 常量保持一致）。
// execTimeoutSecs=0 表示不限超时；maxRetries=0 表示不重试；retryDelaySecs 默认 60s。
// 实际的超时中断 / 重试退避行为由后续 feature 实现，本处仅用于表单留空回填。
export const DEFAULT_EXEC_TIMEOUT_SECS = 0;
export const DEFAULT_MAX_RETRIES = 0;
export const DEFAULT_RETRY_DELAY_SECS = 60;

// 定时任务定义（对应 jobs 表）
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
  // 每次运行超时（秒），0 表示不限；执行语义见后续 exec-timeout feature
  execTimeoutSecs: number;
  // 失败后最大重试次数，0 表示不重试；执行语义见后续 retry-backoff feature
  maxRetries: number;
  // 重试间隔（秒）
  retryDelaySecs: number;
}

// 单次执行记录（对应 job_executions 表）
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
