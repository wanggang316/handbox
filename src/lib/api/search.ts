/**
 * 搜索相关 API 封装
 */

import { apiCall } from './index';
import type { UUID } from '../types';

// 搜索结果类型
export interface SearchResult {
  id: UUID;
  type: 'message' | 'session' | 'artifact';
  title: string;
  content: string;
  snippet: string;
  sessionId?: UUID;
  messageId?: UUID;
  artifactId?: UUID;
  score: number;
  timestamp: number;
  highlights: Array<{ start: number; end: number }>;
}

// 搜索请求
export interface SearchRequest {
  query: string;
  types?: Array<'message' | 'session' | 'artifact'>;
  sessionId?: UUID;
  limit?: number;
  offset?: number;
  sortBy?: 'relevance' | 'timestamp';
  sortOrder?: 'asc' | 'desc';
}

// 搜索响应
export interface SearchResponse {
  results: SearchResult[];
  total: number;
  query: string;
  took: number;
}

/**
 * 搜索消息和会话
 */
export async function search(request: SearchRequest): Promise<SearchResponse> {
  return apiCall<SearchResponse>('search_query', request);
}

/**
 * 获取搜索建议
 */
export async function getSearchSuggestions(
  query: string,
  limit?: number
): Promise<string[]> {
  return apiCall<string[]>('search_suggestions', { query, limit });
}

/**
 * 获取搜索历史
 */
export async function getSearchHistory(limit?: number): Promise<string[]> {
  return apiCall<string[]>('search_history', { limit });
}

/**
 * 添加搜索历史
 */
export async function addSearchHistory(query: string): Promise<void> {
  return apiCall<void>('search_add_history', { query });
}

/**
 * 清空搜索历史
 */
export async function clearSearchHistory(): Promise<void> {
  return apiCall<void>('search_clear_history');
}

/**
 * 重建搜索索引
 */
export async function rebuildSearchIndex(): Promise<{ 
  indexed: number; 
  took: number 
}> {
  return apiCall<{ 
    indexed: number; 
    took: number 
  }>('search_rebuild_index');
}

/**
 * 获取索引统计
 */
export async function getIndexStats(): Promise<{
  totalDocuments: number;
  totalTerms: number;
  indexSize: number;
  lastUpdated: number;
}> {
  return apiCall<{
    totalDocuments: number;
    totalTerms: number;
    indexSize: number;
    lastUpdated: number;
  }>('search_index_stats');
}