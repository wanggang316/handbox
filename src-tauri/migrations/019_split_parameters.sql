-- Split parameters into support_parameters, default_parameters, and max_parameters
-- default_parameters and max_parameters are simple key-value maps: {"temperature": 1.0, "top_p": 0.95}
-- Only store values that exist, do not store null or 0

-- Add new columns
ALTER TABLE models ADD COLUMN support_parameters TEXT;
ALTER TABLE models ADD COLUMN default_parameters TEXT;
ALTER TABLE models ADD COLUMN max_parameters TEXT;

-- Drop the old parameters column
ALTER TABLE models DROP COLUMN parameters;
