/**
 * GenUI state - Svelte 5 runes.
 */

import type { GenUi, UUID } from "../types";
import * as genuiApi from "../api/genui";

export const genuiState = $state({
  genuis: [] as GenUi[],

  isLoading: false,

  error: null as string | null,
});

export const genuiActions = {
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

  async getGenui(genuiId: UUID): Promise<GenUi> {
    return genuiApi.getGenui(genuiId);
  },

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

  clearError(): void {
    genuiState.error = null;
  },
};
