/**
 * 窗口管理 API
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * 打开设置。设置页在主窗口内渲染：该命令聚焦主窗口并通知其导航，
 * 供划词等其他 webview 窗口调用；主窗口内部直接 goto("/settings/...") 即可。
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
