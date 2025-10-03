-- Add tools field to messages table
ALTER TABLE messages ADD COLUMN tools TEXT; -- JSON encoded MessageTools data