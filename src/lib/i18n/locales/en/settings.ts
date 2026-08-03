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
  "settings.general.section": "General",
  "settings.general.appearanceDesc": "Choose the light or dark color scheme",
  "settings.general.languageDesc": "Interface display language",
  "settings.general.autoScrollDesc": "Auto-scroll to the bottom on new messages",

  // Sidebar
  "settings.sidebar.backToApp": "Back to app",
  "settings.sidebar.search": "Search settings...",
  "settings.sidebar.group.personal": "Personal",
  "settings.sidebar.group.features": "Features",
  "settings.sidebar.group.other": "Other",
  "settings.sidebar.account": "Account",
  "settings.sidebar.general": "General",
  "settings.sidebar.session": "Sessions",
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

  // Quick Tools page
  "settings.quicktools.selectionToolbarGroup": "Text Selection Toolbar",
  "settings.quicktools.showToolbarOnSelection": "Show toolbar on text selection",
  "settings.quicktools.translationAgent": "Selection translate Agent",
  "settings.quicktools.translationAgentDesc":
    "Agent used by the toolbar Translate button; falls back to the built-in translator when unset",
  "settings.quicktools.translationAgentDefault": "Built-in translator",
  "settings.quicktools.quickActionGroup": "Quick Action",
  "settings.quicktools.enableQuickAction": "Enable Quick Action",
  "settings.quicktools.enableQuickActionDesc":
    "Summon the quick action overlay with the global hotkey {shortcut}",
  "settings.quicktools.permissionRequired": "Accessibility permission required",
  "settings.quicktools.disabledApps": "Disabled apps",
  "settings.quicktools.disabledAppsEmpty":
    "Apps where the selection tool is disabled will appear here.",
  "settings.quicktools.permissionGuide":
    'Enabling this feature requires accessibility permission. Go to "System Settings > Privacy & Security > Accessibility" and enable HandBox.',
  "settings.quicktools.openSystemSettings": "Open System Settings",
  "settings.quicktools.refreshPermission": "Refresh permission status",

  // Session page
  "settings.session.section": "Session Title",
  "settings.session.titleGeneration": "Title generation",
  "settings.session.titleGeneration.firstMessage": "After the first message",
  "settings.session.titleGeneration.everyMessage": "After every message",
  "settings.session.titleGeneration.off": "Off",

  // Agent Tools page
  "settings.agentTools.title": "Agent Tools",
  "settings.agentTools.description":
    "Tools enabled by default for new Agent sessions. Existing sessions are unaffected.",
  "settings.agentTools.webSearch.title": "Web Search",
  "settings.agentTools.webSearch.provider": "Search provider",
  "settings.agentTools.webSearch.apiKey": "API Key",
  "settings.agentTools.webSearch.apiKeyPlaceholder": "tvly-...",
  "settings.agentTools.system.title": "System",
  "settings.agentTools.uiExtensions.title": "UI",
  "settings.agentTools.skill.title": "Skill",
  "settings.agentTools.renderCardDesc": "Render interactive HTML cards inline in the conversation",
  "settings.agentTools.renderAppDesc": "Build full HTML apps in a side panel (preview + source)",
  "settings.agentTools.skillDesc": "Let the model discover and load skills on demand",

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
