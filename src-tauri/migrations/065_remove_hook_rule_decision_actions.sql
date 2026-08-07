-- Hooks execute actions; gating what the agent may do belongs to the agent's
-- permission configuration. The declarative decision actions (deny/ask/allow)
-- are removed from the action set, so rows carrying them can no longer be
-- decoded — delete them rather than leave rules that look active but never
-- load. A command hook can still veto through its verdict; that path is
-- unaffected.
DELETE FROM agent_hook_rules WHERE action IN ('deny', 'ask', 'allow');
