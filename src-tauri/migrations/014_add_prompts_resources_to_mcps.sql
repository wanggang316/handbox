-- Add prompts and resources fields to MCP servers table
ALTER TABLE mcps ADD COLUMN prompts TEXT NOT NULL DEFAULT '[]';
ALTER TABLE mcps ADD COLUMN resources TEXT NOT NULL DEFAULT '[]';
