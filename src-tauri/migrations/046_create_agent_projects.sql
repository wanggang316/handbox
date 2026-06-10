-- Create agent_projects table for grouping Agent-mode sessions by working directory.
-- Independent from Chat-mode tables (sessions/messages) and /agents presets (agents).
CREATE TABLE IF NOT EXISTS agent_projects (
    id TEXT PRIMARY KEY NOT NULL,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Link Agent-mode sessions to a project; NULL means "no working directory".
ALTER TABLE agent_sessions ADD COLUMN project_id TEXT REFERENCES agent_projects (id);
