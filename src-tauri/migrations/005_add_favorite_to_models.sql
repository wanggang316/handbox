-- Add favorite field to models table
ALTER TABLE models ADD COLUMN favorite BOOLEAN NOT NULL DEFAULT 0;

-- Create index for favorite field
CREATE INDEX IF NOT EXISTS idx_models_favorite ON models (favorite);