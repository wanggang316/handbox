/**
 * update namespace strings (filled by migration subagent).
 */
import type { updateZh } from "../zh/update";

export const updateEn: Record<keyof typeof updateZh, string> = {
  "update.newVersionFound": "New version available",
  "update.downloading": "Downloading update…",
  "update.remindLater": "Remind me later",
  "update.updating": "Updating…",
  "update.updateNow": "Update now",
};
