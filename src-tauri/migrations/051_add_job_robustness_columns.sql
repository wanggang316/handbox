-- Add robustness columns to jobs: per-job execution timeout and retry policy.
-- Data layer only: these columns are stored, validated, and defaulted here;
-- the timeout-interrupt and retry-backoff behaviour are implemented by the
-- later exec-timeout / retry-backoff features.
--
-- SQLite ALTER TABLE can only ADD COLUMN, so each is NOT NULL with a DEFAULT
-- (the default backfills existing rows). Semantics:
--   exec_timeout_secs = 0  -> no timeout (unbounded run)
--   max_retries       = 0  -> no retries
--   retry_delay_secs       -> delay between retries (default 60s)
ALTER TABLE jobs ADD COLUMN exec_timeout_secs INTEGER NOT NULL DEFAULT 0;
ALTER TABLE jobs ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 0;
ALTER TABLE jobs ADD COLUMN retry_delay_secs INTEGER NOT NULL DEFAULT 60;
