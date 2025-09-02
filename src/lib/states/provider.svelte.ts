/**
 * 供应商相关状态管理 - 使用 Svelte 5 runes
 */

import type { Provider, Model, ProviderConfig, FrontendProviderConfig, UUID } from '../types';
import * as providerApi from '../api/provider';

// 供应商配置模板（从后端获取）
export let providerConfigs = $state<{
  providers: FrontendProviderConfig[];
  custom_providers: FrontendProviderConfig[];
}>({
  providers: [],
  custom_providers: []
});

// 获取供应商配置信息的工具函数
export function getProviderConfig(providerType: string): FrontendProviderConfig | undefined {
  return [...providerConfigs.providers, ...providerConfigs.custom_providers]
    .find(t => t.provider_type === providerType);
}

// 获取供应商图标
export function getProviderIcon(provider: Provider): string | undefined {
  const config = getProviderConfig(provider.provider_type);
  return config?.icon || undefined;
}

// 获取供应商显示名称
export function getProviderDisplayName(providerType: string): string {
  const config = getProviderConfig(providerType);
  return config?.type_name || providerType;
}

// 全局状态对象
export const providerState = $state({
  // 供应商列表
  providers: [] as Provider[],
  
  // 当前选中的供应商（用于详情页面和编辑）
  currentProvider: null as Provider | null,
  
  // 正在编辑的供应商（用于模态框）
  editingProvider: null as Provider | null,
  
  // 所有可用模型
  availableModels: [] as Model[],
  
  // 加载状态
  isLoading: false,

  // 获取模型列表状态
  isFetchingModels: null as UUID | null,
  
  // 错误状态
  error: null as string | null,
});

// 派生状态：已启用的供应商（函数形式）
export function getEnabledProviders(): Provider[] {
  return providerState.providers.filter(p => p.enabled);
}

// 派生状态：可用的模型（来自已启用的供应商）
export function getEnabledModels(): Model[] {
  const enabledProviderIds = getEnabledProviders().map(p => p.id);
  return providerState.availableModels.filter(m => 
    enabledProviderIds.includes(m.provider_id) && m.enabled
  );
}

// 派生状态：按供应商分组的模型
export function getModelsByProvider(): Record<string, { provider: Provider; models: Model[] }> {
  const groups: Record<string, { provider: Provider; models: Model[] }> = {};
  
  for (const provider of providerState.providers) {
    const providerModels = providerState.availableModels.filter(m => m.provider_id === provider.id);
    if (providerModels.length > 0) {
      groups[provider.id] = { provider, models: providerModels };
    }
  }
  
  return groups;
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
      console.log('providerList', providerList);
      providerState.providers = providerList;
      
      // 如果提供商包含模型，收集所有模型
      const allModels = providerList.flatMap(p => (p as any).models || []);
      providerState.availableModels = allModels;
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '加载供应商列表失败';
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  /**
   * 创建供应商
   */
  async createProvider(config: ProviderConfig): Promise<Provider> {
    try {
      providerState.isLoading = true;
      const provider = await providerApi.createProvider(config);
      
      // 添加到列表
      providerState.providers.push(provider);
      
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
  async updateProvider(providerId: UUID, config: Partial<ProviderConfig>): Promise<void> {
    try {
      providerState.isLoading = true;
      const updatedProvider = await providerApi.updateProvider(providerId, config);
      
      // 更新列表中的供应商
      const index = providerState.providers.findIndex(p => p.id === providerId);
      if (index !== -1) {
        providerState.providers[index] = updatedProvider;
      }
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
  async fetchProviderModels(providerId: UUID, forceRefresh = false): Promise<void> {
    try {
      providerState.isFetchingModels = providerId;
      const response = await providerApi.getProviderModels({ 
        providerId: providerId, 
        forceRefresh: forceRefresh 
      });
      
      // 更新全局模型列表
      const otherModels = providerState.availableModels.filter(m => m.provider_id !== providerId);
      providerState.availableModels = [...otherModels, ...response.models];
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
      console.log('toggleProvider', providerId, enabled);
      const updatedProvider = await providerApi.toggleProvider(providerId, enabled);
      
      const index = providerState.providers.findIndex(p => p.id === providerId);
      if (index !== -1) {
        providerState.providers[index] = updatedProvider;
      }
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
      await providerApi.toggleModel(providerId, modelId, enabled);
      
      // 更新模型状态
      const index = providerState.availableModels.findIndex(m => 
        m.id === modelId && m.provider_id === providerId
      );
      if (index !== -1) {
        providerState.availableModels[index] = { 
          ...providerState.availableModels[index], 
          enabled 
        };
      }
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '切换模型状态失败';
      throw error;
    }
  },

  /**
   * 选择供应商（保持向后兼容）
   */
  selectProvider(provider: Provider | null): void {
    providerStateActions.setCurrentProvider(provider);
  },

  /**
   * 根据模型ID查找模型
   */
  findModel(modelId: string): Model | undefined {
    return providerState.availableModels.find(m => m.id === modelId);
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
    providerState.availableModels = [];
    providerState.isLoading = false;
    providerState.isFetchingModels = null;
    providerState.error = null;
    
    // 重置模板
    providerConfigs.providers = [];
    providerConfigs.custom_providers = [];
  }
};