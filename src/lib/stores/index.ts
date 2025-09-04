/**
 * 状态管理 - 统一导出
 */

// 聊天相关 - 使用新的状态管理
export { chatState } from '../states/chat.svelte';

// 供应商相关
export {
  providerState,
  providerActions,
  providerStateActions,
  providerConfigs,
  getProviderConfig,
  getProviderIcon,
  getEnabledProviders,
  getProviderDropdownOptions
} from '../states/provider.svelte';

// Artifact 相关
export {
  artifacts,
  selectedArtifact,
  isLoading as artifactLoading,
  artifactError,
  artifactFilter,
  artifactStats,
  filteredArtifacts,
  artifactActions
} from './artifact';

// 设置相关
export {
  appSettings,
  settingsLoading,
  settingsError,
  settingsActions
} from './settings';

// 搜索相关
export {
  searchQuery,
  searchResults,
  searchHistory,
  searchSuggestions,
  searchLoading,
  searchError,
  hasSearchResults,
  searchActions
} from './search';

// UI 相关
export {
  sidebarOpen,
  currentPage,
  modals,
  notifications,
  theme,
  themeColor,
  language,
  globalLoading,
  isDarkMode,
  uiActions
} from './ui';

// 类型导出
export type { Notification } from './ui';