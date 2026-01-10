/**
 * 设置相关类型定义
 */

// 主题类型
export type Theme = 'light' | 'dark' | 'system';

// 主题色
export type ThemeColor = 'blue' | 'green' | 'red' | 'yellow' | 'purple' | 'orange' | 'pink' | 'brown' | 'system';

// 语言
export type Language = 'zh-CN' | 'en-US';

// 翻译目标语言（支持 system 或任意语言标签）
export type TranslationTargetLanguage = 'system' | string;

// 快捷键配置
export interface ShortcutConfig {
  sendMessage: string;
  newLine: string;
  switchModel?: string;
}

// 通用设置
export interface GeneralSettings {
  theme: Theme;
  themeColor: ThemeColor;
  language: Language;
  autoScroll: boolean;
  shortcuts: ShortcutConfig;
}

// 翻译设置
export interface TranslationSettings {
  modelId?: string | null;
  providerId?: string | null;
  targetLanguage: TranslationTargetLanguage;
}

// MCP 服务器配置
export interface MCPServer {
  name: string;
  command: string;
  args: string[];
  enabled: boolean;
  workingDir?: string;
  env?: Record<string, string>;
}

// MCP 设置
export interface MCPSettings {
  servers: MCPServer[];
}

// 用户信息
export interface UserInfo {
  id?: string;
  name?: string;
  email?: string;
  avatar?: string;
  isPremium?: boolean;
}

// 账户设置
export interface AccountSettings {
  user?: UserInfo;
  isLoggedIn: boolean;
}

// 应用设置
export interface AppSettings {
  general: GeneralSettings;
  mcp: MCPSettings;
  account: AccountSettings;
  translation: TranslationSettings;
}

// 设置更新请求
export interface UpdateSettingsRequest {
  section: keyof AppSettings;
  data: Partial<AppSettings[keyof AppSettings]>;
}

// 导入导出设置
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
