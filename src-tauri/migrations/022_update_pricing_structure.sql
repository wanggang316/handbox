
DROP TABLE IF EXISTS models;
CREATE TABLE models (
    id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    context_length INTEGER,
    output_max_tokens INTEGER,
    pricing TEXT,
    input_modalities TEXT,
    output_modalities TEXT,
    support_parameters TEXT,
    default_parameters TEXT,
    max_parameters TEXT,
    supported_features TEXT,
    metadata TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    favorite BOOLEAN NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    PRIMARY KEY (id, provider_id),
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_models_provider ON models (provider_id);
CREATE INDEX IF NOT EXISTS idx_models_enabled ON models (enabled);
