/**
 * 设置相关状态管理
 */

import { writable } from 'svelte/store';
import type { AppSettings, UpdateSettingsRequest } from '../types';
import * as settingsApi from '../api/settings';

// 应用设置
export const appSettings = writable<AppSettings | null>(null);

// 加载状态
export const settingsLoading = writable(false);

// 错误状态
export const settingsError = writable<string | null>(null);

/**
 * 设置操作
 */
export const settingsActions = {
  /**
   * 加载设置
   */
  async loadSettings(): Promise<void> {
    try {
      settingsLoading.set(true);
      const settings = await settingsApi.getSettings();
      appSettings.set(settings);
    } catch (error) {
      settingsError.set(error instanceof Error ? error.message : '加载设置失败');
      throw error;
    } finally {
      settingsLoading.set(false);
    }
  },

  /**
   * 更新设置
   */
  async updateSettings(request: UpdateSettingsRequest): Promise<void> {
    try {
      settingsLoading.set(true);
      const updatedSettings = await settingsApi.updateSettings(request);
      appSettings.set(updatedSettings);
    } catch (error) {
      settingsError.set(error instanceof Error ? error.message : '更新设置失败');
      throw error;
    } finally {
      settingsLoading.set(false);
    }
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    settingsError.set(null);
  },

  /**
   * 重置状态
   */
  reset(): void {
    appSettings.set(null);
    settingsLoading.set(false);
    settingsError.set(null);
  }
};