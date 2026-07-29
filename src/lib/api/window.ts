import { invoke } from '@tauri-apps/api/core';

/**
 * Settings render inside the main window: this command focuses it and tells it
 * to navigate, for use from other webview windows (e.g. selection panels).
 * From within the main window, just goto("/settings/...") directly.
 */
export async function openSettingsWindow(path?: string): Promise<void> {
  try {
    await invoke('open_settings_window', { path: path || null });
  } catch (error) {
    console.error('Failed to open settings window:', error);
    throw error;
  }
}
