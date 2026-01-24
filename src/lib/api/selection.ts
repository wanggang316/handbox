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
