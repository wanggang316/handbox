CREATE TABLE IF NOT EXISTS words (
  id TEXT PRIMARY KEY NOT NULL,
  term TEXT NOT NULL,
  language TEXT NOT NULL,
  translation TEXT NOT NULL,
  phonetic TEXT,
  note TEXT,
  tags TEXT,
  source TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_words_term ON words(term);
CREATE INDEX IF NOT EXISTS idx_words_created_at ON words(created_at DESC);

CREATE TABLE IF NOT EXISTS word_contexts (
  id TEXT PRIMARY KEY NOT NULL,
  word_id TEXT NOT NULL,
  context_text TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_id TEXT,
  created_at INTEGER NOT NULL,
  FOREIGN KEY (word_id) REFERENCES words(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_word_contexts_word_id ON word_contexts(word_id);

CREATE TABLE IF NOT EXISTS word_reviews (
  word_id TEXT PRIMARY KEY NOT NULL,
  ease REAL NOT NULL,
  interval_days INTEGER NOT NULL,
  next_review_at INTEGER NOT NULL,
  last_reviewed_at INTEGER,
  review_count INTEGER NOT NULL,
  FOREIGN KEY (word_id) REFERENCES words(id) ON DELETE CASCADE
);
