-- Create MCP servers table
CREATE TABLE IF NOT EXISTS mcps (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    display_name TEXT,
    description TEXT,
    command TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]',
    working_dir TEXT,
    env TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'inactive',
    tools TEXT NOT NULL DEFAULT '[]',
    last_sync_at INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mcps_enabled ON mcps(enabled);
CREATE INDEX IF NOT EXISTS idx_mcps_updated_at ON mcps(updated_at);
