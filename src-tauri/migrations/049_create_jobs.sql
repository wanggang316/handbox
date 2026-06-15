-- Create jobs table for scheduled-job definitions.
-- Schema foundation for the scheduled-jobs feature; downstream features
-- (types / repository / executor / scheduler) depend on this table.
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    target_kind TEXT NOT NULL,              -- 'artifact' | 'agent' | 'prompt'
    target_config TEXT NOT NULL DEFAULT '{}', -- JSON: target-specific config
    cron_expr TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'local',
    enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at INTEGER,
    next_run_at INTEGER,
    last_status TEXT,
    run_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Index for the scheduler's due-job scan (enabled jobs ordered by next run).
CREATE INDEX IF NOT EXISTS idx_jobs_enabled_next ON jobs (enabled, next_run_at);
