/**
 * 供应商相关状态管理 - 使用 Svelte 5 runes
 */

import type {
	Provider,
	Model,
	AddProviderRequest,
	ProviderConfig,
	UUID,
	ProviderWithModels
} from '../types';
import type { ModelWithProvider } from '../types/provider';
import * as providerApi from '../api/provider';
import * as modelApi from '../api/model';
import { listen, emit, type UnlistenFn } from '@tauri-apps/api/event';

declare global {
  interface Window {
    __TAURI__?: unknown;
    isTauri?: boolean;
  }
}

/**
 * 检测是否在 Tauri 环境中运行
 * Tauri 2.0+ 提供 window.isTauri 和 window.__TAURI__
 * 注意：只有在 Tauri webview 中才会有这些属性（npm run tauri dev）
 * 如果运行 npm run dev（浏览器模式），这些属性不存在
 */
function isTauriEnvironment(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  // Tauri 2.0+ 推荐方式
  if ('isTauri' in window && window.isTauri === true) {
    return true;
  }

  // 兼容旧版本
  if ('__TAURI__' in window && window.__TAURI__) {
    return true;
  }

  return false;
}

/**
 * 使用 Tauri 2 的 emit() API 向所有窗口广播事件
 * emit() 会自动将事件发送到所有窗口，无需手动遍历
 */
async function emitProvidersUpdated(
  payload: Record<string, unknown>
): Promise<void> {
  console.log('[emitProvidersUpdated] Checking environment...');
  console.log('[emitProvidersUpdated] isTauriEnvironment:', isTauriEnvironment());

  if (!isTauriEnvironment()) {
    console.log('[emitProvidersUpdated] Not in Tauri environment, skipping emit');
    return;
  }

  try {
    console.log('[emitProvidersUpdated] Emitting providers:updated event with payload:', payload);
    // Tauri 2: emit() 自动广播到所有窗口
    await emit('providers:updated', payload);
    console.log('[emitProvidersUpdated] Event emitted successfully to all windows');
  } catch (error) {
    console.error('[emitProvidersUpdated] Failed to broadcast providers:updated event:', error);
  }
}

// 供应商配置模板（从后端获取）
export let providerConfigs = $state<{
  providers: ProviderConfig[];
  custom_providers: ProviderConfig[];
}>({
  providers: [],
  custom_providers: []
});

// 获取供应商配置信息的工具函数
export function getProviderConfig(providerType: string): ProviderConfig | undefined {
  return [...providerConfigs.providers, ...providerConfigs.custom_providers]
    .find(t => t.provider_type === providerType);
}

// 获取供应商图标
export function getProviderIcon(provider: Provider): string | undefined {
  const config = getProviderConfig(provider.provider_type);
  return config?.icon || undefined;
}

// 根据 providerId 获取供应商配置
export function getProviderConfigById(providerId: string): ProviderConfig | undefined {
  // 先从当前 provider 列表中查找对应的供应商
  const provider = providerState.providers.find(p => p.id === providerId) ||
                  providerState.providersWithModels.find(p => p.id === providerId);

  if (provider) {
    return getProviderConfig(provider.provider_type);
  }

  return undefined;
}

// 根据 providerId 获取供应商图标
export function getProviderIconById(providerId: string): string | undefined {
  const config = getProviderConfigById(providerId);
  return config?.icon || undefined;
}

// 全局状态对象
export const providerState = $state({
  // 供应商列表
  providers: [] as Provider[],
  
  // 当前选中的供应商（用于详情页面和编辑）
  currentProvider: null as Provider | null,
  
  // 正在编辑的供应商（用于模态框）
  editingProvider: null as Provider | null,
  
  // currentProvider的模型
  currentModels: [] as Model[],
  
  // 带模型的供应商列表（用于聊天功能）
  providersWithModels: [] as ProviderWithModels[],
  providersWithModelsNeedRefresh: true,
  
  // 加载状态
  isLoading: false,
  isLoadingWithModels: false,

  // 获取模型列表状态
  isFetchingModels: null as UUID | null,
  
  // 错误状态
  error: null as string | null,
});

function markProvidersWithModelsDirty(
  reason: string,
  data?: Record<string, unknown>
): void {
  console.log('[markProvidersWithModelsDirty] Called with reason:', reason, 'data:', data);
  providerState.providersWithModelsNeedRefresh = true;
  const payload = data ? { reason, ...data } : { reason };
  console.log('[markProvidersWithModelsDirty] Calling emitProvidersUpdated with payload:', payload);
  void emitProvidersUpdated(payload);
}

