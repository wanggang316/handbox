/**
 * Single source of truth for the built-in agent tools' registration names,
 * ordering and display labels: the coding-agent built-ins plus HandBox's
 * extension tools. Consumed by both the agent-tools settings page and the
 * per-session tool popover in AgentInput, so the two views always list the
 * same tools in the same order.
 *
 * `id` is the registration name the backend gates on; array order is the
 * display order. `labelKey` is an i18n key, rendered via `t()`.
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
  Globe,
  PanelTop,
  AppWindow,
  MessageCircleQuestionMark,
  Sparkles,
} from "@lucide/svelte";
import type { MessageKey } from "$lib/i18n";
import { BUILTIN_TOOL_IDS } from "./builtinToolIds";

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
  { id: "web_search", labelKey: "agent.tool.web_search", icon: Globe, requiresWorkingDir: false },
  { id: "render_card", labelKey: "agent.tool.render_card", icon: PanelTop, requiresWorkingDir: false },
  { id: "render_app", labelKey: "agent.tool.render_app", icon: AppWindow, requiresWorkingDir: false },
  { id: "ask_question", labelKey: "agent.tool.ask_question", icon: MessageCircleQuestionMark, requiresWorkingDir: false },
  { id: "skill", labelKey: "agent.tool.skill", icon: Sparkles, requiresWorkingDir: false },
];

/**
 * All tool ids in canonical order — the default enabled set (everything on).
 * Re-exported from the icon-free `builtinToolIds` module so pure modules can
 * import the ids without this file's Lucide `.svelte` imports. `BUILTIN_TOOLS`
 * above is kept in sync by convention (same ids, same order), not derived.
 */
export { BUILTIN_TOOL_IDS };
