-- Add api_key column to providers table
ALTER TABLE providers ADD COLUMN api_key TEXT DEFAULT '';
