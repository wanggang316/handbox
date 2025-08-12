/**
 * Artifact 相关状态管理
 */

import { writable, derived } from 'svelte/store';
import type { Artifact, ArtifactFilter, ArtifactStats, UUID } from '../types';
import * as artifactApi from '../api/artifact';

// Artifact 列表
export const artifacts = writable<Artifact[]>([]);

// 当前选中的 Artifact
export const selectedArtifact = writable<Artifact | null>(null);

// 加载状态
export const isLoading = writable(false);

// 错误状态
export const artifactError = writable<string | null>(null);

// 搜索过滤器
export const artifactFilter = writable<ArtifactFilter>({});

// 统计信息
export const artifactStats = writable<ArtifactStats | null>(null);

// 派生状态：过滤后的 Artifact 列表
export const filteredArtifacts = derived(
  [artifacts, artifactFilter],
  ([$artifacts, $filter]) => {
    let filtered = [...$artifacts];
    
    // 搜索过滤
    if ($filter.search) {
      const searchLower = $filter.search.toLowerCase();
      filtered = filtered.filter(a => 
        a.name.toLowerCase().includes(searchLower) ||
        (a.description && a.description.toLowerCase().includes(searchLower))
      );
    }
    
    // 标签过滤
    if ($filter.tags && $filter.tags.length > 0) {
      filtered = filtered.filter(a => 
        a.tags && a.tags.some(tag => $filter.tags!.includes(tag))
      );
    }
    
    // 排序
    if ($filter.sortBy) {
      filtered.sort((a, b) => {
        const ascending = $filter.sortOrder === 'asc';
        let comparison = 0;
        
        switch ($filter.sortBy) {
          case 'name':
            comparison = a.name.localeCompare(b.name);
            break;
          case 'createdAt':
            comparison = a.createdAt - b.createdAt;
            break;
          case 'lastUsedAt':
            comparison = (a.lastUsedAt || 0) - (b.lastUsedAt || 0);
            break;
          case 'useCount':
            comparison = a.useCount - b.useCount;
            break;
        }
        
        return ascending ? comparison : -comparison;
      });
    }
    
    return filtered;
  }
);

/**
 * Artifact 操作
 */
export const artifactActions = {
  /**
   * 加载 Artifact 列表
   */
  async loadArtifacts(filter?: ArtifactFilter): Promise<void> {
    try {
      isLoading.set(true);
      const artifactList = await artifactApi.getArtifacts(filter);
      artifacts.set(artifactList);
      
      if (filter) {
        artifactFilter.set(filter);
      }
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '加载 Artifact 列表失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 创建 Artifact
   */
  async createArtifact(name: string, description?: string, config?: any, tags?: string[]): Promise<Artifact> {
    try {
      isLoading.set(true);
      const artifact = await artifactApi.createArtifact({
        name,
        description,
        config: config || {},
        tags
      });
      
      // 添加到列表
      artifacts.update(list => [artifact, ...list]);
      
      return artifact;
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '创建 Artifact 失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 更新 Artifact
   */
  async updateArtifact(id: UUID, updates: Partial<Artifact>): Promise<void> {
    try {
      isLoading.set(true);
      const updatedArtifact = await artifactApi.updateArtifact({
        id,
        ...updates
      });
      
      // 更新列表中的 Artifact
      artifacts.update(list =>
        list.map(a => a.id === id ? updatedArtifact : a)
      );
      
      // 如果是当前选中的，也更新
      selectedArtifact.update(current => 
        current && current.id === id ? updatedArtifact : current
      );
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '更新 Artifact 失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 删除 Artifact
   */
  async deleteArtifact(id: UUID): Promise<void> {
    try {
      isLoading.set(true);
      await artifactApi.deleteArtifact(id);
      
      // 从列表中移除
      artifacts.update(list => list.filter(a => a.id !== id));
      
      // 如果是当前选中的，清空选择
      selectedArtifact.update(current => 
        current && current.id === id ? null : current
      );
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '删除 Artifact 失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 使用 Artifact 创建新会话
   */
  async useArtifact(artifactId: UUID, sessionName?: string): Promise<any> {
    try {
      const session = await artifactApi.useArtifact({ artifactId, sessionName });
      
      // 更新使用统计
      artifacts.update(list =>
        list.map(a => 
          a.id === artifactId 
            ? { ...a, useCount: a.useCount + 1, lastUsedAt: Date.now() }
            : a
        )
      );
      
      return session;
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '使用 Artifact 失败');
      throw error;
    }
  },

  /**
   * 加载统计信息
   */
  async loadStats(): Promise<void> {
    try {
      const stats = await artifactApi.getArtifactStats();
      artifactStats.set(stats);
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '加载统计信息失败');
      throw error;
    }
  },

  /**
   * 选择 Artifact
   */
  selectArtifact(artifact: Artifact | null): void {
    selectedArtifact.set(artifact);
  },

  /**
   * 搜索 Artifact
   */
  async searchArtifacts(query: string): Promise<void> {
    try {
      isLoading.set(true);
      const results = await artifactApi.searchArtifacts(query);
      artifacts.set(results);
    } catch (error) {
      artifactError.set(error instanceof Error ? error.message : '搜索 Artifact 失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 更新过滤器
   */
  updateFilter(newFilter: Partial<ArtifactFilter>): void {
    artifactFilter.update(current => ({ ...current, ...newFilter }));
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    artifactError.set(null);
  },

  /**
   * 重置状态
   */
  reset(): void {
    artifacts.set([]);
    selectedArtifact.set(null);
    artifactFilter.set({});
    artifactStats.set(null);
    isLoading.set(false);
    artifactError.set(null);
  }
};