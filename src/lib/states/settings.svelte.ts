/**
 * Settings state - Svelte 5.
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

  /** Load settings (skipped if already loaded, unless forced). */
  async loadSettings(forceReload = false): Promise<void> {
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

  /** Partially update settings in memory only (no persistence). */
  updateLocalSettings(updates: Partial<AppSettings>): void {
    if (this.state.settings) {
      this.state.settings = { ...this.state.settings, ...updates };
    }
  }

  clearError(): void {
    this.setError(null);
  }

  reset(): void {
    this.state.settings = null;
    this.state.isLoading = false;
    this.state.error = null;
  }
}

export const settingsState = new SettingsState();