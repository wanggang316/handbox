-- Update models table to include additional metadata fields
ALTER TABLE models ADD COLUMN metadata TEXT;
ALTER TABLE models ADD COLUMN output_token_limit INTEGER;
ALTER TABLE models ADD COLUMN description TEXT;
ALTER TABLE models ADD COLUMN input_modalities TEXT;
ALTER TABLE models ADD COLUMN output_modalities TEXT;
ALTER TABLE models ADD COLUMN pricing TEXT;
