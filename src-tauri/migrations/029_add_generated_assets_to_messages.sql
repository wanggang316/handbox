-- Add generated_assets column to messages table for storing output resources
ALTER TABLE messages ADD COLUMN generated_assets TEXT;
