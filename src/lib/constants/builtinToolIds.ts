/**
 * Canonical built-in tool ids, icon-free — the registration names the backend
 * gates on, in display order: the coding-agent built-ins plus HandBox's
 * extension tools (`web_search` / `render_card` / `render_app` are registered
 * backend-side via `extra_tools`; `skill` gates the coding-agent skill
 * pipeline).
 *
 * Kept icon-free so pure modules and Node-environment tests can import the
 * list without the Lucide `.svelte` imports in `agentTools.ts`, which
 * re-exports it. That file's `BUILTIN_TOOLS` is kept in sync with this list by
 * convention (same ids, same order) — neither is derived from the other.
 */
export const BUILTIN_TOOL_IDS: string[] = [
  "read",
  "write",
  "edit",
  "bash",
  "grep",
  "find",
  "ls",
  "web_search",
  "render_card",
  "render_app",
  "ask_question",
  "skill",
];
