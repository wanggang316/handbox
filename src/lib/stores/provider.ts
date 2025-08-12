/**
 * 供应商相关状态管理
 */

import { writable, derived, get } from 'svelte/store';
import type { Provider, Model, ProbeResult, ProviderConfig, UUID } from '../types';
import * as providerApi from '../api/provider';

// 供应商列表
export const providers = writable<Provider[]>([]);

// 当前选中的供应商
export const selectedProvider = writable<Provider | null>(null);

// 所有可用模型
export const availableModels = writable<Model[]>([]);

// 当前选中的模型
export const selectedModel = writable<Model | null>(null);

// 加载状态
export const isLoading = writable(false);

// 探活状态
export const isProbingProvider = writable<UUID | null>(null);

// 获取模型列表状态
export const isFetchingModels = writable<UUID | null>(null);

// 错误状态
export const providerError = writable<string | null>(null);

// 派生状态：已启用的供应商
export const enabledProviders = derived(
  providers,
  ($providers) => $providers.filter(p => p.enabled)
);

// 派生状态：可用的模型（来自已启用的供应商）
export const enabledModels = derived(
  [providers, availableModels],
  ([$providers, $availableModels]) => {
    const enabledProviderIds = $providers
      .filter(p => p.enabled)
      .map(p => p.id);
    
    return $availableModels.filter(m => 
      enabledProviderIds.includes(m.provider) && m.enabled
    );
  }
);

// 派生状态：按供应商分组的模型
export const modelsByProvider = derived(
  [providers, availableModels],
  ([$providers, $availableModels]) => {
    const groups: Record<string, { provider: Provider; models: Model[] }> = {};
    
    for (const provider of $providers) {
      const providerModels = $availableModels.filter(m => m.provider === provider.id);
      if (providerModels.length > 0) {
        groups[provider.id] = { provider, models: providerModels };
      }
    }
    
    return groups;
  }
);

/**
 * 供应商操作
 */
export const providerActions = {
  /**
   * 加载供应商列表
   */
  async loadProviders(): Promise<void> {
    try {
      isLoading.set(true);
      const providerList = await providerApi.getProviders();
      providers.set(providerList);
      
      // 收集所有模型
      const allModels = providerList.flatMap(p => p.models);
      availableModels.set(allModels);
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '加载供应商列表失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 创建供应商
   */
  async createProvider(config: ProviderConfig): Promise<Provider> {
    try {
      isLoading.set(true);
      const provider = await providerApi.createProvider(config);
      
      // 添加到列表
      providers.update(list => [...list, provider]);
      
      return provider;
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '创建供应商失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 更新供应商
   */
  async updateProvider(providerId: UUID, config: Partial<ProviderConfig>): Promise<void> {
    try {
      isLoading.set(true);
      const updatedProvider = await providerApi.updateProvider(providerId, config);
      
      // 更新列表中的供应商
      providers.update(list => 
        list.map(p => p.id === providerId ? updatedProvider : p)
      );
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '更新供应商失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 删除供应商
   */
  async deleteProvider(providerId: UUID): Promise<void> {
    try {
      isLoading.set(true);
      await providerApi.deleteProvider(providerId);
      
      // 从列表中移除
      providers.update(list => list.filter(p => p.id !== providerId));
      
      // 如果是当前选中的供应商，清空选择
      const current = get(selectedProvider);
      if (current && current.id === providerId) {
        selectedProvider.set(null);
      }
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '删除供应商失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 探活检测供应商
   */
  async probeProvider(providerId: UUID): Promise<ProbeResult> {
    try {
      isProbingProvider.set(providerId);
      const result = await providerApi.probeProvider(providerId);
      
      // 更新供应商的探活结果
      providers.update(list =>
        list.map(p => 
          p.id === providerId 
            ? { ...p, probeResult: result, lastProbeAt: Date.now() }
            : p
        )
      );
      
      return result;
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '探活检测失败');
      throw error;
    } finally {
      isProbingProvider.set(null);
    }
  },

  /**
   * 获取供应商模型列表
   */
  async fetchProviderModels(providerId: UUID, forceRefresh = false): Promise<void> {
    try {
      isFetchingModels.set(providerId);
      const response = await providerApi.getProviderModels({ 
        providerId, 
        forceRefresh 
      });
      
      // 更新供应商的模型列表
      providers.update(list =>
        list.map(p => 
          p.id === providerId 
            ? { ...p, models: response.models }
            : p
        )
      );
      
      // 更新全局模型列表
      availableModels.update(currentModels => {
        // 移除该供应商的旧模型，添加新模型
        const otherModels = currentModels.filter(m => m.provider !== providerId);
        return [...otherModels, ...response.models];
      });
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '获取模型列表失败');
      throw error;
    } finally {
      isFetchingModels.set(null);
    }
  },

  /**
   * 启用/禁用供应商
   */
  async toggleProvider(providerId: UUID, enabled: boolean): Promise<void> {
    try {
      const updatedProvider = await providerApi.toggleProvider(providerId, enabled);
      
      providers.update(list =>
        list.map(p => p.id === providerId ? updatedProvider : p)
      );
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '切换供应商状态失败');
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
      availableModels.update(list =>
        list.map(m => 
          m.id === modelId && m.provider === providerId 
            ? { ...m, enabled }
            : m
        )
      );
      
      // 更新供应商中的模型状态
      providers.update(list =>
        list.map(p => 
          p.id === providerId
            ? {
                ...p,
                models: p.models.map(m => 
                  m.id === modelId ? { ...m, enabled } : m
                )
              }
            : p
        )
      );
    } catch (error) {
      providerError.set(error instanceof Error ? error.message : '切换模型状态失败');
      throw error;
    }
  },

  /**
   * 选择供应商
   */
  selectProvider(provider: Provider | null): void {
    selectedProvider.set(provider);
  },

  /**
   * 选择模型
   */
  selectModel(model: Model | null): void {
    selectedModel.set(model);
  },

  /**
   * 根据模型ID查找模型
   */
  findModel(modelId: string): Model | undefined {
    const models = get(availableModels);
    return models.find(m => m.id === modelId);
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    providerError.set(null);
  },

  /**
   * 重置所有状态
   */
  reset(): void {
    providers.set([]);
    selectedProvider.set(null);
    availableModels.set([]);
    selectedModel.set(null);
    isLoading.set(false);
    isProbingProvider.set(null);
    isFetchingModels.set(null);
    providerError.set(null);
  }
};