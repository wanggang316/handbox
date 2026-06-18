/**
 * Shared strings reused across modules.
 *
 * `Record<keyof typeof commonZh, string>` enforces this file stays in sync
 * with the canonical zh catalog at compile time (locally, per namespace).
 */
import type { commonZh } from "../zh/common";

export const commonEn: Record<keyof typeof commonZh, string> = {
  // actions
  "common.save": "Save",
  "common.cancel": "Cancel",
  "common.confirm": "Confirm",
  "common.delete": "Delete",
  "common.remove": "Remove",
  "common.close": "Close",
  "common.edit": "Edit",
  "common.add": "Add",
  "common.create": "Create",
  "common.rename": "Rename",
  "common.duplicate": "Duplicate",
  "common.copy": "Copy",
  "common.copied": "Copied",
  "common.apply": "Apply",
  "common.submit": "Submit",
  "common.send": "Send",
  "common.reset": "Reset",
  "common.retry": "Retry",
  "common.refresh": "Refresh",
  "common.clear": "Clear",
  "common.select": "Select",
  "common.open": "Open",
  "common.export": "Export",
  "common.import": "Import",
  "common.download": "Download",
  "common.upload": "Upload",
  "common.back": "Back",
  "common.next": "Next",
  "common.previous": "Previous",
  "common.done": "Done",
  "common.ok": "OK",
  "common.yes": "Yes",
  "common.no": "No",
  "common.expand": "Expand",
  "common.collapse": "Collapse",
  // states
  "common.loading": "Loading…",
  "common.saving": "Saving…",
  "common.search": "Search",
  "common.enable": "Enable",
  "common.disable": "Disable",
  "common.enabled": "Enabled",
  "common.disabled": "Disabled",
  "common.error": "Error",
  "common.success": "Success",
  "common.warning": "Warning",
  "common.empty": "No data",
  "common.all": "All",
  "common.none": "None",
  "common.default": "Default",
  "common.untitled": "Untitled",
  "common.unknown": "Unknown",
  // fields
  "common.name": "Name",
  "common.description": "Description",
  // existing
  "common.settings": "Settings",
  "common.account": "Account",
  "common.login": "Sign in",
  "common.logout": "Sign out",
};
