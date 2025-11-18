-- Add generated_images column to messages table
-- This stores JSON array of image metadata (paths, mime types)
ALTER TABLE messages ADD COLUMN generated_images TEXT;
