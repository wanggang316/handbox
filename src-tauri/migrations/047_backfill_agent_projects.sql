-- Backfill agent_projects from existing agent_sessions working directories.
-- working_dir values are stored canonical, so grouping is plain string equality
-- (no filesystem access here). Sessions with NULL/empty working_dir stay unlinked.
--
-- Project name = path basename. SQLite has no basename function, so:
--   replace(path, '/', '')              -> charset of all non-slash characters
--   rtrim(path, <charset>)              -> prefix up to and including the last '/'
--   replace(path, <prefix>, '')         -> basename (the prefix ends at the last
--                                          '/', so it can only occur at position 0)
-- When the basename comes out empty (root '/', trailing-slash paths), fall back
-- to the full path. Paths without any '/' resolve to themselves because
-- replace(X, '', '') returns X unchanged.
--
-- Project created_at/updated_at = max session activity in that directory,
-- where activity = coalesce(last_message_at, created_at).
INSERT INTO agent_projects (id, path, name, created_at, updated_at)
SELECT
    lower(hex(randomblob(16))),
    working_dir,
    CASE
        WHEN replace(working_dir, rtrim(working_dir, replace(working_dir, '/', '')), '') = ''
            THEN working_dir
        ELSE replace(working_dir, rtrim(working_dir, replace(working_dir, '/', '')), '')
    END,
    MAX(COALESCE(last_message_at, created_at)),
    MAX(COALESCE(last_message_at, created_at))
FROM agent_sessions
WHERE working_dir IS NOT NULL AND working_dir != ''
GROUP BY working_dir;

-- Link existing sessions to their backfilled project. Only the new project_id
-- column is written; no pre-existing agent_sessions column is touched.
UPDATE agent_sessions
SET project_id = (
    SELECT p.id FROM agent_projects p WHERE p.path = agent_sessions.working_dir
)
WHERE working_dir IS NOT NULL AND working_dir != '';
