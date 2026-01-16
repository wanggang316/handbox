CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);

CREATE TABLE IF NOT EXISTS favorite_tags (
    favorite_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (favorite_id, tag_id),
    FOREIGN KEY (favorite_id) REFERENCES favorites(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_favorite_tags_favorite_id ON favorite_tags(favorite_id);
CREATE INDEX IF NOT EXISTS idx_favorite_tags_tag_id ON favorite_tags(tag_id);
