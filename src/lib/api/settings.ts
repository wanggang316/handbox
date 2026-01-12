/**
 * 设置相关 API 封装
 */

import { apiCall } from './index';
import type { 
  AppSettings, 
  UpdateSettingsRequest,
  ExportSettingsOptions,
  ImportSettingsRequest,
  MCPServer 
} from '../types';

/**
 * 获取应用设置
 */
export async function getSettings(): Promise<AppSettings> {
  return apiCall<AppSettings>('settings_get');
}

/**
 * 更新设置
 */
export async function updateSettings(request: UpdateSettingsRequest): Promise<AppSettings> {
  return apiCall<AppSettings>('settings_update', { request });
}

/**
 * 重置设置到默认值
 */
export async function resetSettings(sections?: Array<keyof AppSettings>): Promise<AppSettings> {
  return apiCall<AppSettings>('settings_reset', { sections });
}

/**
 * 导出设置
 */
export async function exportSettings(options?: ExportSettingsOptions): Promise<string> {
  return apiCall<string>('settings_export', options);
}

/**
 * 导入设置
 */
export async function importSettings(request: ImportSettingsRequest): Promise<AppSettings> {
  return apiCall<AppSettings>('settings_import', { request });
}

/**
 * 验证 MCP 配置
 */
export async function validateMCPConfig(config: string): Promise<{ valid: boolean; errors?: string[] }> {
  return apiCall<{ valid: boolean; errors?: string[] }>('settings_validate_mcp', { config });
}

/**
 * 测试 MCP 服务器
 */
export async function testMCPServer(server: MCPServer): Promise<{ success: boolean; error?: string }> {
  return apiCall<{ success: boolean; error?: string }>('settings_test_mcp_server', { server });
}

/**
 * 获取系统信息
 */
export async function getSystemInfo(): Promise<{
  version: string;
  platform: string;
  arch: string;
  tauri_version: string;
}> {
  return apiCall<{
    version: string;
    platform: string;
    arch: string;
    tauri_version: string;
  }>('settings_system_info');
}

/**
 * 检查更新
 */
export async function checkForUpdates(): Promise<{
  hasUpdate: boolean;
  latestVersion?: string;
  releaseNotes?: string;
  downloadUrl?: string;
}> {
  return apiCall<{
    hasUpdate: boolean;
    latestVersion?: string;
    releaseNotes?: string;
    downloadUrl?: string;
  }>('settings_check_updates');
}
