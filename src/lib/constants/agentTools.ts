/**
 * Built-in coding-agent tools — the single source of truth for the 7 tools'
 * registration names, ordering and display labels. Consumed by both the
 * "Agent 工具" settings page (global default) and the per-session tool popover
 * in AgentInput, so the two views always list the same tools in the same order.
 *
 * `id` == the coding-agent registration name the backend gates on; the order
 * here is the canonical display order. `labelKey` is an i18n key — consumers
 * render it via `t(tool.labelKey)` so labels follow the UI language.
 */
import type { Icon as IconType } from "@lucide/svelte";
import {
  FileText,
  FilePlus,
  FilePen,
  Terminal,
  Search,
  FileSearch,
  FolderTree,
} from "@lucide/svelte";
import type { MessageKey } from "$lib/i18n";

export interface BuiltinTool {
  /** coding-agent registration name; backend `build_agent_session` gates on this. */
  id: string;
  /** i18n key for the display label; render with `t(labelKey)`. */
  labelKey: MessageKey;
  icon: typeof IconType;
  /** Tool operates inside the working dir; disabled when a session has none. */
  requiresWorkingDir: boolean;
}

export const BUILTIN_TOOLS: BuiltinTool[] = [
  { id: "read", labelKey: "agent.tool.read", icon: FileText, requiresWorkingDir: true },
  { id: "write", labelKey: "agent.tool.write", icon: FilePlus, requiresWorkingDir: true },
  { id: "edit", labelKey: "agent.tool.edit", icon: FilePen, requiresWorkingDir: true },
  { id: "bash", labelKey: "agent.tool.bash", icon: Terminal, requiresWorkingDir: true },
  { id: "grep", labelKey: "agent.tool.grep", icon: Search, requiresWorkingDir: true },
  { id: "find", labelKey: "agent.tool.find", icon: FileSearch, requiresWorkingDir: true },
  { id: "ls", labelKey: "agent.tool.ls", icon: FolderTree, requiresWorkingDir: true },
];

/** All 7 tool ids in canonical order — the default enabled set (everything on). */
export const BUILTIN_TOOL_IDS: string[] = BUILTIN_TOOLS.map((t) => t.id);
