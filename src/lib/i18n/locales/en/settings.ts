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
  "settings.general.autoScrollDesc":
    "Auto-scroll to the bottom on new messages",
  "settings.general.sidebarVibrancy": "Translucent sidebar",
  "settings.general.sidebarVibrancyDesc":
    "Give the sidebar the native macOS frosted-glass look",

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
  "settings.sidebar.hooks": "Hooks",
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
  "settings.quicktools.showToolbarOnSelection":
    "Show toolbar on text selection",
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
  "settings.agentTools.renderCardDesc":
    "Render interactive HTML cards inline in the conversation",
  "settings.agentTools.renderAppDesc":
    "Build full HTML apps in a side panel (preview + source)",
  "settings.agentTools.skillDesc":
    "Let the model discover and load skills on demand",

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

  // Hook rules page
  "settings.hooks.description":
    "Actions run automatically at the agent's lifecycle points (tool calls, prompt submit, turn end, approval), matched in order — the first hit decides",
  "settings.hooks.add": "Add rule",
  "settings.hooks.addTitle": "Add rule",
  "settings.hooks.editTitle": "Edit rule",
  "settings.hooks.empty": "No rules yet",
  "settings.hooks.emptyHint":
    "Add a rule to run a command around tool calls (format, commit, inject context), or get notified when an operation you care about happens.",
  "settings.hooks.anyArgument": "any argument",
  "settings.hooks.promptSubject": "prompt",
  "settings.hooks.replySubject": "reply",
  "settings.hooks.event.before": "Before tool call",
  "settings.hooks.event.after": "After tool call",
  "settings.hooks.event.prompt": "On prompt submit",
  "settings.hooks.event.turnEnd": "On turn end",
  "settings.hooks.event.approval": "On approval request",
  "settings.hooks.action.notify": "Notify",
  "settings.hooks.action.runCommand": "Run command",
  "settings.hooks.field.name": "Name",
  "settings.hooks.field.namePlaceholder": "e.g. Format after write",
  "settings.hooks.field.event": "When",
  "settings.hooks.field.action": "Action",
  "settings.hooks.field.toolPattern": "Tool name",
  "settings.hooks.field.argField": "Argument (optional)",
  "settings.hooks.field.argContains": "Argument contains (optional)",
  "settings.hooks.field.promptContains": "Prompt contains (optional)",
  "settings.hooks.field.promptContainsPlaceholder":
    "Leave empty to fire on every prompt",
  "settings.hooks.field.replyContains": "Reply contains (optional)",
  "settings.hooks.field.replyContainsPlaceholder":
    "Leave empty to fire when every reply ends",
  "settings.hooks.field.command": "Command",
  "settings.hooks.field.commandHint":
    'The script runs via /bin/sh in the session\'s working directory; the event arrives as JSON on stdin, 10s timeout by default.\n\nEnvironment variables\n· $HANDBOX_HOOK_EVENT / $HANDBOX_TOOL_NAME\n· $HANDBOX_SESSION_ID / $HANDBOX_RULE_NAME\n\nWhat the output means\n· Plain output: becomes context for the model on prompt submit\n· {"decision":"deny","reason":"…"}: blocks this call\n· {"updatedInput":{…}}: rewrites the arguments; after a call, the result (redaction)\n· Non-zero exit: treated as a block\n\nPer-event differences\n· On turn end: deny sends the agent back to work with the reason (up to 3 rounds); a broken command only reports\n· On approval request: side effects only (ring, push) — output never affects the decision',
  "settings.hooks.field.message": "Message (optional)",
  "settings.hooks.field.messagePlaceholder":
    "Shown alongside the notice when the rule fires",
  "settings.hooks.field.hint":
    "Tool name to match; * is a wildcard:\n· write — exact match\n· mcp__* — prefix match\n· * — every tool\n\nEmpty argument name: search all arguments\nEmpty contains: match on the tool name alone",
  "settings.hooks.deleteTitle": "Delete rule",
  "settings.hooks.deleteMessage": 'Delete "{name}"?',

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
