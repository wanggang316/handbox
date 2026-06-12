-- Add per-session enabled_skills to agent_sessions.
-- Nullable with NO DEFAULT, mirroring enabled_tools: existing rows stay NULL
-- and are read back as an empty Vec by the repository (NULL-decode footgun guard).
ALTER TABLE agent_sessions ADD COLUMN enabled_skills TEXT;             -- JSON: Vec<String> (skill names)
