-- Drop the legacy chat-mode `sessions` and `messages` tables.
--
-- Chat mode has been replaced by the unified agent engine: every conversation
-- now runs through `hand_coding_agent::AgentSession` and persists to
-- `agent_sessions` / `agent_session_messages` (JSONL transcript). The chat
-- backend that read and wrote these tables -- the chat_engine, message, and
-- session services with their repositories and IPC commands, plus the
-- provider/model/mcp deletion guards that counted chats against them -- has
-- been removed, so no surviving code references either table.
--
-- Drop `messages` first: its `session_id` foreign key references `sessions (id)`
-- (migration 042). DROP TABLE also removes a table's indexes and triggers, so
-- the message-count triggers on `sessions` are torn down with `messages`.
DROP TABLE IF EXISTS messages;
DROP TABLE IF EXISTS sessions;
