-- Add enabled_tools field to MCP servers table
-- This field stores a JSON array of enabled tool names
ALTER TABLE mcps ADD COLUMN enabled_tools TEXT NOT NULL DEFAULT '[]';
