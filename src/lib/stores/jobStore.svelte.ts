/**
 * Scheduled-job store.
 *
 * Svelte 5 `$state` for the job list plus loading/error state, exported as a
 * singleton. Calls backend IPC commands via `api/job`; CRUD syncs the
 * in-memory list in place to avoid a full refetch every time.
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

  /** Normalize an error into a displayable string and store it in `error`. */
  function setError(e: unknown): void {
    error = e instanceof AppError ? e.message : String(e);
  }

  /** Upsert a job into the in-memory list (replace by id or insert at the head). */
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

    /** Load the job list (newest first). */
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

    /** Fetch one job, upsert it into the list, and return the latest value. */
    async refresh(jobId: UUID): Promise<Job> {
      const job = await getJob(jobId);
      upsert(job);
      return job;
    },

    /** Create a job and insert it at the head of the list. */
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

    /** Update a job definition and sync the list. */
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

    /** Delete a job and remove it from the list. */
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

    /** Enable/disable a job and sync the list. */
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

    clearError(): void {
      error = null;
    },
  };
}

export const jobStore = createJobStore();
