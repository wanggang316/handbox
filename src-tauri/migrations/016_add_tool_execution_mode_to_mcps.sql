-- Add tool_execution_mode field to MCP servers table
-- This field stores a JSON object mapping tool names to their execution modes
-- Format: { "tool_name": "auto" | "manual" }
-- Default is "auto" for all tools
ALTER TABLE mcps ADD COLUMN tool_execution_mode TEXT NOT NULL DEFAULT '{}';
