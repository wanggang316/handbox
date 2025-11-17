-- Add turn_count column to chats table
-- This column is used to limit the number of conversation turns included in the context

ALTER TABLE chats ADD COLUMN turn_count INTEGER DEFAULT 5;
