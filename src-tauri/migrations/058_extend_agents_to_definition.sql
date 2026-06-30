-- Extend `agents` into the AgentDefinition model: capability set + run policy +
-- builtin flag + presentation. All columns are additive and nullable; the
-- repository decodes NULL with the same guards used for mcp_servers/skills, so
-- legacy rows keep working untouched.
ALTER TABLE agents ADD COLUMN provider_id TEXT;          -- chosen provider (fixes provider=None)
ALTER TABLE agents ADD COLUMN icon TEXT;                 -- lucide icon name
ALTER TABLE agents ADD COLUMN description TEXT;          -- short blurb
ALTER TABLE agents ADD COLUMN builtin INTEGER;           -- 1 = system row (NULL/0 = user)
ALTER TABLE agents ADD COLUMN builtin_tools TEXT;        -- JSON Vec<String>, coding-agent tool names
ALTER TABLE agents ADD COLUMN working_dir_mode TEXT;     -- "required" | "optional" | "none"
ALTER TABLE agents ADD COLUMN tool_execution_mode TEXT;  -- "auto" | "manual" default policy
ALTER TABLE agents ADD COLUMN thinking_level TEXT;       -- coding-agent thinking level
ALTER TABLE agents ADD COLUMN starters TEXT;             -- JSON Vec<String>, starter prompts

-- Seed the two builtin AgentDefinitions with fixed ids. INSERT OR IGNORE keeps
-- this idempotent across re-runs and never clobbers a user's later edits.
-- builtin-chat: zero builtin tools -> pure conversation (empty enabled set
-- registers no tools); no working dir; MCP can still be attached at use time.
INSERT OR IGNORE INTO agents (
    id, name, icon, description, builtin, builtin_tools,
    working_dir_mode, tool_execution_mode, created_at, updated_at
) VALUES (
    'builtin-chat', '通用对话', 'message-circle',
    '无内置工具的纯对话，可挂载 MCP 工具', 1, '[]',
    'none', 'auto',
    CAST(strftime('%s','now') AS INTEGER) * 1000,
    CAST(strftime('%s','now') AS INTEGER) * 1000
);

-- builtin-coding: the seven coding-agent built-in tools, working dir required,
-- dangerous tools gated behind manual approval.
INSERT OR IGNORE INTO agents (
    id, name, icon, description, builtin, builtin_tools,
    working_dir_mode, tool_execution_mode, created_at, updated_at
) VALUES (
    'builtin-coding', 'Coding', 'code',
    '带文件读写与命令执行的编码助手', 1,
    '["read","write","edit","bash","grep","find","ls"]',
    'required', 'manual',
    CAST(strftime('%s','now') AS INTEGER) * 1000,
    CAST(strftime('%s','now') AS INTEGER) * 1000
);
