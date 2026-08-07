export type Theme = "light" | "dark" | "system";

export type Language = "zh-CN" | "en-US";

// "system" or any language tag.
export type TranslationTargetLanguage = "system" | string;

export interface ShortcutConfig {
  sendMessage: string;
  newLine: string;
  switchModel?: string;
}

export interface GeneralSettings {
  theme: Theme;
  language: Language;
  autoScroll: boolean;
  /** macOS frosted-glass sidebar (window vibrancy); ignored elsewhere. */
  sidebarVibrancy: boolean;
  shortcuts: ShortcutConfig;
}

export interface TranslationSettings {
  sessionId?: string | null;
  agentId?: string | null; // Agent definition used to create sessionId; null = builtin fallback.
}

export interface MCPServer {
  name: string;
  command: string;
  args: string[];
  enabled: boolean;
  workingDir?: string;
  env?: Record<string, string>;
}

export interface MCPSettings {
  servers: MCPServer[];
}

export interface UserInfo {
  id?: string;
  name?: string;
  email?: string;
  avatar?: string;
  isPremium?: boolean;
}

export interface AccountSettings {
  user?: UserInfo;
  isLoggedIn: boolean;
}

export interface SelectionBlacklist {
  pids: number[];
  bundleIds: string[];
}

export interface QuickToolsSettings {
  showToolbarOnSelection: boolean;
  translationAgentId?: string | null; // Agent definition for selection "translate"; null = builtin fallback.
  selectionBlacklist: SelectionBlacklist;
}

// Search-provider config for the agent web_search tool.
export interface WebSearchSettings {
  provider: string; // Search provider id (currently only "tavily").
  apiKey: string; // Empty string = unconfigured; the tool is not registered.
}

export interface AgentSettings {
  defaultEnabledTools: string[]; // Enabled by default for new sessions (registration names, incl. extension tools).
  defaultEditorId?: string | null; // Default "Open in ..." target id (see api/openIn.ts).
  webSearch?: WebSearchSettings; // Absent = unconfigured.
}

export interface QuickActionSettings {
  enabled?: boolean; // Absent = true; when disabled the global shortcut is not registered.
  shortcut?: string; // Global shortcut that summons the quick-action panel.
  modelId?: string | null; // Unset falls back to the default-model resolver.
  providerId?: string | null; // Unset falls back to the default-model resolver.
}

// When a session's title is regenerated automatically. The manual "generate
// title" action is unaffected by this rule.
export type TitleGenerationRule = "firstMessage" | "everyMessage" | "off";

export interface SessionSettings {
  titleGeneration: TitleGenerationRule;
}

export interface AppSettings {
  general: GeneralSettings;
  mcp: MCPSettings;
  account: AccountSettings;
  translation: TranslationSettings;
  quickTools: QuickToolsSettings;
  agent: AgentSettings;
  quickAction?: QuickActionSettings;
  session?: SessionSettings; // Absent on configs written before the section existed.
}

export interface UpdateSettingsRequest {
  section: keyof AppSettings;
  data: Partial<AppSettings[keyof AppSettings]>;
}

export interface ExportSettingsOptions {
  includeProviders?: boolean;
  includeMCP?: boolean;
  includeShortcuts?: boolean;
}

export interface ImportSettingsRequest {
  data: string;
  overwrite?: boolean;
  sections?: Array<keyof AppSettings>;
}
