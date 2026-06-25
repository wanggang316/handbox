/**
 * Built-in coding-agent tool ids — the icon-free source of truth for the 7
 * tools' registration names in canonical order.
 *
 * Split out of `agentTools.ts` so pure, icon-free modules (e.g. the
 * quick-action session-request builder and its Node-environment Vitest suite)
 * can import the canonical id list without dragging in the Lucide `.svelte`
 * icon imports that `agentTools.ts` carries. `agentTools.ts` re-exports
 * `BUILTIN_TOOL_IDS` from here and derives it for `BUILTIN_TOOLS`, so the order
 * stays single-sourced.
 *
 * Each id == the coding-agent registration name the backend gates on, in
 * canonical display order.
 */
export const BUILTIN_TOOL_IDS: string[] = [
  "read",
  "write",
  "edit",
  "bash",
  "grep",
  "find",
  "ls",
];
