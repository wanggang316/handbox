/**
 * 搜索相关状态管理
 */

import { writable, derived } from 'svelte/store';
import type { SearchRequest, SearchResponse, SearchResult } from '../api/search';
import * as searchApi from '../api/search';

// 搜索查询
export const searchQuery = writable<string>('');

// 搜索结果
export const searchResults = writable<SearchResult[]>([]);

// 搜索历史
export const searchHistory = writable<string[]>([]);

// 搜索建议
export const searchSuggestions = writable<string[]>([]);

// 加载状态
export const searchLoading = writable(false);

// 错误状态
export const searchError = writable<string | null>(null);

// 派生状态：是否有搜索结果
export const hasSearchResults = derived(
  searchResults,
  ($results) => $results.length > 0
);

/**
 * 搜索操作
 */
export const searchActions = {
  /**
   * 执行搜索
   */
  async search(request: SearchRequest): Promise<void> {
    try {
      searchLoading.set(true);
      const response = await searchApi.search(request);
      searchResults.set(response.results);
      
      // 添加到搜索历史
      if (request.query && request.query.trim()) {
        await searchApi.addSearchHistory(request.query);
        searchHistory.update(history => [
          request.query,
          ...history.filter(q => q !== request.query)
        ].slice(0, 10));
      }
    } catch (error) {
      searchError.set(error instanceof Error ? error.message : '搜索失败');
      throw error;
    } finally {
      searchLoading.set(false);
    }
  },

  /**
   * 获取搜索建议
   */
  async getSuggestions(query: string): Promise<void> {
    if (!query.trim()) {
      searchSuggestions.set([]);
      return;
    }

    try {
      const suggestions = await searchApi.getSearchSuggestions(query, 5);
      searchSuggestions.set(suggestions);
    } catch (error) {
      console.warn('获取搜索建议失败:', error);
      searchSuggestions.set([]);
    }
  },

  /**
   * 加载搜索历史
   */
  async loadSearchHistory(): Promise<void> {
    try {
      const history = await searchApi.getSearchHistory(10);
      searchHistory.set(history);
    } catch (error) {
      console.warn('加载搜索历史失败:', error);
    }
  },

  /**
   * 清空搜索历史
   */
  async clearSearchHistory(): Promise<void> {
    try {
      await searchApi.clearSearchHistory();
      searchHistory.set([]);
    } catch (error) {
      searchError.set(error instanceof Error ? error.message : '清空搜索历史失败');
      throw error;
    }
  },

  /**
   * 清除搜索结果
   */
  clearResults(): void {
    searchResults.set([]);
    searchQuery.set('');
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    searchError.set(null);
  }
};