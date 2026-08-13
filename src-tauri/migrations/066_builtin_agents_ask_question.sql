-- Grant the `ask_question` tool to both builtin AgentDefinitions.
--
-- `builtin-chat` was seeded (058) with an EMPTY tool set, so a plain "New chat"
-- session registers no tools at all — which made the reverse-questioning tool
-- unreachable on the very surface it matters most on (`ask_question` needs no
-- working directory and is a conversation tool, not a coding one).
--
-- APPEND-ONLY and idempotent: the NOT EXISTS guard means a definition that
-- already lists the tool is left untouched, and nothing else in a user's
-- curated `builtin_tools` array is rewritten. Builtins are editable (only
-- deletion is blocked), so this must never clobber their list.
--
-- Existing SESSIONS keep their own `enabled_tools` snapshot and are deliberately
-- unaffected, matching the settings page's stated contract.
UPDATE agents
SET builtin_tools = json_insert(builtin_tools, '$[#]', 'ask_question'),
    updated_at = CAST(strftime('%s', 'now') AS INTEGER) * 1000
WHERE id IN ('builtin-chat', 'builtin-coding')
  AND json_valid(builtin_tools)
  AND NOT EXISTS (
      SELECT 1 FROM json_each(agents.builtin_tools) WHERE value = 'ask_question'
  );

-- Backfill the UNUSED builtin-chat sessions too.
--
-- "New chat" does not always create a session: it REUSES an existing empty
-- builtin-chat session (MainSidebar.handleNewChatClick) when one exists. Such a
-- session was snapshotted before this migration, so without this the user keeps
-- landing in a tool-less chat and `ask_question` can never fire — the exact
-- symptom that the definition-level update above appears to fix but doesn't.
--
-- Restricted to `message_count = 0`: a session with no history is
-- indistinguishable from a fresh one (that is precisely the assumption the reuse
-- path already makes), so upgrading it matches what the user would get today.
-- Sessions that carry history keep their snapshot, per the settings page's
-- "existing sessions are unaffected" contract.
UPDATE agent_sessions
SET enabled_tools = json_insert(enabled_tools, '$[#]', 'ask_question'),
    updated_at = CAST(strftime('%s', 'now') AS INTEGER) * 1000
WHERE agent_definition_id = 'builtin-chat'
  AND message_count = 0
  AND json_valid(enabled_tools)
  AND NOT EXISTS (
      SELECT 1 FROM json_each(agent_sessions.enabled_tools) WHERE value = 'ask_question'
  );
