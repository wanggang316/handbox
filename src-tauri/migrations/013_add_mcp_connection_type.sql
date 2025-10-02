-- Add connection type support to MCP servers table
ALTER TABLE mcps ADD COLUMN connection_type TEXT NOT NULL DEFAULT 'stdio';
ALTER TABLE mcps ADD COLUMN endpoint TEXT;
ALTER TABLE mcps ADD COLUMN headers TEXT DEFAULT '{}';
ALTER TABLE mcps ADD COLUMN timeout_ms INTEGER;

-- Create index for connection type queries
CREATE INDEX IF NOT EXISTS idx_mcps_connection_type ON mcps(connection_type);

-- Update existing records to use stdio type
UPDATE mcps SET connection_type = 'stdio' WHERE connection_type IS NULL;