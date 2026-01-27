-- Create agents table for storing reusable agent configurations
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    model TEXT,
    temperature REAL,
    top_p REAL,
    top_k INTEGER,
    reasoning TEXT,              -- JSON: AgentReasoningConfig
    max_tokens INTEGER,
    system_prompt TEXT,
    mcp_servers TEXT,            -- JSON: Vec<McpServerConfig>
    skills TEXT,                 -- JSON: Vec<String> (skill names/IDs)
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_agents_updated_at ON agents (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_agents_name ON agents (name);
