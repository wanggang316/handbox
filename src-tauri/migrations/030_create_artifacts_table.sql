-- Create artifacts table
-- Artifacts are executable applications that can optionally use AI models
CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,

    -- Application type: shell, python, web
    type TEXT NOT NULL CHECK(type IN ('shell', 'python', 'web')),

    -- Code/Resource paths (relative to sandbox artifacts directory)
    entry_file TEXT NOT NULL,      -- Main entry point (e.g., "main.sh", "app.py", "index.html")
    source_path TEXT,              -- Source directory path for multi-file apps

    -- Optional AI model configuration
    model_id TEXT,
    provider_id TEXT,
    system_prompt TEXT,
    model_parameters TEXT,         -- JSON: { temperature, topP, maxTokens, etc. }
    tools TEXT,                    -- JSON: array of enabled tools/MCP servers

    -- Execution configuration
    execution_config TEXT,         -- JSON: { args: [], env: {}, permissions: [], timeout: 30000 }

    -- Installation & lifecycle
    is_builtin BOOLEAN NOT NULL DEFAULT 0,     -- Internal vs user-created
    is_installed BOOLEAN NOT NULL DEFAULT 0,   -- Whether copied to sandbox
    installed_version TEXT,                     -- Version string for updates
    installed_at INTEGER,
    last_run_at INTEGER,
    run_count INTEGER NOT NULL DEFAULT 0,

    -- Metadata
    tags TEXT,                     -- JSON: array of tag strings
    icon TEXT,                     -- Icon name or emoji
    author TEXT,                   -- Creator name

    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    FOREIGN KEY (model_id, provider_id) REFERENCES models (id, provider_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts (type);
CREATE INDEX IF NOT EXISTS idx_artifacts_is_builtin ON artifacts (is_builtin);
CREATE INDEX IF NOT EXISTS idx_artifacts_is_installed ON artifacts (is_installed);
CREATE INDEX IF NOT EXISTS idx_artifacts_last_run_at ON artifacts (last_run_at DESC);
CREATE INDEX IF NOT EXISTS idx_artifacts_updated_at ON artifacts (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_artifacts_model ON artifacts (model_id, provider_id);
