-- Add turn_id column to messages table to group related messages in a conversation turn
-- turn_id is an integer starting from 1, incrementing for each conversation turn
ALTER TABLE messages ADD COLUMN turn_id INTEGER;

-- Create index for turn_id to optimize queries
CREATE INDEX IF NOT EXISTS idx_messages_turn_id ON messages (turn_id);

-- Create composite index for chat_id and turn_id queries
CREATE INDEX IF NOT EXISTS idx_messages_chat_turn ON messages (chat_id, turn_id);