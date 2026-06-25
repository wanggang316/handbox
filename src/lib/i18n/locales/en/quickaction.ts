/**
 * quickaction namespace strings (Quick Action overlay).
 */
import type { quickactionZh } from "../zh/quickaction";

export const quickactionEn: Record<keyof typeof quickactionZh, string> = {
  "quickaction.placeholder": "Type what you want to do…",
  "quickaction.send": "Send",
  "quickaction.stop": "Stop",
  "quickaction.newClear": "New",
  "quickaction.runFailed": "Failed to start, please try again.",
  "quickaction.sessionName": "Quick Action",
  "quickaction.noModel.title": "No usable model configured",
  "quickaction.noModel.description":
    "Enable a provider and pick a default model in Settings to get started.",
  "quickaction.noModel.openSettings": "Open Settings",
};
