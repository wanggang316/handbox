/**
 * "Open in ..." — open a working directory in an external editor / terminal /
 * system file manager. Detection and launching happen in the backend
 * (commands/open_in.rs); the frontend only lists targets and passes an id back.
 */

import { apiCall } from "./index";

/** Mirrors the backend's `OpenInTarget`. */
export interface OpenInTarget {
  /** Stable id passed back to `openInTarget`; `"system"` = Finder / file manager. */
  id: string;
  name: string;
  kind: "editor" | "terminal" | "system";
  /** PNG data URL; null when unavailable (frontend falls back to a builtin icon). */
  icon?: string | null;
}

/** Lists installed editors/terminals plus the system file manager. */
export async function listOpenInTargets(): Promise<OpenInTarget[]> {
  return apiCall<OpenInTarget[]>("open_in_list_targets");
}

/** `path` must be an existing directory. */
export async function openInTarget(
  path: string,
  targetId: string,
): Promise<void> {
  return apiCall<void>("open_in_open", { path, targetId });
}
