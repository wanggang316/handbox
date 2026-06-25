/**
 * quickaction namespace strings (Quick Action overlay).
 */
import type { quickactionZh } from "../zh/quickaction";

export const quickactionEn: Record<keyof typeof quickactionZh, string> = {
  "quickaction.placeholder": "Type what you want to do…",
  "quickaction.searchPlaceholder": "Search agents…",
  "quickaction.messagePlaceholder": "Message {name}…",
  "quickaction.send": "Send",
  "quickaction.continueInChat": "Continue in Chat",
  "quickaction.stop": "Stop",
  "quickaction.newClear": "New",
  "quickaction.select": "Select",
  "quickaction.navigate": "Navigate",
  "quickaction.runFailed": "Failed to send, please try again.",
  "quickaction.sessionName": "Quick Action",
  "quickaction.noAgents.title": "No agents yet",
  "quickaction.noAgents.description":
    "Create one under Agents in the app to chat here.",
  "quickaction.noMatch": "No matching agents",
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

  // Settings page · default-model selector
  "quickaction.model.title": "Default model",
  "quickaction.model.label": "Default model",
  "quickaction.model.hint":
    "The model used by default when you summon the Quick Action overlay; you can switch it on the fly inside the overlay.",
  "quickaction.model.none": "Not selected",
  "quickaction.model.unavailable":
    "The selected model is no longer available, please re-select",
  "quickaction.model.emptyCatalog":
    "Enable a provider and add a model under Models first.",
  "quickaction.model.openModels": "Open model settings",
};
