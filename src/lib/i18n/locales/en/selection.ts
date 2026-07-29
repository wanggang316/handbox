/**
 * selection namespace strings.
 */
import type { selectionZh } from "../zh/selection";

export const selectionEn: Record<keyof typeof selectionZh, string> = {
  // settings panel
  "selection.hideUntilRestart": "Hide until this app restarts",
  "selection.disableForApp": "Disable for this app",
  "selection.disableGlobal": "Disable globally",
  // modes
  "selection.modeShow": "Show",
  "selection.modeTranslate": "Translate",
  "selection.modeAi": "Ask",
  // content panel
  "selection.unpin": "Unpin",
  "selection.pin": "Pin",
  "selection.translating": "Translating...",
  "selection.waitingTranslation": "Waiting for translation...",
  "selection.noContent": "No content",
  "selection.retranslate": "Retranslate",
  "selection.regenerate": "Regenerate",
  "selection.continueAsk": "Continue asking",
  "selection.translationConfigHint": "Configure a default model in Settings → Shortcuts first",
  "selection.translationFailed": "Translation failed",
};
