/**
 * 定时任务状态管理 Store。
 *
 * 使用 Svelte 5 的 `$state` 维护任务列表与加载/错误状态，单例导出。
 * 经 `api/job` 调用后端 IPC 命令；CRUD 后就地同步内存列表，避免每次都全量重拉。
 */

import {
  createJob,
  deleteJob,
  getJob,
  listJobs,
  setJobEnabled,
  updateJob,
  type JobCreateInput,
  type JobUpdateInput,
} from "$lib/api/job";
import { AppError } from "$lib/api";
import type { Job, UUID } from "$lib/types";

function createJobStore() {
  let jobs = $state<Job[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  /** 统一把错误归一化为可展示的字符串，并存入 `error`。 */
  function setError(e: unknown): void {
    error = e instanceof AppError ? e.message : String(e);
  }

  /** 把单个任务 upsert 进内存列表（按 id 替换或插入到队首）。 */
  function upsert(job: Job): void {
    const idx = jobs.findIndex((j) => j.id === job.id);
    if (idx >= 0) {
      jobs[idx] = job;
    } else {
      jobs = [job, ...jobs];
    }
  }

  return {
    get jobs() {
      return jobs;
    },
    get isLoading() {
      return isLoading;
    },
    get error() {
      return error;
    },

    /** 加载任务列表（最新优先）。 */
    async load(limit?: number, offset?: number): Promise<void> {
      isLoading = true;
      error = null;
      try {
        jobs = await listJobs(limit, offset);
      } catch (e) {
        setError(e);
        throw e;
      } finally {
        isLoading = false;
      }
    },

    /** 拉取单个任务并 upsert 进列表，返回最新值。 */
    async refresh(jobId: UUID): Promise<Job> {
      const job = await getJob(jobId);
      upsert(job);
      return job;
    },

    /** 创建任务并插入列表队首。 */
    async create(input: JobCreateInput): Promise<Job> {
      error = null;
      try {
        const job = await createJob(input);
        upsert(job);
        return job;
      } catch (e) {
        setError(e);
        throw e;
      }
    },

    /** 更新任务定义并同步列表。 */
    async update(jobId: UUID, input: JobUpdateInput): Promise<Job> {
      error = null;
      try {
        const job = await updateJob(jobId, input);
        upsert(job);
        return job;
      } catch (e) {
        setError(e);
        throw e;
      }
    },

    /** 删除任务并从列表移除。 */
    async delete(jobId: UUID): Promise<void> {
      error = null;
      try {
        await deleteJob(jobId);
        jobs = jobs.filter((j) => j.id !== jobId);
      } catch (e) {
        setError(e);
        throw e;
      }
    },

    /** 启用/禁用任务并同步列表。 */
    async setEnabled(jobId: UUID, enabled: boolean): Promise<Job> {
      error = null;
      try {
        const job = await setJobEnabled(jobId, enabled);
        upsert(job);
        return job;
      } catch (e) {
        setError(e);
        throw e;
      }
    },

    /** 清空错误状态。 */
    clearError(): void {
      error = null;
    },
  };
}

// 导出单例实例
export const jobStore = createJobStore();
