/**
 * 浏览器相关工具函数
 */

import { openUrl } from '@tauri-apps/plugin-opener';

/**
 * 在系统默认浏览器中打开URL
 * @param url 要打开的URL
 */
export async function openInBrowser(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    console.error('Failed to open URL in browser:', error);
    throw error;
  }
}