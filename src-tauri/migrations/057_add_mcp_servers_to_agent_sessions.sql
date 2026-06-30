-- P1: per-session MCP server bindings for the unified agent loop.
-- JSON array of McpServerConfig { serverId, executionMode, enabledTools }.
-- Nullable / no default: NULL or absent decodes to an empty Vec in row_to_session,
-- matching the existing NULL-decode guard used for enabled_tools.
ALTER TABLE agent_sessions ADD COLUMN mcp_servers TEXT;
