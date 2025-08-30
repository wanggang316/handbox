/**
 * 供应商相关状态管理 - 使用 Svelte 5 runes
 */

import type { Provider, Model, ProbeResult, ProviderConfig, UUID } from '../types';
import * as providerApi from '../api/provider';

// 预定义供应商模板
export const preProviders = [
  {
    name: "OpenAI",
    provider_type: "openai" as const,
    iconSrc: "/logo-openai.png",
    base_url_placeholder: "https://api.openai.com/v1",
  },
  {
    name: "Anthropic",
    provider_type: "anthropic" as const,
    iconSrc: "/logo-anthropic.png",
    base_url_placeholder: "https://api.anthropic.com",
  },
  {
    name: "Google AI",
    provider_type: "google" as const,
    iconSrc: "/logo-google.png",
    base_url_placeholder: "https://generativelanguage.googleapis.com/v1",
  },
  {
    name: "DeepSeek",
    provider_type: "deepseek" as const,
    iconSrc: "/logo-deepseek.png",
    base_url_placeholder: "https://api.deepseek.com",
  },
  {
    name: "OpenRouter",
    provider_type: "openrouter" as const,
    iconSrc: "/logo-openrouter.png",
    base_url_placeholder: "https://openrouter.ai/api/v1",
  },
];

// 获取预定义供应商信息的工具函数
export function getPreProvider(provider: Provider): typeof preProviders[0] | undefined {
  return preProviders.find(t => t.name === provider.name);
}

// 获取供应商图标
export function getProviderIcon(provider: Provider): string | undefined {
  const template = getPreProvider(provider);
  return template?.iconSrc || undefined;
}

// 全局状态对象
export const providerState = $state({
  // 供应商列表
  providers: [] as Provider[],
  
  // 当前选中的供应商
  selectedProvider: null as Provider | null,
  
  // 所有可用模型
  availableModels: [] as Model[],
  
  // 当前选中的模型
  selectedModel: null as Model | null,
  
  // 加载状态
  isLoading: false,
  
  // 探活状态
  isProbingProvider: null as UUID | null,
  
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

/**
 * 供应商操作
 */
export const providerActions = {
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
      if (providerState.selectedProvider?.id === providerId) {
        providerState.selectedProvider = null;
      }
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '删除供应商失败';
      throw error;
    } finally {
      providerState.isLoading = false;
    }
  },

  /**
   * 探活检测供应商
   */
  async probeProvider(providerId: UUID): Promise<ProbeResult> {
    try {
      providerState.isProbingProvider = providerId;
      const result = await providerApi.probeProvider(providerId);
      
      // 更新供应商的探活结果
      const index = providerState.providers.findIndex(p => p.id === providerId);
      if (index !== -1) {
        providerState.providers[index] = { 
          ...providerState.providers[index], 
          probe_result: result, 
          last_probe_at: Date.now() 
        };
      }
      
      return result;
    } catch (error) {
      providerState.error = error instanceof Error ? error.message : '探活检测失败';
      throw error;
    } finally {
      providerState.isProbingProvider = null;
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
   * 选择供应商
   */
  selectProvider(provider: Provider | null): void {
    providerState.selectedProvider = provider;
  },

  /**
   * 选择模型
   */
  selectModel(model: Model | null): void {
    providerState.selectedModel = model;
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
    providerState.selectedProvider = null;
    providerState.availableModels = [];
    providerState.selectedModel = null;
    providerState.isLoading = false;
    providerState.isProbingProvider = null;
    providerState.isFetchingModels = null;
    providerState.error = null;
  }
};