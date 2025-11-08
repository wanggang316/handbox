-- Add top_k column to chats table
-- This column is used to control the number of highest probability vocabulary tokens to keep for top-k-filtering

ALTER TABLE chats ADD COLUMN top_k INTEGER;
