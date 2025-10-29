-- Add model_created_at column to models table
-- This field stores the model creation timestamp from the API provider (optional)
ALTER TABLE models ADD COLUMN model_created_at INTEGER;
