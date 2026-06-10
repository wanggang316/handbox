/**
 * Agent Project 分组折叠态 - Svelte 5 runes + localStorage 持久化
 *
 * 按 project id 记忆折叠状态；未分组桶使用保留 key `UNGROUPED_COLLAPSE_KEY`。
 * 持久化形态为 `{ [id]: true }`（只记录折叠项；展开为默认态不落盘）。
 * 损坏 / 缺失 / 非法值一律 fallback 到展开（空 map）。
 */

const COLLAPSE_STORAGE_KEY = "agentProjectCollapse";

/** 未分组桶的保留折叠 key（不会与 UUID 项目 id 冲突）。 */
export const UNGROUPED_COLLAPSE_KEY = "__ungrouped__";

function loadPersistedCollapse(): Record<string, boolean> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(COLLAPSE_STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed)
    ) {
      return {};
    }
    // 仅保留严格为 true 的条目，其余视为损坏并 fallback 到展开。
    const result: Record<string, boolean> = {};
    for (const [key, value] of Object.entries(parsed)) {
      if (value === true) {
        result[key] = true;
      }
    }
    return result;
  } catch {
    return {};
  }
}

let collapsed = $state<Record<string, boolean>>(loadPersistedCollapse());

function persistCollapse(): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(COLLAPSE_STORAGE_KEY, JSON.stringify(collapsed));
  } catch (error) {
    // 持久化失败（如配额）不影响内存内状态。
    console.error("Failed to persist agent project collapse state:", error);
  }
}

export const agentProjectCollapse = {
  /** 某分组是否处于折叠态（缺失即展开）。 */
  isCollapsed(id: string): boolean {
    return collapsed[id] === true;
  },

  /** 切换某分组折叠态并持久化。 */
  toggle(id: string): void {
    const next = { ...collapsed };
    if (next[id] === true) {
      delete next[id];
    } else {
      next[id] = true;
    }
    collapsed = next;
    persistCollapse();
  },

  /** 强制展开某分组（供「打开 session 自动展开所属分组」使用）。 */
  expand(id: string): void {
    if (collapsed[id] !== true) return;
    const next = { ...collapsed };
    delete next[id];
    collapsed = next;
    persistCollapse();
  },
};