let providersUpdatedUnlisten: UnlistenFn | null = null;

// 派生状态：已启用的供应商（函数形式）
export function getEnabledProviders(): Provider[] {
  return providerState.providers.filter(p => p.enabled);
}

// 派生状态：所有可用模型（带供应商信息）
export function getAllModels(): ModelWithProvider[] {
  return providerState.providersWithModels.flatMap(provider =>
    provider.models.map(model => ({
      ...model,
      providerName: provider.name,
      providerType: provider.provider_type
    }))
  );
}

// 派生状态：收藏模型
export function getFavoriteModels(): ModelWithProvider[] {
  return getAllModels().filter(model => model.favorite);
}


// 获取供应商下拉选项组
export function getProviderDropdownOptions() {
  const preProviderOptions = providerConfigs.providers.map(provider => ({
    value: provider.provider_type,
    label: provider.type_name,
    icon: provider.icon
  }));

  const customProviderOptions = providerConfigs.custom_providers.map(provider => ({
    value: provider.provider_type,
    label: provider.type_name,
    icon: provider.icon
  }));

  return [
    {
      title: "",
      options: preProviderOptions
    },
    {
      title: "",
      options: customProviderOptions
    }
  ];
}

// 供应商状态管理辅助函数
export const providerStateActions = {
  /**
   * 设置当前供应商（用于详情页面）
   */
  setCurrentProvider(provider: Provider | null): void {
    providerState.currentProvider = provider;
  },

  /**
   * 根据ID设置当前供应商
   */
  setCurrentProviderById(providerId: UUID): Provider | null {
    const provider = providerState.providers.find(p => p.id === providerId);
    providerState.currentProvider = provider || null;
    return provider || null;
  },

  /**
   * 开始编辑供应商（用于模态框）
   */
  startEditProvider(provider: Provider | null): void {
    providerState.editingProvider = provider;
  },

  /**
   * 结束编辑供应商
   */
  endEditProvider(): void {
    providerState.editingProvider = null;
  },

  /**
   * 更新当前供应商信息（用于实时更新UI）
   */
  updateCurrentProvider(updatedProvider: Provider): void {
    if (providerState.currentProvider && providerState.currentProvider.id === updatedProvider.id) {
      providerState.currentProvider = updatedProvider;
    }
  },

  /**
   * 刷新当前供应商的详细信息（包括模型列表）
   */
  async refreshCurrentProvider(): Promise<void> {
    if (providerState.currentProvider && providerState.currentProvider.id) {
      const providerId = providerState.currentProvider.id;
      try {
        // 重新获取供应商信息
        const updatedProvider = await providerActions.getProvider(providerId);
        providerState.currentProvider = updatedProvider;
        
        // 强制刷新模型列表
        await providerActions.fetchProviderModels(providerId, true);
      } catch (error) {
        console.error("Failed to refresh current provider:", error);
      }
    }
  },

  /**
   * 清除所有选中状态
   */
  clearSelection(): void {
    providerState.currentProvider = null;
    providerState.editingProvider = null;
  }
};

/**
 * 供应商操作
 */
