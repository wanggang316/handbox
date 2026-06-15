-- Create job_executions table for per-run execution history of jobs.
-- `job_id` cascades on delete so removing a job removes its run history.
-- Cascade requires `PRAGMA foreign_keys = ON` (sqlx enables it by default).
CREATE TABLE IF NOT EXISTS job_executions (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL,
    status TEXT NOT NULL,           -- 'running' | 'success' | 'failed' | 'timeout'
    trigger TEXT NOT NULL,          -- 'schedule' | 'manual'
    attempt INTEGER NOT NULL DEFAULT 1,
    stdout TEXT,
    stderr TEXT,
    exit_code INTEGER,
    error TEXT,
    result_ref TEXT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    duration INTEGER,
    created_at INTEGER NOT NULL,

    FOREIGN KEY (job_id) REFERENCES jobs (id) ON DELETE CASCADE
);

-- Index for retrieving a job's executions newest-first.
CREATE INDEX IF NOT EXISTS idx_job_exec_job ON job_executions (job_id, created_at DESC);
