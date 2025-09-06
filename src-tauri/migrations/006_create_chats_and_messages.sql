-- Create chats table (renamed from ChatSession)
CREATE TABLE IF NOT EXISTS chats (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    last_message_at INTEGER,
    message_count INTEGER NOT NULL DEFAULT 0,
    artifact_id TEXT,
    system_prompt TEXT,
    mcp_servers TEXT, -- JSON encoded array of MCP server names
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Create messages table
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY NOT NULL,
    chat_id TEXT NOT NULL,
    role TEXT NOT NULL, -- 'user', 'assistant', 'system'
    content TEXT NOT NULL,
    
    -- Model information moved to message level
    model_id TEXT,
    provider_id TEXT,
    
    -- Model parameters for this specific message
    temperature REAL,
    top_p REAL,
    max_tokens INTEGER,
    stream BOOLEAN DEFAULT 1,
    
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
    
    FOREIGN KEY (chat_id) REFERENCES chats (id) ON DELETE CASCADE,
    FOREIGN KEY (model_id, provider_id) REFERENCES models (id, provider_id)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_chats_updated_at ON chats (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chats_artifact_id ON chats (artifact_id);
CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages (chat_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_role ON messages (role);
CREATE INDEX IF NOT EXISTS idx_messages_model ON messages (model_id, provider_id);

-- Create trigger to update chat's last_message_at and message_count
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