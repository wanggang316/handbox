-- User-defined agent tools ("dynamic tools").
--
-- A definition is PRESENTATIONAL: the tool performs no work. The model calls it
-- with the parameters declared here, and the frontend renders the linked GenUI
-- spec with those arguments as its state model. Same doctrine as `render_card`
-- (services/extensions/render_card.rs): the payload travels in the toolcall
-- block's arguments, so the Rust handler only validates and acknowledges.
--
-- `name` is the registration name the model sees and is therefore UNIQUE: it
-- shares one namespace with the built-in tool ids and with `mcp__*`, both of
-- which the service layer rejects.
--
-- `parameters` is a JSON array of {name, type, description, required}; the tool
-- schema handed to the model is compiled from it (`extensions::dynamic_tool`),
-- never stored, so a schema fix ships with the code rather than needing a
-- migration over user rows.
--
-- Global scope in v1, mirroring agent_hook_rules (migration 063): `enabled` is
-- the only gate and every agent session sees the same enabled set. Per-agent /
-- per-session scoping would put the tool's `name` on the existing
-- `enabled_tools` list rather than adding a column here.
CREATE TABLE tool_definitions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    icon TEXT,
    description TEXT NOT NULL,
    parameters TEXT NOT NULL DEFAULT '[]',
    genui_id TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Every run assembles "the enabled definitions, in display order", so that is
-- the access shape worth an index.
CREATE INDEX idx_tool_definitions_enabled ON tool_definitions (enabled, sort_order);
