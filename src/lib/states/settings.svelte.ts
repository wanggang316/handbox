/**
 * 设置相关状态管理 - Svelte 5
 */

import type { AppSettings, UpdateSettingsRequest } from '../types';
import * as settingsApi from '../api/settings';

interface SettingsStateData {
  settings: AppSettings | null;
  isLoading: boolean;
  error: string | null;
}

class SettingsState {
  private state = $state<SettingsStateData>({
    settings: null,
    isLoading: false,
    error: null,
  });

  // Getters
  get settings() {
    return this.state.settings;
  }

  get isLoading() {
    return this.state.isLoading;
  }

  get error() {
    return this.state.error;
  }

  // Actions
  setLoading(loading: boolean) {
    this.state.isLoading = loading;
  }

  setError(error: string | null) {
    this.state.error = error;
  }

  setSettings(settings: AppSettings | null) {
    this.state.settings = settings;
  }

  /**
   * 加载设置（如果已加载则跳过）
   */
  async loadSettings(forceReload = false): Promise<void> {
    // 如果已经加载过且不强制重新加载，直接返回
    if (!forceReload && this.state.settings) {
      return;
    }

    try {
      this.setLoading(true);
      this.setError(null);

      const settings = await settingsApi.getSettings();
      this.setSettings(settings);
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '加载设置失败');
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 更新设置
   */
  async updateSettings(request: UpdateSettingsRequest): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);
      
      const updatedSettings = await settingsApi.updateSettings(request);
      this.setSettings(updatedSettings);
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '更新设置失败');
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 部分更新设置
   */
  updateLocalSettings(updates: Partial<AppSettings>): void {
    if (this.state.settings) {
      this.state.settings = { ...this.state.settings, ...updates };
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
    this.state.settings = null;
    this.state.isLoading = false;
    this.state.error = null;
  }
}

// 导出单例实例
export const settingsState = new SettingsState();