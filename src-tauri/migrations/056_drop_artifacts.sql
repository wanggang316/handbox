-- Remove the Artifacts feature.
--
-- The Artifacts feature (sidebar "Artifacts" entry, the /artifacts management
-- page, and the artifact job target) has been removed from the app, so the
-- `artifacts` table is no longer read or written.
--
-- The `sessions.artifact_id` column was dead schema: it was added with the very
-- first chats table but never wired to any chat↔artifact behaviour. Drop it (and
-- its index) now that the artifacts table is gone. SQLite >= 3.35 supports
-- ALTER TABLE ... DROP COLUMN (already used by migration 039); no table rebuild
-- needed. No foreign key ever referenced these, so message/chat persistence is
-- unaffected.
DROP INDEX IF EXISTS idx_sessions_artifact_id;
ALTER TABLE sessions DROP COLUMN artifact_id;
DROP TABLE IF EXISTS artifacts;
