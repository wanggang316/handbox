/**
 * ui namespace strings (filled by migration subagent).
 */
import type { uiZh } from "../zh/ui";

export const uiEn: Record<keyof typeof uiZh, string> = {
  // Avatar
  "ui.avatarAlt": "Avatar",
  "ui.clickToUpload": "Click to upload",
  // Toast
  "ui.toastSuccessTitle": "Success",
  "ui.toastWarningTitle": "Heads up",
  "ui.toastInfoTitle": "Note",
  "ui.toastErrorTitle": "Something went wrong",
  "ui.gotIt": "Got it",
  // InfoTooltip
  "ui.showHelp": "Show help",
  // ConfirmModal
  "ui.confirmTitle": "Confirm action",
  "ui.confirmMessage": "Are you sure you want to do this?",
  "ui.processing": "Processing…",
  // ChatList
  "ui.chatHeading": "Chat",
  "ui.newChat": "New chat",
  "ui.renamePlaceholder": "Enter a new name",
  "ui.generateTitle": "Generate title",
  "ui.copyTitle": "Copy title",
  "ui.copyId": "Copy ID",
  // Textarea
  "ui.charCount": "Characters: {count}",
  // ResizableSidebar
  "ui.resizeSidebar": "Resize sidebar",
  // TitleBar
  "ui.hideSidebar": "Hide sidebar (⌘B)",
  "ui.showSidebar": "Show sidebar (⌘B)",
  // NumberStepper
  "ui.increase": "Increase",
  "ui.decrease": "Decrease",
  // TextRow
  "ui.inputPlaceholder": "Enter a value",
  // UserSidebar
  "ui.userFallbackName": "User",
  "ui.notLoggedIn": "Not signed in",
  // Splash (+page)
  "ui.loading": "Loading",
  // Auth callback
  "ui.processingLogin": "Signing you in…",
};
