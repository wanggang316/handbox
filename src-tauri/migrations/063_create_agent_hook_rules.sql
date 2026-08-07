-- Declarative hook rules evaluated against the agent's tool calls.
--
-- Global scope in v1: every agent session sees the same enabled rule set,
-- matched in `sort_order`, stopping at the first hit. Per-agent / per-session
-- scoping would add a nullable owner column here rather than a second table.
--
-- `arg_field` NULL means the substring in `arg_contains` is matched against the
-- whole serialized arguments object, so a rule can catch a value without knowing
-- which parameter carries it. `arg_contains` NULL/empty matches on the tool
-- pattern alone.
CREATE TABLE agent_hook_rules (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    event TEXT NOT NULL,
    tool_pattern TEXT NOT NULL,
    arg_field TEXT,
    arg_contains TEXT,
    action TEXT NOT NULL,
    message TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- The dispatch path reads "enabled rules for one event, in order" on every
-- single tool call, so that access shape is the index.
CREATE INDEX idx_agent_hook_rules_event ON agent_hook_rules (event, enabled, sort_order);
