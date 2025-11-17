-- Rename output_token_limit column to output_max_tokens on models table
ALTER TABLE models RENAME COLUMN output_token_limit TO output_max_tokens;
