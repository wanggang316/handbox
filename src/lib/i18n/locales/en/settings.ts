/**
 * Settings page strings.
 */
import type { settingsZh } from "../zh/settings";

export const settingsEn: Record<keyof typeof settingsZh, string> = {
  "settings.general.appearance": "Appearance",
  "settings.general.theme.system": "Follow system",
  "settings.general.theme.light": "Light",
  "settings.general.theme.dark": "Dark",
  "settings.general.language": "Language",
  "settings.general.autoScroll": "Auto-scroll chat view",

  // Sidebar
  "settings.sidebar.account": "Account",
  "settings.sidebar.general": "General",
  "settings.sidebar.quicktools": "Quick Tools",
  "settings.sidebar.models": "Models",
  "settings.sidebar.agentTools": "Agent Tools",
  "settings.sidebar.skills": "Skills",
  "settings.sidebar.components": "Components",
  "settings.sidebar.shortcuts": "Shortcuts",
  "settings.sidebar.about": "About",

  // About page
  "settings.about.softwareUpdate": "Software Update",
  "settings.about.autoCheck": "Check for updates automatically",
  "settings.about.autoCheckHint": "Check on launch",
  "settings.about.checkUpdate": "Check for updates",
  "settings.about.checking": "Checking…",
  "settings.about.updateAvailable": "New version v{version} available",
  "settings.about.currentVersion": "Current version v{version}",
  "settings.about.title": "About",
  "settings.about.changelog": "Changelog",
  "settings.about.officialSite": "Official Website",

  // Shortcuts page
  "settings.shortcuts.sendMessage": "Send message",

  // Quick Tools page
  "settings.quicktools.showToolbarOnSelection": "Show toolbar on text selection",
  "settings.quicktools.permissionRequired": "Accessibility permission required",
  "settings.quicktools.disabledApps": "Disabled apps",
  "settings.quicktools.disabledAppsEmpty":
    "Apps where the selection tool is disabled will appear here.",
  "settings.quicktools.permissionGuide":
    'Enabling this feature requires accessibility permission. Go to "System Settings > Privacy & Security > Accessibility" and enable HandBox.',
  "settings.quicktools.openSystemSettings": "Open System Settings",
  "settings.quicktools.refreshPermission": "Refresh permission status",

  // Agent Tools page
  "settings.agentTools.title": "Agent Tools",
  "settings.agentTools.description":
    "Tools enabled by default for new Agent sessions. Existing sessions are unaffected.",

  // Skills page
  "settings.skills.title": "Skills",
  "settings.skills.description":
    "Place SKILL.md in the skills directory to list it here; valid skills can be toggled on or off",
  "settings.skills.loading": "Loading skills...",
  "settings.skills.scope.user": "User",
  "settings.skills.scope.project": "Project",
  "settings.skills.scope.appData": "App",
  "settings.skills.openDir": "Open directory",
  "settings.skills.collapseBody": "Collapse",
  "settings.skills.expandBody": "View content",
  "settings.skills.empty": "No skills yet",
  "settings.skills.emptyHint":
    "Put a SKILL.md file in the skills directory, then click Refresh to see it here.",

  // Account page
  "settings.account.editProfile": "Edit Profile",
  "settings.account.loggingOut": "Signing out...",
  "settings.account.logout": "Sign out",
  "settings.account.updateFailed": "Update failed, please try again",
  "settings.account.logoutFailed": "Sign out failed, please try again",
  "settings.account.notLoggedIn": "Not signed in",
  "settings.account.defaultUsername": "User",
  "settings.account.username": "Username",
  "settings.account.usernamePlaceholder": "Enter",
};
