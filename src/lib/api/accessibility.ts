/**
 * 辅助功能权限 API
 */

import { apiCall } from './index';

/**
 * 检查辅助功能权限是否已授予（静默检查，不显示系统弹窗）
 */
export async function checkAccessibilityPermission(): Promise<boolean> {
  return apiCall<boolean>('accessibility_check_permission');
}

/**
 * 请求辅助功能权限（如未授予则显示系统弹窗引导用户开启）
 */
export async function requestAccessibilityPermission(): Promise<boolean> {
  return apiCall<boolean>('accessibility_request_permission');
}

/**
 * 打开系统辅助功能设置页面
 */
export async function openAccessibilitySettings(): Promise<void> {
  return apiCall<void>('accessibility_open_settings');
}
