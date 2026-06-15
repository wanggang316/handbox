/**
 * 定时任务 (Scheduled Job) 相关 API 封装。
 *
 * 经 `apiCall<T>` 调用后端 IPC 命令（`job_*`），统一复用 AppError。
 * 后端命令签名见 `src-tauri/src/commands/job.rs`：
 * - `job_create(request: JobCreatePayload)`
 * - `job_list(limit?, offset?)`
 * - `job_get(jobId)`
 * - `job_update(jobId, request: JobUpdatePayload)`
 * - `job_delete(jobId)`
 * - `job_set_enabled(jobId, enabled)`
 * - `job_execution_list(jobId, limit?, offset?)`
 * - `job_run_now(jobId)`
 */

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

/** 创建任务的入参（对应后端 `JobCreatePayload`，字段 camelCase）。 */
export interface JobCreateInput {
  name: string;
  description?: string;
  target: JobTarget;
  cronExpr: string;
  timezone: string;
  enabled?: boolean;
}

/** 更新任务定义的入参（对应后端 `JobUpdatePayload`）。 */
export interface JobUpdateInput {
  name: string;
  description?: string;
  target: JobTarget;
  cronExpr: string;
  timezone: string;
  enabled: boolean;
}

/**
 * 预览 cron 调度：返回未来至多 `n`（默认 5）个本地时区毫秒时间戳，升序，
 * 首项严格晚于当前时刻。稀疏调度返回真实条数（可能少于 n、甚至为空）。
 * 非法 / 越界 / 空白 cron 由后端抛出结构化 `AppError`（`{code,message,hint}`）。
 */
export async function previewSchedule(
  cron: string,
  n?: number,
): Promise<Timestamp[]> {
  return apiCall<Timestamp[]>("job_preview_schedule", { cronExpr: cron, n });
}

/** 创建新的定时任务。 */
export async function createJob(input: JobCreateInput): Promise<Job> {
  return apiCall<Job>("job_create", { request: input });
}

/** 获取任务列表（最新优先），可分页。 */
export async function listJobs(
  limit?: number,
  offset?: number,
): Promise<Job[]> {
  return apiCall<Job[]>("job_list", { limit, offset });
}

/** 获取单个任务详情。 */
export async function getJob(jobId: UUID): Promise<Job> {
  return apiCall<Job>("job_get", { jobId });
}

/** 全量替换任务定义字段。 */
export async function updateJob(
  jobId: UUID,
  input: JobUpdateInput,
): Promise<Job> {
  return apiCall<Job>("job_update", { jobId, request: input });
}

/** 删除任务（其执行历史级联删除）。 */
export async function deleteJob(jobId: UUID): Promise<void> {
  return apiCall<void>("job_delete", { jobId });
}

/** 启用/禁用任务。 */
export async function setJobEnabled(
  jobId: UUID,
  enabled: boolean,
): Promise<Job> {
  return apiCall<Job>("job_set_enabled", { jobId, enabled });
}

/**
 * 获取任务的执行历史（最新优先），可分页。包含进行中（running）行，
 * 因此时间线无需依赖事件订阅即可显示在跑的运行。从未执行的任务返回空数组。
 */
export async function listExecutions(
  jobId: UUID,
  limit?: number,
  offset?: number,
): Promise<JobExecution[]> {
  return apiCall<JobExecution[]>("job_execution_list", { jobId, limit, offset });
}

/**
 * 立即手动运行一个任务（`trigger = manual`），与调度无关。禁用任务也可运行
 * （禁用仅停自动调度）。返回该次执行的终态记录；若该任务已有执行进行中
 * （调度或先前的手动运行），后端返回 `CONFLICT` 错误且不写第二条记录。
 */
export async function runNow(jobId: UUID): Promise<JobExecution> {
  return apiCall<JobExecution>("job_run_now", { jobId });
}

/**
 * `job_executed` 事件 payload（镜像后端 `JobExecutedEvent`）。
 * 执行开始（写入 running 行）与完成（终态）时各 emit 一次：
 * - `jobId`：让 /jobs 列表知道刷新哪张卡片；
 * - `executionId`：让打开的详情时间线按 id 匹配并原地翻转该行；
 * - `status`：该行当前状态（开始为 `running`，完成为终态）。
 */
export interface JobExecutedEvent {
  jobId: UUID;
  executionId: UUID;
  status: ExecutionStatus;
}

/**
 * 订阅 `job_executed` 事件。返回的 `UnlistenFn` 必须在组件卸载时调用以取消订阅。
 *
 * 该事件仅作为「实时刷新触发器」——事实来源始终是 `job_execution_list` /
 * `job_get` 命令，错过一次事件不会导致状态错乱（VAL-HISTORY-030）。
 */
export async function listenJobExecuted(
  handler: (payload: JobExecutedEvent) => void,
): Promise<UnlistenFn> {
  return listen<JobExecutedEvent>("job_executed", (event) => {
    handler(event.payload);
  });
}