export const providerActions = {
  /**
   * 加载供应商配置模板
   */
  async loadProviderConfigs(): Promise<void> {
    try {
      const configs = await providerApi.getProviderConfigs();
      providerConfigs.providers = configs.providers;
      providerConfigs.custom_providers = configs.custom_providers;
    } catch (error) {
      console.error('Failed to load provider templates:', error);
      // 不抛出错误，因为这不应该阻止应用启动
    }
  },

  /**
   * 加载供应商列表
   */
  async loadProviders(): Promise<void> {
    try {
      providerState.isLoading = true;
      const providerList = await providerApi.getProviders();
      providerState.providers = providerList;
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '加载供应商列表失败';
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  /**
   * 加载带模型的供应商列表（用于聊天功能）
   */
  /**
  * 加载带模型的供应商列表
  * @param refreshFromRemote 当为 true 时，会先从远程拉取最新模型并同步数据库；默认仅从本地数据库读取
  */
  async loadProvidersWithModels(refreshFromRemote = false): Promise<void> {
    try {
      providerState.isLoadingWithModels = true;
      providerState.error = null;

      const providersWithModels = await modelApi.getAllModelsWithProviders(refreshFromRemote);
      providerState.providersWithModels = providersWithModels;

      console.log(
        "action do ->> providerState.providersWithModelsNeedRefresh: " +
          providerState.providersWithModelsNeedRefresh
      );

      providerState.providersWithModelsNeedRefresh = false;

    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '加载供应商列表失败';
      providerState.providersWithModelsNeedRefresh = true;
      throw error;
    } finally {
      providerState.isLoadingWithModels = false;
    }
  },

  /**
   * 获取单个供应商
   */
  async getProvider(providerId: string): Promise<Provider> {
    const response = await providerApi.getProvider(providerId);
    return response;
  },

  /**
   * 创建供应商
   */
  async createProvider(config: AddProviderRequest): Promise<Provider> {
    try {
      providerState.isLoading = true;
      const provider = await providerApi.createProvider(config);
      
      // 添加到列表
      providerState.providers.push(provider);
      markProvidersWithModelsDirty('provider-created', { providerId: provider.id });
      
      return provider;
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '创建供应商失败';
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  /**
   * 更新供应商
   */
  async updateProvider(providerId: UUID, config: Partial<AddProviderRequest>): Promise<void> {
    try {
      providerState.isLoading = true;
      const updatedProvider = await providerApi.updateProvider(providerId, config);
      
      // 更新列表中的供应商
      const index = providerState.providers.findIndex(p => p.id === providerId);
      if (index !== -1) {
        providerState.providers[index] = updatedProvider;
      }
      markProvidersWithModelsDirty('provider-updated', { providerId });
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '更新供应商失败';
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  /**
   * 删除供应商
   */
  async deleteProvider(providerId: UUID): Promise<void> {
    try {
      providerState.isLoading = true;
      await providerApi.deleteProvider(providerId);
      
      // 从列表中移除
      providerState.providers = providerState.providers.filter(p => p.id !== providerId);
      
      // 如果是当前选中的供应商，清空选择
      if (providerState.currentProvider?.id === providerId) {
        providerStateActions.clearSelection();
      }
      markProvidersWithModelsDirty('provider-deleted', { providerId });
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '删除供应商失败';
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },



  /**
   * 获取供应商模型列表
   */
  async fetchProviderModels(providerId: UUID, refreshFromRemote = false): Promise<void> {
    try {
      providerState.isFetchingModels = providerId;
      const response = await modelApi.getProviderModels(providerId, refreshFromRemote);
      
      // 更新当前模型列表
      providerState.currentModels = response.models;

      const providersWithModelsIndex = providerState.providersWithModels.findIndex(
        provider => provider.id === providerId
      );
      if (providersWithModelsIndex !== -1) {
        providerState.providersWithModels[providersWithModelsIndex] = {
          ...providerState.providersWithModels[providersWithModelsIndex],
          models: response.models
        };
      }

      if (refreshFromRemote) {
        providerState.providersWithModelsNeedRefresh = true;
      }

    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '获取模型列表失败';
      throw error;
    } finally {
      providerState.isFetchingModels = null;
    }
  },

  /**
   * 启用/禁用供应商
   */
  async toggleProvider(providerId: UUID, enabled: boolean): Promise<void> {
    try {
      const updatedProvider = await providerApi.toggleProvider(providerId, enabled);
      
      const index = providerState.providers.findIndex(p => p.id === providerId);
      if (index !== -1) {
        providerState.providers[index] = updatedProvider;
      }

      const providersWithModelsIndex = providerState.providersWithModels.findIndex(
        provider => provider.id === providerId
      );
      if (providersWithModelsIndex !== -1) {
        providerState.providersWithModels[providersWithModelsIndex] = {
          ...providerState.providersWithModels[providersWithModelsIndex],
          enabled
        };
      }

      markProvidersWithModelsDirty('provider-toggled', { providerId, enabled });

      console.log(
        "set providerState.providersWithModelsNeedRefresh: " +
          providerState.providersWithModelsNeedRefresh
      );
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '切换供应商状态失败';
      throw error;
    }
  },

  /**
   * 启用/禁用模型
   */
  async toggleModel(providerId: UUID, modelId: string, enabled: boolean): Promise<void> {
    try {
      await modelApi.toggleModel(providerId, modelId, enabled);
      
      // 更新当前模型状态
      const index = providerState.currentModels.findIndex(m => 
        m.id === modelId
      );
      if (index !== -1) {
        providerState.currentModels[index] = { 
          ...providerState.currentModels[index], 
          enabled 
        };
      }

      const providerIndex = providerState.providersWithModels.findIndex(p => p.id === providerId);
      if (providerIndex !== -1) {
        const modelIndex = providerState.providersWithModels[providerIndex].models.findIndex(m => m.id === modelId);
        if (modelIndex !== -1) {
          providerState.providersWithModels[providerIndex].models[modelIndex] = {
            ...providerState.providersWithModels[providerIndex].models[modelIndex],
            enabled
          };
        }
      }

      markProvidersWithModelsDirty('model-toggled', { providerId, modelId, enabled });
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '切换模型状态失败';
      throw error;
    }
  },

  /**
   * 切换模型收藏状态
   */
  async toggleModelFavorite(
    providerId: UUID,
    modelId: string,
    favorite: boolean,
    options?: { skipRefreshFlag?: boolean }
  ): Promise<void> {
    try {
      await modelApi.toggleModelFavorite(providerId, modelId, favorite);
      
      // 更新当前模型状态 (currentModels)
      const currentIndex = providerState.currentModels.findIndex(m => 
        m.id === modelId
      );
      if (currentIndex !== -1) {
        providerState.currentModels[currentIndex] = { 
          ...providerState.currentModels[currentIndex], 
          favorite 
        };
      }

      const providerIndex = providerState.providersWithModels.findIndex(p => p.id === providerId);
      if (providerIndex !== -1) {
        const modelIndex = providerState.providersWithModels[providerIndex].models.findIndex(m => m.id === modelId);
        if (modelIndex !== -1) {
          providerState.providersWithModels[providerIndex].models[modelIndex] = {
            ...providerState.providersWithModels[providerIndex].models[modelIndex],
            favorite
          };
        }
      }

      if (!options?.skipRefreshFlag) {
        markProvidersWithModelsDirty('model-favorite-toggled', {
          providerId,
          modelId,
          favorite
        });
      }
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '切换模型收藏状态失败';
      throw error;
    }
  },

  /**
   * 根据模型ID查找模型
   */
  findModel(modelId: string): Model | undefined {
    return providerState.currentModels.find(m => m.id === modelId);
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    providerState.error = null;
  },

  /**
   * 重置所有状态
   */
  reset(): void {
    providerState.providers = [];
    providerState.currentProvider = null;
    providerState.editingProvider = null;
    providerState.currentModels = [];
    providerState.providersWithModels = [];
    providerState.isLoading = false;
    providerState.isLoadingWithModels = false;
    providerState.isFetchingModels = null;
    providerState.error = null;
    providerState.providersWithModelsNeedRefresh = true;
    
    // 重置模板
    providerConfigs.providers = [];
    providerConfigs.custom_providers = [];
  }
};

/**
 * 注册 providers:updated 事件监听器
 * 应该在组件 onMount 时调用，确保 Tauri 环境已准备好
 */
export async function setupProvidersUpdatedListener(): Promise<void> {
  console.log('[setupProvidersUpdatedListener] Setting up listener...');
  console.log('[setupProvidersUpdatedListener] Environment check:');
  console.log('  - typeof window:', typeof window);
  console.log('  - window.isTauri:', typeof window !== 'undefined' ? window.isTauri : 'N/A');
  console.log('  - window.__TAURI__:', typeof window !== 'undefined' ? window.__TAURI__ : 'N/A');
  console.log('  - isTauriEnvironment():', isTauriEnvironment());
  console.log('[setupProvidersUpdatedListener] providersUpdatedUnlisten:', providersUpdatedUnlisten);

  if (!isTauriEnvironment()) {
    console.warn('[setupProvidersUpdatedListener] ⚠️  Not in Tauri environment!');
    console.warn('  Make sure you are running "npm run tauri dev", not just "npm run dev"');
    console.warn('  Cross-window events will not work in browser-only mode');
    return;
  }

  if (providersUpdatedUnlisten) {
    console.log('[setupProvidersUpdatedListener] Listener already set up');
    return;
  }

  try {
    console.log('[setupProvidersUpdatedListener] Registering listener for providers:updated event');
    providersUpdatedUnlisten = await listen('providers:updated', event => {
      console.log('[providersUpdatedListener] providers:updated event received', event);
      // 仅标记需要刷新，不自动加载
      // 让各个组件根据自己的需要在打开时检查并加载
      providerState.providersWithModelsNeedRefresh = true;
      console.log('[providersUpdatedListener] Set providersWithModelsNeedRefresh to true');
    });
    console.log('[setupProvidersUpdatedListener] Listener registered successfully');
  } catch (error) {
    console.error('[setupProvidersUpdatedListener] Failed to register providers:updated listener:', error);
  }
}

/**
 * 清理事件监听器
 */
export function cleanupProvidersUpdatedListener(): void {
  if (providersUpdatedUnlisten) {
    console.log('[cleanupProvidersUpdatedListener] Cleaning up listener');
    providersUpdatedUnlisten();
    providersUpdatedUnlisten = null;
  }
}
