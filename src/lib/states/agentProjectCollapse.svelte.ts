/**
 * Sidebar group collapse state - Svelte 5 runes + localStorage persistence.
 *
 * Collapse state is remembered per opaque string key supplied by the caller:
 * project groups use project.id and the ungrouped group uses
 * `UNGROUPED_BUCKET_KEY`. Persisted shape is `{ [key]: true }` (only collapsed
 * entries are stored; expanded is the default and never written). Corrupt /
 * missing / invalid values all fall back to expanded (empty map) — which is
 * also what stale keys from an earlier grouping degrade to.
 */

const COLLAPSE_STORAGE_KEY = "agentProjectCollapse";

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
    // Keep only entries that are strictly true; anything else is treated as
    // corrupt and falls back to expanded.
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
    // Persistence failures (e.g. quota) do not affect in-memory state.
    console.error("Failed to persist agent project collapse state:", error);
  }
}

export const agentProjectCollapse = {
  /** Whether a group is collapsed (missing means expanded). */
  isCollapsed(id: string): boolean {
    return collapsed[id] === true;
  },

  /** Toggle a group's collapse state and persist. */
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

  /** Force-expand a group (used when opening a session auto-expands its group). */
  expand(id: string): void {
    if (collapsed[id] !== true) return;
    const next = { ...collapsed };
    delete next[id];
    collapsed = next;
    persistCollapse();
  },
};
