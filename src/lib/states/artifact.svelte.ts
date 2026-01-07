/**
 * Artifact 相关状态管理 - Svelte 5
 */

import type { Artifact, ArtifactFilter, UUID } from "../types";
import * as artifactApi from "../api/artifact";

interface ArtifactStateData {
  artifacts: Artifact[];
  selectedArtifact: Artifact | null;
  isLoading: boolean;
  error: string | null;
  filter: ArtifactFilter;
}

class ArtifactState {
  private state = $state<ArtifactStateData>({
    artifacts: [],
    selectedArtifact: null,
    isLoading: false,
    error: null,
    filter: {},
  });

  // Getters
  get artifacts() {
    return this.state.artifacts;
  }

  get selectedArtifact() {
    return this.state.selectedArtifact;
  }

  get isLoading() {
    return this.state.isLoading;
  }

  get error() {
    return this.state.error;
  }

  get filter() {
    return this.state.filter;
  }

  // 派生状态：过滤后的 Artifact 列表
  get filteredArtifacts(): Artifact[] {
    let filtered = [...this.state.artifacts];
    const filter = this.state.filter;

    // 搜索过滤
    if (filter.search) {
      const searchLower = filter.search.toLowerCase();
      filtered = filtered.filter(
        (a) =>
          a.name.toLowerCase().includes(searchLower) ||
          (a.description && a.description.toLowerCase().includes(searchLower)),
      );
    }

    // 标签过滤
    if (filter.tags && filter.tags.length > 0) {
      filtered = filtered.filter(
        (a) => a.tags && a.tags.some((tag) => filter.tags!.includes(tag)),
      );
    }

    // 排序
    if (filter.sortBy) {
      filtered.sort((a, b) => {
        const ascending = filter.sortOrder === "asc";
        let comparison = 0;

        switch (filter.sortBy) {
          case "name":
            comparison = a.name.localeCompare(b.name);
            break;
          case "createdAt":
            comparison = a.createdAt - b.createdAt;
            break;
          case "lastRunAt":
            comparison = (a.lastRunAt || 0) - (b.lastRunAt || 0);
            break;
          case "runCount":
            comparison = a.runCount - b.runCount;
            break;
        }

        return ascending ? comparison : -comparison;
      });
    }

    return filtered;
  }

  // Actions
  setLoading(loading: boolean) {
    this.state.isLoading = loading;
  }

  setError(error: string | null) {
    this.state.error = error;
  }

  setArtifacts(artifacts: Artifact[]) {
    this.state.artifacts = artifacts;
  }

  addArtifact(artifact: Artifact) {
    this.state.artifacts.unshift(artifact);
  }

  updateArtifact(id: UUID, updates: Partial<Artifact>) {
    const index = this.state.artifacts.findIndex((a) => a.id === id);
    if (index !== -1) {
      this.state.artifacts[index] = {
        ...this.state.artifacts[index],
        ...updates,
      };
    }

    // 如果是当前选中的，也更新
    if (this.state.selectedArtifact && this.state.selectedArtifact.id === id) {
      this.state.selectedArtifact = {
        ...this.state.selectedArtifact,
        ...updates,
      };
    }
  }

  removeArtifact(id: UUID) {
    this.state.artifacts = this.state.artifacts.filter((a) => a.id !== id);

    // 如果是当前选中的，清空选择
    if (this.state.selectedArtifact && this.state.selectedArtifact.id === id) {
      this.state.selectedArtifact = null;
    }
  }

  selectArtifact(artifact: Artifact | null) {
    this.state.selectedArtifact = artifact;
  }

  updateFilter(newFilter: Partial<ArtifactFilter>) {
    this.state.filter = { ...this.state.filter, ...newFilter };
  }

  /**
   * 加载 Artifact 列表
   */
  async loadArtifacts(filter?: ArtifactFilter): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      const artifactList = await artifactApi.getArtifacts(filter);
      this.setArtifacts(artifactList);

      if (filter) {
        this.state.filter = filter;
      }
    } catch (error) {
      this.setError(
        error instanceof Error ? error.message : "加载 Artifact 列表失败",
      );
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 创建 Artifact
   */
  async createArtifact(request: any): Promise<Artifact> {
    try {
      this.setLoading(true);
      this.setError(null);

      const artifact = await artifactApi.createArtifact(request);

      this.addArtifact(artifact);
      return artifact;
    } catch (error) {
      this.setError(
        error instanceof Error ? error.message : "创建 Artifact 失败",
      );
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 更新 Artifact
   */
  async updateArtifactData(
    id: UUID,
    updates: Partial<Artifact>,
  ): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      const updatedArtifact = await artifactApi.updateArtifact({
        id,
        ...updates,
      });

      this.updateArtifact(id, updatedArtifact);
    } catch (error) {
      this.setError(
        error instanceof Error ? error.message : "更新 Artifact 失败",
      );
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 删除 Artifact
   */
  async deleteArtifact(id: UUID): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);

      await artifactApi.deleteArtifact(id);
      this.removeArtifact(id);
    } catch (error) {
      this.setError(
        error instanceof Error ? error.message : "删除 Artifact 失败",
      );
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 安装 Artifact
   */
  async installArtifact(request: any): Promise<Artifact> {
    try {
      this.setLoading(true);
      this.setError(null);

      const artifact = await artifactApi.installArtifact(request);

      // 更新列表中的 artifact
      const index = this.state.artifacts.findIndex((a) => a.id === artifact.id);
      if (index !== -1) {
        this.state.artifacts[index] = artifact;
      }

      return artifact;
    } catch (error) {
      this.setError(
        error instanceof Error ? error.message : "安装 Artifact 失败",
      );
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 执行 Artifact
   */
  async executeArtifact(request: any): Promise<any> {
    try {
      this.setError(null);

      const result = await artifactApi.executeArtifact(request);
      return result;
    } catch (error) {
      this.setError(
        error instanceof Error ? error.message : "执行 Artifact 失败",
      );
      throw error;
    }
  }

  /**
   * 初始化内置 Artifacts
   */
  async initBuiltinArtifacts(): Promise<Artifact[]> {
    try {
      const artifacts = await artifactApi.initBuiltinArtifacts();
      console.log(`Initialized ${artifacts.length} builtin artifacts`);
      return artifacts;
    } catch (error) {
      console.error('Failed to init builtin artifacts:', error);
      return [];
    }
  }

  /**
   * 清除错误状态
   */
  clearError(): void {
    this.setError(null);
  }

  /**
   * 重置状态
   */
  reset(): void {
    this.state.artifacts = [];
    this.state.selectedArtifact = null;
    this.state.filter = {};
    this.state.isLoading = false;
    this.state.error = null;
  }
}

// 导出单例实例
export const artifactState = new ArtifactState();
