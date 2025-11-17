/**
 * 窗口管理 API
 * 
 * 提供窗口打开、关闭、切换等功能
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * 打开设置窗口
 * @param path 可选的路径参数，例如 '/mcp' 或 '/mcp/server-id'
 */
export async function openSettingsWindow(path?: string): Promise<void> {
  try {
    await invoke('open_settings_window', { path: path || null });
  } catch (error) {
    console.error('Failed to open settings window:', error);
    throw error;
  }
}

/**
 * 关闭设置窗口
 */
export async function closeSettingsWindow(): Promise<void> {
  try {
    await invoke('close_settings_window');
  } catch (error) {
    console.error('Failed to close settings window:', error);
    throw error;
  }
}

/**
 * 切换设置窗口显示状态
 */
export async function toggleSettingsWindow(): Promise<void> {
  try {
    await invoke('toggle_settings_window');
  } catch (error) {
    console.error('Failed to toggle settings window:', error);
    throw error;
  }
}
