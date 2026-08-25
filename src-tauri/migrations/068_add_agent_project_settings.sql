-- Per-project sidebar settings: pin, color, and a default "Open in ..." target.
--
-- `pinned` is presentation state, like `agent_sessions.pinned`: it floats a
-- project to the top of the sidebar's project section and takes no part in the
-- activity ordering. NOT NULL DEFAULT 0 makes every existing row unpinned
-- without a backfill pass and lets the repository decode it as a plain `bool`.
--
-- `color` and `default_editor_id` are nullable because NULL is a real, distinct
-- value for both: no color (the row keeps the sidebar's default text color) and
-- "follow the global default editor" (`settings.agent.defaultEditorId`).
ALTER TABLE agent_projects ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_projects ADD COLUMN color TEXT;
ALTER TABLE agent_projects ADD COLUMN default_editor_id TEXT;
