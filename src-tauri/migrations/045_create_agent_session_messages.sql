-- Create agent_session_messages table for Agent-mode message history.
-- `payload` stores a full serialized hand-agent Message as JSON.
CREATE TABLE IF NOT EXISTS agent_session_messages (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    role TEXT NOT NULL,
    payload TEXT NOT NULL,          -- JSON: serialized hand-agent Message
    created_at INTEGER NOT NULL,

    FOREIGN KEY (session_id) REFERENCES agent_sessions (id) ON DELETE CASCADE
);

-- Create index for ordered retrieval within a session
CREATE INDEX IF NOT EXISTS idx_agent_session_messages_session_seq ON agent_session_messages (session_id, seq);
