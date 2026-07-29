import { apiCall } from "./index";

/** Silent check; never shows the system permission prompt. */
export async function checkAccessibilityPermission(): Promise<boolean> {
  return apiCall<boolean>("accessibility_check_permission");
}

/** Shows the system permission prompt when not yet granted. */
export async function requestAccessibilityPermission(): Promise<boolean> {
  return apiCall<boolean>("accessibility_request_permission");
}

export async function openAccessibilitySettings(): Promise<void> {
  return apiCall<void>("accessibility_open_settings");
}
