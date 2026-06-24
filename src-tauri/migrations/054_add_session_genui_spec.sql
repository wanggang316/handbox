-- Snapshot the linked GenUI template spec onto the chat session at creation.
-- "Example" (few-shot) generative-UI mode: when a chat agent links a saved GenUI
-- (agents.genui_id -> genui.id), its spec text is frozen onto the session here so
-- message-build can inject it as an exemplar. Write-once, mirroring generative_ui /
-- system_prompt: nullable with NO DEFAULT, existing rows stay NULL and decode to
-- `None` (no example), and update_session deliberately never writes this column.
ALTER TABLE sessions ADD COLUMN genui_spec TEXT;  -- write-once snapshot of the linked GenUI spec (few-shot example)
