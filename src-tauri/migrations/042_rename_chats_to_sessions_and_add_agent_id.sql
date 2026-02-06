-- Rename chats table to sessions and add agent_id field

-- Step 1: Create new sessions table with agent_id and all existing fields
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    last_message_at INTEGER,
    message_count INTEGER NOT NULL DEFAULT 0,
    artifact_id TEXT,
    agent_id TEXT,
    system_prompt TEXT,
    mcp_servers TEXT, -- JSON encoded array of MCP server names
    temperature REAL,
    top_p REAL,
    top_k INTEGER,
    max_tokens INTEGER,
    stream BOOLEAN DEFAULT 1,
    model_id TEXT,
    provider_id TEXT,
    turn_count INTEGER,
    reasoning TEXT, -- JSON encoded reasoning configuration
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    FOREIGN KEY (agent_id) REFERENCES agents (id) ON DELETE SET NULL
);

-- Step 2: Copy data from chats to sessions
INSERT INTO sessions (
    id, name, last_message_at, message_count, artifact_id,
    system_prompt, mcp_servers, temperature, top_p, top_k,
    max_tokens, stream, model_id, provider_id, turn_count, reasoning,
    created_at, updated_at
)
SELECT
    id, name, last_message_at, message_count, artifact_id,
    system_prompt, mcp_servers, temperature, top_p, top_k,
    max_tokens, stream, model_id, provider_id, turn_count, reasoning,
    created_at, updated_at
FROM chats;

-- Step 3: Drop old chats table
DROP TABLE chats;

-- Step 4: Create indexes for sessions
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions (updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_artifact_id ON sessions (artifact_id);
CREATE INDEX IF NOT EXISTS idx_sessions_agent_id ON sessions (agent_id);
CREATE INDEX IF NOT EXISTS idx_sessions_model ON sessions (model_id, provider_id);

-- Step 5: Update messages table to reference sessions instead of chats
-- Note: SQLite doesn't support ALTER TABLE with DROP FOREIGN KEY,
-- so we need to recreate the messages table

-- Create new messages table with session_id
CREATE TABLE IF NOT EXISTS messages_new (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL, -- 'user', 'assistant', 'system'
    content TEXT NOT NULL,
    reasoning TEXT, -- Reasoning process content

    -- Per-message configuration stored as JSON
    config TEXT, -- JSON encoded configuration

    -- Attachments stored as JSON
    attachments TEXT, -- JSON encoded array of MessageAttachment

    -- Usage and timing metadata
    input_tokens INTEGER,
    output_tokens INTEGER,
    total_tokens INTEGER,
    start_time INTEGER,
    end_time INTEGER,
    duration INTEGER,

    -- Tools
    tools TEXT, -- JSON encoded array of tools

    -- Turn tracking
    turn_id INTEGER,
    tool_call_id TEXT,

    -- Generated assets
    generated_assets TEXT, -- JSON encoded array of generated assets

    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,

    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE CASCADE
);

-- Copy data from old messages to new messages
INSERT INTO messages_new (
    id, session_id, role, content, reasoning, config, attachments,
    input_tokens, output_tokens, total_tokens,
    start_time, end_time, duration,
    tools, turn_id, tool_call_id, generated_assets,
    created_at, updated_at
)
SELECT
    id, chat_id, role, content, reasoning, config, attachments,
    input_tokens, output_tokens, total_tokens,
    start_time, end_time, duration,
    tools, turn_id, tool_call_id, generated_assets,
    created_at, updated_at
FROM messages;

-- Drop old messages table
DROP TABLE messages;

-- Rename messages_new to messages
ALTER TABLE messages_new RENAME TO messages;

-- Create indexes for messages
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages (session_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_role ON messages (role);

-- Step 6: Recreate triggers for sessions
CREATE TRIGGER IF NOT EXISTS update_session_stats_on_message_insert
    AFTER INSERT ON messages
BEGIN
    UPDATE sessions
    SET
        last_message_at = NEW.created_at,
        message_count = message_count + 1,
        updated_at = NEW.created_at
    WHERE id = NEW.session_id;
END;

CREATE TRIGGER IF NOT EXISTS update_session_stats_on_message_delete
    AFTER DELETE ON messages
BEGIN
    UPDATE sessions
    SET
        message_count = message_count - 1,
        updated_at = strftime('%s', 'now') * 1000
    WHERE id = OLD.session_id;

    -- Update last_message_at to the most recent remaining message
    UPDATE sessions
    SET last_message_at = (
        SELECT MAX(created_at)
        FROM messages
        WHERE session_id = OLD.session_id
    )
    WHERE id = OLD.session_id;
END;
