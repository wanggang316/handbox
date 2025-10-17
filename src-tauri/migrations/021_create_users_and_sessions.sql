-- 用户表
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,              -- Google User ID
    username TEXT NOT NULL,           -- 用户名
    email TEXT NOT NULL UNIQUE,       -- 邮箱（唯一）
    avatar TEXT,                      -- 头像 URL
    is_pro INTEGER NOT NULL DEFAULT 0, -- 是否为 Pro 用户
    created_at TEXT NOT NULL,         -- 创建时间
    updated_at TEXT NOT NULL          -- 更新时间
);

-- 用户会话表
CREATE TABLE IF NOT EXISTS user_sessions (
    user_id TEXT PRIMARY KEY,         -- 用户 ID（外键）
    token_expires_at INTEGER NOT NULL, -- Token 过期时间（Unix timestamp）
    created_at TEXT NOT NULL,         -- 创建时间
    updated_at TEXT,                  -- 更新时间
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON user_sessions(user_id);
