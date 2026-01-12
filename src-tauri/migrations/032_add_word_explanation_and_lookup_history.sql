ALTER TABLE words ADD COLUMN explanation TEXT;

CREATE TABLE IF NOT EXISTS word_lookup_history (
  id TEXT PRIMARY KEY NOT NULL,
  term TEXT NOT NULL,
  translation TEXT,
  phonetic TEXT,
  explanation TEXT,
  source_language TEXT,
  target_language TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_word_lookup_history_created_at
  ON word_lookup_history(created_at DESC);
