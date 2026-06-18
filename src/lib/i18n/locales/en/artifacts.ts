/**
 * artifacts namespace strings (filled by migration subagent).
 */
import type { artifactsZh } from "../zh/artifacts";

export const artifactsEn: Record<keyof typeof artifactsZh, string> = {
  "artifacts.searchPlaceholder": "Search apps...",
  "artifacts.clearSearch": "Clear search",
  "artifacts.typeAll": "All",
  "artifacts.emptyList": "No apps found",
  "artifacts.installed": "Installed",
  "artifacts.install": "Install",
  "artifacts.run": "Run",
  "artifacts.deleteConfirm": 'Delete "{name}"?',
  "artifacts.entryFile": "Entry file:",
  "artifacts.version": "Version:",
  "artifacts.runCount": "Run count:",
  "artifacts.executionSuccess": "✅ Execution succeeded",
  "artifacts.executionFailed": "❌ Execution failed",
  "artifacts.stdout": "Stdout:",
  "artifacts.stderr": "Stderr:",
  "artifacts.errorMessage": "Error:",
  "artifacts.exitCode": "Exit code:",
  "artifacts.emptyTitle": "Select an app",
  "artifacts.emptyHint": "Select an app from the list on the left to view details",
};
