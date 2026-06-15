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
 */

import { apiCall } from "./index";
import type { Job, JobTarget, UUID } from "../types";

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
