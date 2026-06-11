-- Create agent_sessions table for Agent-mode session instances
-- Independent from the Chat-mode `sessions` table.
CREATE TABLE IF NOT EXISTS agent_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    model_id TEXT,
    provider_id TEXT,
    system_prompt TEXT,
    thinking_level TEXT,
    temperature REAL,
    max_tokens INTEGER,
    working_dir TEXT,
    enabled_tools TEXT,             -- JSON: Vec<String> (tool names)
    tool_execution_mode TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    last_message_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_agent_sessions_updated_at ON agent_sessions (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_sessions_name ON agent_sessions (name);
