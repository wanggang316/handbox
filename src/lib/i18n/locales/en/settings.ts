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

  // Sidebar
  "settings.sidebar.backToApp": "Back to app",
  "settings.sidebar.search": "Search settings...",
  "settings.sidebar.group.personal": "Personal",
  "settings.sidebar.group.features": "Features",
  "settings.sidebar.group.other": "Other",
  "settings.sidebar.account": "Account",
  "settings.sidebar.general": "General",
  "settings.sidebar.quicktools": "Quick Tools",
  "settings.sidebar.models": "Models",
  "settings.sidebar.agentTools": "Agent Tools",
  "settings.sidebar.hooks": "Hook Rules",
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
    "Actions run automatically around the agent's tool calls and prompts, matched in order — the first hit decides",
  "settings.hooks.add": "Add rule",
  "settings.hooks.addTitle": "Add rule",
  "settings.hooks.editTitle": "Edit rule",
  "settings.hooks.empty": "No rules yet",
  "settings.hooks.emptyHint":
    "Add a rule to run a command around tool calls (format, commit, inject context), or get notified when an operation you care about happens.",
  "settings.hooks.anyArgument": "any argument",
  "settings.hooks.promptSubject": "prompt",
  "settings.hooks.event.before": "Before call",
  "settings.hooks.event.after": "After call",
  "settings.hooks.event.prompt": "On prompt submit",
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
  "settings.hooks.field.command": "Command",
  "settings.hooks.field.commandHint":
    'The event arrives as JSON on stdin; $HANDBOX_HOOK_EVENT / $HANDBOX_TOOL_NAME / $HANDBOX_SESSION_ID / $HANDBOX_RULE_NAME are set. Runs in the session\'s working directory, 10s timeout by default. Print {"decision":"deny","reason":"…"} to block or {"updatedInput":{…}} to rewrite the arguments (after a call this rewrites the result, which is how redaction works); a non-zero exit blocks it. On prompt submission, plain output becomes context for the model — or set it explicitly with {"additionalContext":"…"}.',
  "settings.hooks.field.message": "Message (optional)",
  "settings.hooks.field.messagePlaceholder":
    "Shown alongside the notice when the rule fires",
  "settings.hooks.field.hint":
    "Tool name accepts * as a wildcard, e.g. mcp__* or *. Leave the argument name empty to search all arguments; leave the condition empty to match on the tool name alone.",
  "settings.hooks.deleteTitle": "Delete rule",
  "settings.hooks.deleteMessage": 'Delete "{name}"?',
  // Runtime notices — shown when a rule matches a tool call
  "settings.hooks.notice.denied": 'Rule "{rule}" command blocked {tool}',
  "settings.hooks.notice.observed": 'Rule "{rule}" matched {tool}',
  "settings.hooks.notice.ran": 'Rule "{rule}" ran its command on {tool}',
  "settings.hooks.notice.rewrote":
    'Rule "{rule}" rewrote the arguments to {tool}',
  "settings.hooks.notice.failed": 'Rule "{rule}" command failed on {tool}',
  "settings.hooks.notice.informed": 'Rule "{rule}" added context for this turn',

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
