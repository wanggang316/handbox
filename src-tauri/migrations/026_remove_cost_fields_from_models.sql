-- Remove deprecated input_cost and output_cost fields from models table
-- These fields have been replaced by the pricing JSON field

ALTER TABLE models DROP COLUMN input_cost;
ALTER TABLE models DROP COLUMN output_cost;
