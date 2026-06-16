/**
 * Built-in coding-agent tools — the single source of truth for the 7 tools'
 * registration names, ordering and Chinese labels. Consumed by both the
 * "Agent 工具" settings page (global default) and the per-session tool popover
 * in AgentInput, so the two views always list the same tools in the same order.
 *
 * `id` == the coding-agent registration name the backend gates on; the order
 * here is the canonical display order.
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

export interface BuiltinTool {
  /** coding-agent registration name; backend `build_agent_session` gates on this. */
  id: string;
  /** Chinese display label. */
  label: string;
  icon: typeof IconType;
  /** Tool operates inside the working dir; disabled when a session has none. */
  requiresWorkingDir: boolean;
}

export const BUILTIN_TOOLS: BuiltinTool[] = [
  { id: "read", label: "读取文件", icon: FileText, requiresWorkingDir: true },
  { id: "write", label: "写入文件", icon: FilePlus, requiresWorkingDir: true },
  { id: "edit", label: "编辑文件", icon: FilePen, requiresWorkingDir: true },
  { id: "bash", label: "执行命令", icon: Terminal, requiresWorkingDir: true },
  { id: "grep", label: "搜索内容", icon: Search, requiresWorkingDir: true },
  { id: "find", label: "查找文件", icon: FileSearch, requiresWorkingDir: true },
  { id: "ls", label: "列目录", icon: FolderTree, requiresWorkingDir: true },
];

/** All 7 tool ids in canonical order — the default enabled set (everything on). */
export const BUILTIN_TOOL_IDS: string[] = BUILTIN_TOOLS.map((t) => t.id);
