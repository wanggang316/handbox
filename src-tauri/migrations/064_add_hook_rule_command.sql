-- The `run_command` action: a hook that DOES something rather than only
-- deciding. The command runs on the matched event, receives the event as JSON
-- on stdin, and its stdout/exit code can still decide the call — which makes
-- decide-only actions a special case of this one rather than a separate world.
--
-- Both columns are nullable: every existing rule uses a built-in action and
-- carries neither.
ALTER TABLE agent_hook_rules ADD COLUMN command TEXT;

-- Per-rule budget in milliseconds. NULL takes the built-in default. A hook that
-- never returns must not hang the turn, so this is enforced, not advisory.
ALTER TABLE agent_hook_rules ADD COLUMN timeout_ms INTEGER;
