import { emit } from "@tauri-apps/api/event";
import { apiCall } from "./index";

export type ContentPanelMode = "show" | "translate" | "ai";

export interface SelectionAppInfo {
  name: string;
  bundle_id: string;
  pid: number;
}

export interface SelectionPayload {
  text: string;
  x: number;
  y: number;
  app_info: SelectionAppInfo;
}

export async function hideMenuPanel(): Promise<void> {
  return apiCall<void>("selection_hide_menu_panel");
}

export async function showContentPanel(
  mode: ContentPanelMode,
  payload: SelectionPayload,
): Promise<void> {
  return apiCall<void>("selection_show_content_panel", { mode, payload });
}

export async function hideContentPanel(): Promise<void> {
  return apiCall<void>("selection_hide_content_panel");
}

export async function setContentPanelPinned(pinned: boolean): Promise<void> {
  return apiCall<void>("selection_set_content_pinned", { pinned });
}

export async function getContentPanelPinned(): Promise<boolean> {
  return apiCall<boolean>("selection_get_content_pinned");
}

export async function showSettingsPanel(x: number, y: number): Promise<void> {
  return apiCall<void>("selection_show_settings_panel", { x, y });
}

export async function hideSettingsPanel(): Promise<void> {
  return apiCall<void>("selection_hide_settings_panel");
}

export async function disableCurrentAppByPid(): Promise<void> {
  return apiCall<void>("selection_disable_current_app_by_pid");
}

export async function disableCurrentAppByBundleId(): Promise<void> {
  return apiCall<void>("selection_disable_current_app_by_bundle_id");
}

export async function disableGlobalSelection(): Promise<void> {
  return apiCall<void>("selection_disable_global");
}

export interface DisabledApp {
  bundle_id: string;
  name: string;
}

export async function getDisabledApps(): Promise<DisabledApp[]> {
  return apiCall<DisabledApp[]>("selection_get_disabled_apps");
}

export async function removeDisabledApp(bundleId: string): Promise<void> {
  return apiCall<void>("selection_remove_disabled_app", { bundleId });
}
