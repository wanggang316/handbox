-- Remove the status index that is no longer needed
DROP INDEX IF EXISTS idx_providers_status;

-- Remove probe-related fields from providers table
ALTER TABLE providers DROP COLUMN status;
ALTER TABLE providers DROP COLUMN last_probe_at;
ALTER TABLE providers DROP COLUMN probe_result;

