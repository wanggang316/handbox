/**
 * search namespace strings (filled by migration subagent).
 */
import type { searchZh } from "../zh/search";

export const searchEn: Record<keyof typeof searchZh, string> = {
  "search.placeholder": "Search chat history...",
  "search.failed": "Search failed: {error}",
  "search.history": "Recent searches",
  "search.searching": "Searching...",
  "search.noResultsPrefix": "No chat history found for",
  "search.noResultsSuffix": "",
};
