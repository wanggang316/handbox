/**
 * 划词选择 API
 *
 * 提供划词菜单面板和内容面板的管理功能
 */

import { emit } from "@tauri-apps/api/event";
import { apiCall } from "./index";

/** 内容面板模式 */
export type ContentPanelMode = "show" | "translate" | "ai";

/** 应用信息 */
export interface SelectionAppInfo {
  name: string;
  bundle_id: string;
  pid: number;
}

/** 选中内容的 payload */
export interface SelectionPayload {
  text: string;
  x: number;
  y: number;
  app_info: SelectionAppInfo;
}

/**
 * 隐藏菜单面板
 */
export async function hideMenuPanel(): Promise<void> {
  return apiCall<void>("selection_hide_menu_panel");
}

/**
 * 显示内容面板
 * @param mode 面板模式
 * @param payload 选中内容信息
 */
export async function showContentPanel(
  mode: ContentPanelMode,
  payload: SelectionPayload,
): Promise<void> {
  return apiCall<void>("selection_show_content_panel", { mode, payload });
}

/**
 * 隐藏内容面板
 */
export async function hideContentPanel(): Promise<void> {
  return apiCall<void>("selection_hide_content_panel");
}

/**
 * 设置内容面板置顶状态
 * @param pinned 是否置顶
 */
export async function setContentPanelPinned(pinned: boolean): Promise<void> {
  return apiCall<void>("selection_set_content_pinned", { pinned });
}

/**
 * 获取内容面板置顶状态
 */
export async function getContentPanelPinned(): Promise<boolean> {
  return apiCall<boolean>("selection_get_content_pinned");
}

/**
 * 显示设置面板
 */
export async function showSettingsPanel(x: number, y: number): Promise<void> {
  return apiCall<void>("selection_show_settings_panel", { x, y });
}

/**
 * 隐藏设置面板
 */
export async function hideSettingsPanel(): Promise<void> {
  return apiCall<void>("selection_hide_settings_panel");
}

/**
 * 显示禁用二级面板
 */
export async function showSettingsDisablePanel(
  x: number,
  y: number,
): Promise<void> {
  return apiCall<void>("selection_show_settings_disable_panel", {
    x,
    y,
  });
}

/**
 * 当前应用（pid）禁用
 */
export async function disableCurrentAppByPid(): Promise<void> {
  return apiCall<void>("selection_disable_current_app_by_pid");
}

/**
 * 当前应用（bundleId）禁用
 */
export async function disableCurrentAppByBundleId(): Promise<void> {
  return apiCall<void>("selection_disable_current_app_by_bundle_id");
}

/**
 * 全局禁用
 */
export async function disableGlobalSelection(): Promise<void> {
  return apiCall<void>("selection_disable_global");
}

/**
 * 隐藏禁用二级面板
 */
export async function hideSettingsDisablePanel(): Promise<void> {
  return apiCall<void>("selection_hide_settings_disable_panel");
}
