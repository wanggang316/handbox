-- Migration to restructure chat and message configuration
-- Move configuration fields from messages to chats table 
-- Add config field to messages table for per-message configuration

-- Step 1: Add new configuration fields to chats table
ALTER TABLE chats ADD COLUMN temperature REAL;
ALTER TABLE chats ADD COLUMN top_p REAL;  
ALTER TABLE chats ADD COLUMN max_tokens INTEGER;
ALTER TABLE chats ADD COLUMN stream BOOLEAN DEFAULT 1;
ALTER TABLE chats ADD COLUMN model_id TEXT;
ALTER TABLE chats ADD COLUMN provider_id TEXT;

-- Step 2: Add config field to messages table for per-message configuration
ALTER TABLE messages ADD COLUMN config TEXT; -- JSON encoded configuration

-- Step 3: Migrate existing message-level configurations to chat-level defaults
-- This will take the configuration from the first message in each chat as the chat's default
UPDATE chats SET 
    temperature = (
        SELECT temperature FROM messages 
        WHERE chat_id = chats.id AND temperature IS NOT NULL 
        ORDER BY created_at ASC LIMIT 1
    ),
    top_p = (
        SELECT top_p FROM messages 
        WHERE chat_id = chats.id AND top_p IS NOT NULL 
        ORDER BY created_at ASC LIMIT 1
    ),
    max_tokens = (
        SELECT max_tokens FROM messages 
        WHERE chat_id = chats.id AND max_tokens IS NOT NULL 
        ORDER BY created_at ASC LIMIT 1
    ),
    stream = (
        SELECT stream FROM messages 
        WHERE chat_id = chats.id AND stream IS NOT NULL 
        ORDER BY created_at ASC LIMIT 1
    ),
    model_id = (
        SELECT model_id FROM messages 
        WHERE chat_id = chats.id AND model_id IS NOT NULL 
        ORDER BY created_at ASC LIMIT 1
    ),
    provider_id = (
        SELECT provider_id FROM messages 
        WHERE chat_id = chats.id AND provider_id IS NOT NULL 
        ORDER BY created_at ASC LIMIT 1
    );

-- Step 4: Populate config field in messages with per-message configuration
UPDATE messages SET config = json_object(
    'temperature', temperature,
    'top_p', top_p,
    'max_tokens', max_tokens,
    'stream', stream,
    'model_id', model_id,
    'provider_id', provider_id,
    'system_prompt', (SELECT system_prompt FROM chats WHERE id = messages.chat_id),
    'mcp_servers', (SELECT mcp_servers FROM chats WHERE id = messages.chat_id)
) WHERE temperature IS NOT NULL 
   OR top_p IS NOT NULL 
   OR max_tokens IS NOT NULL 
   OR stream IS NOT NULL 
   OR model_id IS NOT NULL 
   OR provider_id IS NOT NULL;

-- Step 5: Remove old configuration fields from messages table
-- Note: SQLite doesn't support DROP COLUMN directly, so we need to recreate the table

-- Create temporary messages table with new structure
CREATE TABLE messages_new (
    id TEXT PRIMARY KEY NOT NULL,
    chat_id TEXT NOT NULL,
    role TEXT NOT NULL, -- 'user', 'assistant', 'system'
    content TEXT NOT NULL,
    
    -- Per-message configuration stored as JSON
    config TEXT, -- JSON encoded configuration including temperature, top_p, max_tokens, stream, model_id, provider_id, system_prompt, mcp_servers etc.
    
    -- Attachments stored as JSON
    attachments TEXT, -- JSON encoded array of MessageAttachment
    
    -- Usage and timing metadata
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    start_time INTEGER,
    end_time INTEGER,
    duration INTEGER,
    
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    
    FOREIGN KEY (chat_id) REFERENCES chats (id) ON DELETE CASCADE
);

-- Copy data from old messages table to new one
INSERT INTO messages_new (
    id, chat_id, role, content, config, attachments,
    input_tokens, output_tokens, total_tokens,
    start_time, end_time, duration,
    created_at, updated_at
)
SELECT 
    id, chat_id, role, content, config, attachments,
    input_tokens, output_tokens, total_tokens,
    start_time, end_time, duration,
    created_at, updated_at
FROM messages;

-- Drop old messages table and rename new one
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;

-- Step 6: Update indexes for performance
CREATE INDEX IF NOT EXISTS idx_chats_updated_at ON chats (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chats_artifact_id ON chats (artifact_id);
CREATE INDEX IF NOT EXISTS idx_chats_model ON chats (model_id, provider_id);
CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages (chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_role ON messages (role);

-- Step 7: Recreate triggers for chat statistics
CREATE TRIGGER IF NOT EXISTS update_chat_stats_on_message_insert
    AFTER INSERT ON messages
BEGIN
    UPDATE chats 
    SET 
        last_message_at = NEW.created_at,
        message_count = message_count + 1,
        updated_at = NEW.created_at
    WHERE id = NEW.chat_id;
END;

CREATE TRIGGER IF NOT EXISTS update_chat_stats_on_message_delete
    AFTER DELETE ON messages
BEGIN
    UPDATE chats 
    SET 
        message_count = message_count - 1,
        updated_at = strftime('%s', 'now') * 1000
    WHERE id = OLD.chat_id;
    
    -- Update last_message_at to the most recent remaining message
    UPDATE chats 
    SET last_message_at = (
        SELECT MAX(created_at) 
        FROM messages 
        WHERE chat_id = OLD.chat_id
    )
    WHERE id = OLD.chat_id;
END;