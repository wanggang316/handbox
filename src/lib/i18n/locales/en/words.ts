/**
 * words namespace strings (filled by migration subagent).
 */
import type { wordsZh } from "../zh/words";

export const wordsEn: Record<keyof typeof wordsZh, string> = {
  "words.title": "Words",
  "words.tab.lookup": "Lookup",
  "words.tab.learn": "Learn",
  "words.listSearchPlaceholder": "Search words or definitions",
  "words.lookupPlaceholder": "Enter a word, phrase, or sentence",
  "words.translationAgent": "Translation Agent",
  "words.querying": "Querying...",
  "words.query": "Query",
  "words.noAgentHint": "No agents available. Create a translation agent on the Agent management page first.",
  "words.selectAgentHint": "Select a translation agent",
  "words.selectModelHint": "Select a translation model",
  "words.history": "Recent lookups",
  "words.removeFromWordbook": "Remove from words",
  "words.addToWordbook": "Add to words",
  "words.emptyList": "No words yet",
  "words.back": "Back to words",
  "words.notFound": "Word not found",
  "words.explanation": "Brief explanation",
  "words.noExplanation": "No explanation",
  "words.note": "Note",
  "words.noNote": "No note",
  // error messages
  "words.error.createSessionFailed": "Failed to create translation session",
  "words.error.loadWordsFailed": "Failed to load words",
  "words.error.configRequired": "Please configure a translation agent and model first",
  "words.error.translateFailed": "Translation failed",
  "words.error.lookupFailed": "Lookup failed",
  "words.error.addWordFailed": "Failed to add word",
  "words.error.deleteWordFailed": "Failed to delete word",
  "words.error.deleteHistoryFailed": "Failed to delete history",
  "words.error.removeWordFailed": "Failed to remove word",
  "words.error.saveConfigFailed": "Failed to save config",
  "words.error.invalidId": "Invalid word ID",
  "words.error.loadDetailFailed": "Failed to load details",
};
