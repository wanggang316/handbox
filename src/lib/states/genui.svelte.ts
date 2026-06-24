/**
 * GenUI 相关状态管理 - 使用 Svelte 5 runes
 */

import type { GenUi, UUID } from "../types";
import * as genuiApi from "../api/genui";

// 全局状态对象
export const genuiState = $state({
  // GenUI 列表
  genuis: [] as GenUi[],

  // 加载状态
  isLoading: false,

  // 错误状态
  error: null as string | null,
});

/**
 * GenUI 操作
 */
export const genuiActions = {
  /**
   * 加载 GenUI 列表
   */
  async loadGenuis(): Promise<void> {
    try {
      genuiState.isLoading = true;
      genuiState.error = null;
      genuiState.genuis = await genuiApi.getGenuis();
    } catch (error) {
      genuiState.error =
        error instanceof Error ? error.message : "加载 GenUI 列表失败";
      throw error;
    } finally {
      genuiState.isLoading = false;
    }
  },

  /**
   * 获取单个 GenUI
   */
  async getGenui(genuiId: UUID): Promise<GenUi> {
    return genuiApi.getGenui(genuiId);
  },

  /**
   * 创建 GenUI
   */
  async createGenui(name: string, spec: string): Promise<GenUi> {
    try {
      genuiState.error = null;
      const genui = await genuiApi.createGenui(name, spec);
      genuiState.genuis.unshift(genui);
      return genui;
    } catch (error) {
      genuiState.error =
        error instanceof Error ? error.message : "创建 GenUI 失败";
      throw error;
    }
  },

  /**
   * 更新 GenUI（名称 / spec）
   */
  async updateGenui(
    genuiId: UUID,
    name?: string,
    spec?: string,
  ): Promise<GenUi> {
    try {
      genuiState.error = null;
      const updated = await genuiApi.updateGenui(genuiId, name, spec);

      const index = genuiState.genuis.findIndex((g) => g.id === genuiId);
      if (index !== -1) {
        genuiState.genuis[index] = updated;
      }

      return updated;
    } catch (error) {
      genuiState.error =
        error instanceof Error ? error.message : "更新 GenUI 失败";
      throw error;
    }
  },

  /**
   * 删除 GenUI
   */
  async deleteGenui(genuiId: UUID): Promise<void> {
    try {
      genuiState.error = null;
      await genuiApi.deleteGenui(genuiId);
      genuiState.genuis = genuiState.genuis.filter((g) => g.id !== genuiId);
    } catch (error) {
      genuiState.error =
        error instanceof Error ? error.message : "删除 GenUI 失败";
      throw error;
    }
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    genuiState.error = null;
  },
};
