/**
 * Pure helpers for presenting a tool call: the namespaced MCP name split and
 * the one-line argument summary a collapsed tool row shows.
 *
 * Kept icon-free (no `.svelte` imports) so the module runs under the plain-Node
 * Vitest environment; the icon side of the same presentation lives in
 * `$lib/constants/agentTools`.
 */

/** Namespace a per-session MCP tool registers under: `mcp__<serverId>__<tool>`. */
const MCP_PREFIX = "mcp__";
const MCP_SEPARATOR = "__";

export interface McpToolName {
  /** MCP server id (a uuid) — resolve it against the server list for a label. */
  serverId: string;
  /** Tool name as the server itself exposes it. */
  tool: string;
}

/**
 * Split `mcp__<serverId>__<tool>` (see `agent_run.rs`, which builds the name);
 * null for a built-in, or for a name that only looks namespaced. Server ids are
 * uuids and carry no `__`, so the first separator after the prefix is the split.
 */
export function parseMcpToolName(toolName: string): McpToolName | null {
  if (!toolName.startsWith(MCP_PREFIX)) return null;
  const rest = toolName.slice(MCP_PREFIX.length);
  const separator = rest.indexOf(MCP_SEPARATOR);
  if (separator <= 0) return null;
  const tool = rest.slice(separator + MCP_SEPARATOR.length);
  if (!tool) return null;
  return { serverId: rest.slice(0, separator), tool };
}

/**
 * Argument keys that carry the "what" of a call, most specific first: bash's
 * `command`, grep/find's `pattern`, web_search's `query`, read/write/ls's
 * `path`, edit's `file_path`. Order decides the winner when a tool has several:
 * grep takes both `pattern` and `path` and is about the pattern, while write
 * takes both `path` and `content` and is about the path.
 */
const SUMMARY_KEYS = [
  "command",
  "pattern",
  "query",
  "path",
  "file_path",
  "url",
  "name",
  "title",
  "question",
];

/** Cap on the preview held in the DOM; the row also truncates visually. */
const SUMMARY_MAX_LENGTH = 200;

/**
 * One-line preview of what a call operates on — the path it reads, the command
 * it runs — so a collapsed row says something concrete instead of a bare tool
 * name. Empty when the arguments carry no string worth showing (the row then
 * renders the name alone); the expanded body always has the full arguments.
 *
 * Unknown shapes (an MCP server's own schema) fall back to the first non-empty
 * string value, which is nearly always the tool's subject.
 */
export function toolArgSummary(args: unknown): string {
  const record = asRecord(args);
  if (!record) return typeof args === "string" ? oneLine(args) : "";

  for (const key of SUMMARY_KEYS) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) return oneLine(value);
  }
  for (const value of Object.values(record)) {
    if (typeof value === "string" && value.trim()) return oneLine(value);
  }
  return "";
}

/**
 * A call's arguments as a plain object, or `{}` when they are not object-shaped.
 *
 * This is the state model a custom tool's GenUI view renders against, so it must
 * never be null: a card with no data renders empty, which is recoverable, while
 * a missing state model is not.
 */
export function toolArgsRecord(args: unknown): Record<string, unknown> {
  return asRecord(args) ?? {};
}

/**
 * Arguments as an object. The live path can deliver them as a JSON string
 * (`tool_execution` events forward the model's raw arguments), so a string that
 * parses to an object is treated as that object.
 */
function asRecord(args: unknown): Record<string, unknown> | null {
  if (typeof args === "string") {
    try {
      return asRecord(JSON.parse(args));
    } catch {
      return null;
    }
  }
  if (args && typeof args === "object" && !Array.isArray(args)) {
    return args as Record<string, unknown>;
  }
  return null;
}

/** Collapse to a single line and cap the length; multi-line commands are common. */
function oneLine(value: string): string {
  const collapsed = value.replace(/\s+/g, " ").trim();
  return collapsed.length > SUMMARY_MAX_LENGTH
    ? `${collapsed.slice(0, SUMMARY_MAX_LENGTH)}…`
    : collapsed;
}
