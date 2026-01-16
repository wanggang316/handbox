CREATE TABLE favorites (
    id TEXT PRIMARY KEY NOT NULL,
    message_id TEXT NOT NULL,
    chat_id TEXT NOT NULL,
    content TEXT NOT NULL,
    role TEXT NOT NULL,
    message_type TEXT NOT NULL DEFAULT 'message',
    tags TEXT,
    note TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (chat_id) REFERENCES chats (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_favorites_chat_id ON favorites(chat_id);
CREATE INDEX IF NOT EXISTS idx_favorites_tags ON favorites(tags);
CREATE INDEX IF NOT EXISTS idx_favorites_message_type ON favorites(message_type);
