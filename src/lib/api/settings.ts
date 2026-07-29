import { apiCall } from "./index";
import type {
  AppSettings,
  UpdateSettingsRequest,
  ExportSettingsOptions,
  ImportSettingsRequest,
  MCPServer,
} from "../types";

export async function getSettings(): Promise<AppSettings> {
  return apiCall<AppSettings>("settings_get");
}

export async function updateSettings(
  request: UpdateSettingsRequest,
): Promise<AppSettings> {
  return apiCall<AppSettings>("settings_update", { request });
}

export async function resetSettings(
  sections?: Array<keyof AppSettings>,
): Promise<AppSettings> {
  return apiCall<AppSettings>("settings_reset", { sections });
}

export async function exportSettings(
  options?: ExportSettingsOptions,
): Promise<string> {
  return apiCall<string>("settings_export", options);
}

export async function importSettings(
  request: ImportSettingsRequest,
): Promise<AppSettings> {
  return apiCall<AppSettings>("settings_import", { request });
}

export async function validateMCPConfig(
  config: string,
): Promise<{ valid: boolean; errors?: string[] }> {
  return apiCall<{ valid: boolean; errors?: string[] }>(
    "settings_validate_mcp",
    { config },
  );
}

export async function testMCPServer(
  server: MCPServer,
): Promise<{ success: boolean; error?: string }> {
  return apiCall<{ success: boolean; error?: string }>(
    "settings_test_mcp_server",
    { server },
  );
}

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
  }>("settings_system_info");
}

export async function checkForUpdates(): Promise<{
  hasUpdate: boolean;
  latestVersion?: string;
  releaseNotes?: string;
}> {
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) return { hasUpdate: false };
  return {
    hasUpdate: true,
    latestVersion: update.version,
    releaseNotes: update.body,
  };
}
