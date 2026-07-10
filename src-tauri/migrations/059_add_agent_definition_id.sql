-- P3: link an AgentSession back to the AgentDefinition it was instantiated from.
-- Additive + nullable; legacy sessions and sessions created via the direct
-- `create_session` path keep NULL. Decoded with the same Option<T> guard used for
-- the other nullable columns. Write-once at create — the generic update path never
-- rewrites it (same discipline as project_id).
ALTER TABLE agent_sessions ADD COLUMN agent_definition_id TEXT;
