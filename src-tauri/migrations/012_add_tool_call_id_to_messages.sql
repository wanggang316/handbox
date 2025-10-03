-- Add tool_call_id column to messages table to link tool result messages with their tool calls
ALTER TABLE messages ADD COLUMN tool_call_id TEXT;

-- Create index for tool_call_id to optimize queries
CREATE INDEX IF NOT EXISTS idx_messages_tool_call_id ON messages (tool_call_id);