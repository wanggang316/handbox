/**
 * 搜索相关状态管理 - Svelte 5
 */

import type { SearchRequest, SearchResponse, SearchResult } from '../api/search';
import * as searchApi from '../api/search';

interface SearchStateData {
  query: string;
  results: SearchResult[];
  history: string[];
  suggestions: string[];
  isLoading: boolean;
  error: string | null;
}

class SearchState {
  private state = $state<SearchStateData>({
    query: '',
    results: [],
    history: [],
    suggestions: [],
    isLoading: false,
    error: null,
  });

  // Getters
  get query() {
    return this.state.query;
  }

  get results() {
    return this.state.results;
  }

  get history() {
    return this.state.history;
  }

  get suggestions() {
    return this.state.suggestions;
  }

  get isLoading() {
    return this.state.isLoading;
  }

  get error() {
    return this.state.error;
  }

  // 派生状态：是否有搜索结果
  get hasResults(): boolean {
    return this.state.results.length > 0;
  }

  // Actions
  setQuery(query: string) {
    this.state.query = query;
  }

  setResults(results: SearchResult[]) {
    this.state.results = results;
  }

  setHistory(history: string[]) {
    this.state.history = history;
  }

  setSuggestions(suggestions: string[]) {
    this.state.suggestions = suggestions;
  }

  setLoading(loading: boolean) {
    this.state.isLoading = loading;
  }

  setError(error: string | null) {
    this.state.error = error;
  }

  addToHistory(query: string) {
    if (!query.trim()) return;
    
    this.state.history = [
      query,
      ...this.state.history.filter(q => q !== query)
    ].slice(0, 10);
  }

  /**
   * 执行搜索
   */
  async search(request: SearchRequest): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);
      
      const response = await searchApi.search(request);
      this.setResults(response.results);
      this.setQuery(request.query || '');
      
      // 添加到搜索历史
      if (request.query && request.query.trim()) {
        await searchApi.addSearchHistory(request.query);
        this.addToHistory(request.query);
      }
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '搜索失败');
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 获取搜索建议
   */
  async getSuggestions(query: string): Promise<void> {
    if (!query.trim()) {
      this.setSuggestions([]);
      return;
    }

    try {
      const suggestions = await searchApi.getSearchSuggestions(query, 5);
      this.setSuggestions(suggestions);
    } catch (error) {
      console.warn('获取搜索建议失败:', error);
      this.setSuggestions([]);
    }
  }

  /**
   * 加载搜索历史
   */
  async loadHistory(): Promise<void> {
    try {
      const history = await searchApi.getSearchHistory(10);
      this.setHistory(history);
    } catch (error) {
      console.warn('加载搜索历史失败:', error);
    }
  }

  /**
   * 清空搜索历史
   */
  async clearHistory(): Promise<void> {
    try {
      await searchApi.clearSearchHistory();
      this.setHistory([]);
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '清空搜索历史失败');
      throw error;
    }
  }

  /**
   * 清除搜索结果
   */
  clearResults(): void {
    this.setResults([]);
    this.setQuery('');
  }

  /**
   * 清除错误状态
   */
  clearError(): void {
    this.setError(null);
  }

  /**
   * 重置所有状态
   */
  reset(): void {
    this.state.query = '';
    this.state.results = [];
    this.state.history = [];
    this.state.suggestions = [];
    this.state.isLoading = false;
    this.state.error = null;
  }
}

// 导出单例实例
export const searchState = new SearchState();