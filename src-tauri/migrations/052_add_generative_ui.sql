-- Add generative_ui flag to chat agents and their sessions.
-- Nullable with NO DEFAULT, mirroring the other optional columns: existing rows
-- stay NULL and are read back as `None` by the repositories (NULL-decode footgun
-- guard). Semantics: NULL/None == "off". SQLite stores the bool as INTEGER 0/1.
ALTER TABLE agents ADD COLUMN generative_ui INTEGER;    -- bool: enable generative UI for the chat agent
ALTER TABLE sessions ADD COLUMN generative_ui INTEGER;  -- bool: write-once snapshot of the agent's generative_ui at session creation
