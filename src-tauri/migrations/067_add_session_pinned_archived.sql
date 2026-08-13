-- Sidebar pin / archive flags for agent sessions.
--
-- Both are presentation state, not activity: pinned floats a session to the top
-- of its sibling list, archived moves it out of the main tree into the
-- collapsed "Archived" group. Neither participates in the activity ordering
-- (`last_message_at` / `created_at`), so they get their own columns rather than
-- riding on `updated_at`.
--
-- NOT NULL DEFAULT 0 makes every existing row unpinned and unarchived without a
-- backfill pass, and lets the repository decode them as plain `bool`.
ALTER TABLE agent_sessions ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_sessions ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
