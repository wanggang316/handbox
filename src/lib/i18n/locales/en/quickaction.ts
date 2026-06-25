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

  // Settings page · shortcut recorder
  "quickaction.shortcut.title": "Summon Quick Action",
  "quickaction.shortcut.label": "Global hotkey",
  "quickaction.shortcut.hint":
    "Press this combination in any app to summon the Quick Action overlay.",
  "quickaction.shortcut.recording": "Press a shortcut…",
  "quickaction.shortcut.record": "Record",
  "quickaction.shortcut.reset": "Reset to default",
  "quickaction.shortcut.invalid.modifierOnly":
    "Include at least one regular key, e.g. ⌘⇧Space.",
  "quickaction.shortcut.invalid.noModifier":
    "Include at least one modifier key (⌘ / ⌃ / ⌥ / ⇧).",
  "quickaction.shortcut.invalid.unsupportedKey":
    "That key is not supported, please pick another combination.",
  "quickaction.shortcut.registerFailed":
    "Failed to register the shortcut, please try another combination.",
};
