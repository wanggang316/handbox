-- GenUI: named, reusable JSON-Render UI specs that a chat agent can be linked to.
-- `spec` stores the raw spec JSON text (the frontend validates it via explainSpec
-- before saving); the backend treats it as an opaque string and never parses it.
CREATE TABLE IF NOT EXISTS genui (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    spec TEXT NOT NULL,            -- raw JSON-Render spec text (opaque to the backend)
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_genui_updated_at ON genui (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_genui_name ON genui (name);

-- Link a chat agent to a saved GenUI. Nullable with NO DEFAULT, mirroring the
-- other optional agent columns (generative_ui): existing rows stay NULL and read
-- back as `None` (NULL-decode guard). No FK is enforced via ALTER; deleting a
-- GenUI nulls out referencing agents at the repository layer, and any stray
-- dangling id surfaces as "unset" in the form rather than an error.
ALTER TABLE agents ADD COLUMN genui_id TEXT;
