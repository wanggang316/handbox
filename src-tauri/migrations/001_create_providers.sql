-- Create providers table
CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    base_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'inactive',
    enabled BOOLEAN NOT NULL DEFAULT 1,
    last_probe_at INTEGER,
    probe_result TEXT, -- JSON encoded ProbeResult
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    
    -- Create indexes for common queries
    UNIQUE(name)
);

-- Create models table
CREATE TABLE IF NOT EXISTS models (
    id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL,
    context_length INTEGER,
    input_cost REAL,
    output_cost REAL,
    supported_features TEXT NOT NULL, -- JSON encoded array
    enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    
    PRIMARY KEY (id, provider_id),
    FOREIGN KEY (provider_id) REFERENCES providers (id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_providers_type ON providers (provider_type);
CREATE INDEX IF NOT EXISTS idx_providers_status ON providers (status);
CREATE INDEX IF NOT EXISTS idx_providers_enabled ON providers (enabled);
CREATE INDEX IF NOT EXISTS idx_models_provider ON models (provider_id);
CREATE INDEX IF NOT EXISTS idx_models_enabled ON models (enabled);