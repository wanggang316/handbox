/**
 * 聊天相关状态管理 - Svelte 5 状态管理
 */

import type {
  Chat,
  UUID,
  McpServerConfig
} from '../types';
import type { ChatMethodResponse, ModelWithProvider } from '../types/provider';
import * as chatApi from '../api/chat';
import { providerActions, getAllModels, providerState } from './provider.svelte';

// ============================================
// 模型参数管理 - 共享工具函数和常量
// ============================================

/**
 * 参数别名映射，用于处理不同供应商的参数命名差异
 */
const PARAMETER_ALIASES: Record<string, string[]> = {
  temperature: ["temperature"],
  top_p: ["top_p"],
  top_k: ["top_k"],
  streaming: ["streaming", "stream"],
  output_max_tokens: ["output_max_tokens", "max_tokens"],
};

function getPrimaryChatMethod(model?: ModelWithProvider): ChatMethodResponse | null {
  if (!model) {
    return null;
  }

  return model.chat_method ?? null;
}

function getMethodParameters(model?: ModelWithProvider) {
  const method = getPrimaryChatMethod(model);
  if (!method || !Array.isArray(method.parameters)) {
    return [];
  }
  return method.parameters;
}

function findMethodParameter(parameterName: string, model?: ModelWithProvider) {
  const params = getMethodParameters(model);
  if (params.length === 0) {
    return null;
  }

  for (const alias of getParameterAliases(parameterName)) {
    const entry = params.find((param) => param.name === alias);
    if (entry) {
      return entry;
    }
  }

  return null;
}

/**
 * 获取参数的所有别名
 */
export function getParameterAliases(name: string): string[] {
  return PARAMETER_ALIASES[name] ?? [name];
}

/**
 * 将值转换为数字
 */
export function toNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

/**
 * 将值转换为布尔值
 */
export function toBoolean(value: unknown): boolean | null {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes"].includes(normalized)) return true;
    if (["false", "0", "no"].includes(normalized)) return false;
  }
  return null;
}

/**
 * 获取模型支持的参数集合
 */
export function getSupportedParameterSet(model?: ModelWithProvider): Set<string> {
  const supported = new Set<string>();

  if (!model) {
    return supported;
  }

  for (const param of getMethodParameters(model)) {
    const name = typeof param?.name === "string" ? param.name.trim() : "";
    if (name) {
      supported.add(name);
    }
  }

  const supportList = Array.isArray(model.supported_parameters) ? model.supported_parameters : [];

  for (const raw of supportList) {
    if (typeof raw === "string" && raw.trim().length > 0) {
      supported.add(raw.trim());
    }
  }

  if (supported.size === 0 && Array.isArray(model.parameters)) {
    for (const param of model.parameters) {
      const name = typeof param?.name === "string" ? param.name.trim() : "";
      if (name) {
        supported.add(name);
      }
    }
  }

  return supported;
}

/**
 * 检查模型是否支持指定参数
 */
export function hasParameterSupport(parameterName: string, model?: ModelWithProvider): boolean {
  // temperature 是基础参数，默认支持
  if (parameterName === "temperature") {
    return true;
  }

  const supportedParameters = getSupportedParameterSet(model);
  if (supportedParameters.size > 0 && getParameterAliases(parameterName).some((alias) => supportedParameters.has(alias))) {
    return true;
  }

  const entry = findMethodParameter(parameterName, model);
  if (entry) {
    if (entry.support) {
      return true;
    }

    if (entry.values) {
      const hasValue =
        toNumber(entry.values.default ?? null) !== null ||
        toNumber(entry.values.max ?? null) !== null ||
        toNumber(entry.values.min ?? null) !== null;

      if (hasValue) {
        return true;
      }
    }
  }

  return false;
}

/**
 * 获取参数的默认数值
 */
export function getDefaultNumber(parameterName: string, fallback: number, model?: ModelWithProvider): number {
  const entry = findMethodParameter(parameterName, model);
  if (entry?.values) {
    const parsed = toNumber(entry.values.default ?? null);
    if (parsed !== null) {
      return parsed;
    }
  }

  return fallback;
}

/**
 * 获取参数的默认布尔值
 */
export function getDefaultBoolean(parameterName: string, fallback: boolean, model?: ModelWithProvider): boolean {
  const entry = findMethodParameter(parameterName, model);
  if (entry?.values) {
    const parsed = toBoolean(entry.values.default ?? null);
    if (parsed !== null) {
      return parsed;
    }
  }

  return fallback;
}

/**
 * 获取参数的最大值
 */
export function getMaxNumber(parameterName: string, fallback: number, model?: ModelWithProvider): number {
  const entry = findMethodParameter(parameterName, model);
  if (entry?.values) {
    const parsed = toNumber(entry.values.max ?? null);
    if (parsed !== null) {
      return parsed;
    }
  }

  return fallback;
}

/**
 * 确保值是有效数字
 */
export function ensureNumber(value: number | null | undefined, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/**
 * 将值限制在指定范围内
 */
export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  if (!Number.isFinite(max) || max <= min) return value < min ? min : value;
  return Math.min(Math.max(value, min), max);
}

/**
 * 获取模型的默认设置
 */
export function getModelDefaultSettings(model?: ModelWithProvider) {
  const outputFallback = getDefaultNumber(
    "output_max_tokens",
    getDefaultNumber("max_tokens", 4000, model),
    model
  );

  return {
    temperature: getDefaultNumber("temperature", 0.7, model),
    topP: getDefaultNumber("top_p", 1.0, model),
    topK: hasParameterSupport("top_k", model)
      ? Math.max(getDefaultNumber("top_k", 40, model), 1)
      : 0,
    streamResponse: hasParameterSupport("streaming", model)
      ? getDefaultBoolean("streaming", true, model)
      : true,
    maxTokens: outputFallback > 0 ? outputFallback : 4000,
  };
}

// ============================================
// 聊天状态 - 使用 Svelte 5 runes
// ============================================
let currentChat = $state<Chat | null>(null);
let chats = $state<Chat[]>([]);
let isLoading = $state(false);
let chatError = $state<string | null>(null);
let isInitialized = $state(false);
let isInitializing = $state(false);

export const chatState = {
  get currentChat() { return currentChat; },
  set currentChat(value) { currentChat = value; },

  get chats() { return chats; },
  set chats(value) { chats = value; },

  get isLoading() { return isLoading; },
  set isLoading(value) { isLoading = value; },

  get chatError() { return chatError; },
  set chatError(value) { chatError = value; },

  get isInitialized() { return isInitialized; },
  set isInitialized(value) { isInitialized = value; },

  get isInitializing() { return isInitializing; },
  set isInitializing(value) { isInitializing = value; }
};

// 派生状态：是否有活跃聊天
export function hasActiveChat(): boolean {
  return currentChat !== null && currentChat?.id !== undefined;
}

// 派生状态：当前聊天的模型信息
export function currentChatModel(): { model?: ModelWithProvider } {

  console.log('currentChatModel >>> :', currentChat);
  if (!currentChat) {
    return {};
  }

  const modelId = currentChat.modelId;
  const providerId = currentChat.providerId;

  if (!modelId || !providerId) {
    return {};
  }

  const model = getAllModels().find(m => m.id === modelId && m.provider_id === providerId);

  return {
    model
  };
}


// 聊天操作函数
export const chatActions = {

  /**
   * 更新聊天配置
   */
  async updateChatSettings(settings: Partial<Chat>): Promise<void> {
    if (!currentChat?.id) {
      // 如果没有保存的聊天，只更新本地状态
      if (currentChat) {
        Object.assign(currentChat, settings);
      }
      return;
    }

    try {
      // 更新本地状态
      Object.assign(currentChat, settings);

      // 更新后端
      await chatApi.updateChat(currentChat.id, {
        name: currentChat.name,
        modelId: currentChat.modelId,
        providerId: currentChat.providerId,
        temperature: currentChat.temperature,
        topP: currentChat.topP,
        maxTokens: currentChat.maxTokens,
        stream: currentChat.stream,
        systemPrompt: currentChat.systemPrompt,
        mcpServers: currentChat.mcpServers,
        turnCount: currentChat.turnCount
      });
    } catch (error) {
      // 回滚本地状态
      await chatActions.loadChats();
      throw error;
    }
  },

  /**
   * 更新系统提示词
   */
  async updateSystemPrompt(systemPrompt: string): Promise<void> {
    return chatActions.updateChatSettings({ systemPrompt });
  },


  /**
   * 更新模型参数
   */
  async updateModelSettings(settings: {
    temperature?: number;
    topP?: number;
    topK?: number;
    maxTokens?: number;
    stream?: boolean;
  }): Promise<void> {
    return chatActions.updateChatSettings(settings);
  },

  /**
   * 更新MCP服务器配置
   */
  async updateMcpServers(mcpServers: McpServerConfig[]): Promise<void> {
    return chatActions.updateChatSettings({ mcpServers });
  },

  /**
   * 更新当前聊天的模型信息并保存到后端
   */
  async updateChatModel(modelId: string, providerId: string): Promise<void> {
    // 如果没有 currentChat，创建一个临时的 chat 对象来保存模型选择
    // 实际的数据库保存会在用户发送消息时通过 createChat 完成
    console.log("modelid: {}, providerId: {}", modelId, providerId);
    if (!currentChat) {
      console.log('No current chat, creating temporary chat object for model selection');
      currentChat = {
        name: '未命名',
        messageCount: 0,
        modelId,
        providerId,
        temperature: 0.7,
        topP: 1.0,
        maxTokens: 4000,
        stream: true,
        systemPrompt: '',
        mcpServers: [],
        createdAt: Date.now(),
        updatedAt: Date.now()
      } as Chat;

      console.log('Temporary chat object created:', currentChat);
      return;
    }

    // 更新本地状态
    currentChat.modelId = modelId;
    currentChat.providerId = providerId;

    // 如果已经有 id（已保存到后端），则更新后端
    if (currentChat.id) {
      try {
        await chatApi.updateChat(currentChat.id, {
          name: currentChat.name,
          modelId,
          providerId,
          temperature: currentChat.temperature,
          topP: currentChat.topP,
          maxTokens: currentChat.maxTokens,
          stream: currentChat.stream,
          systemPrompt: currentChat.systemPrompt,
          mcpServers: currentChat.mcpServers,
          turnCount: currentChat.turnCount
        });
      } catch (error) {
        // 回滚本地状态
        await chatActions.loadChats(); // 重新加载以恢复状态
        throw error;
      }
    }
  },

  /**
   * 加载聊天列表
   */
  async loadChats(): Promise<void> {
    try {
      isLoading = true;
      const chatList = await chatApi.getChats();
      chats = chatList;
    } catch (error) {
      chatError = error instanceof Error ? error.message : '加载聊天列表失败';
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 创建新聊天
   */
  async createChat(name: string): Promise<Chat> {

    try {

      if (!currentChat) {
        throw new Error('没有当前聊天');
      }

      if (!currentChat.modelId || !currentChat.providerId) {
        throw new Error('没有设置模型信息');
      }

      isLoading = true;

      // 使用完整参数创建聊天，一次性包含所有信息
      const chat = await chatApi.createChat(
        name,
        0.7, // temperature
        1.0, // topP
        4000, // maxTokens
        true, // stream
        currentChat.modelId,
        currentChat.providerId,
        '', // systemPrompt
        [] // mcpServers
      );

      // 更新聊天列表（归一化为数组后再拼接，避免展开不可迭代对象）
      const currentChats = Array.isArray(chats) ? chats : [];
      chats = [chat, ...currentChats];

      // 设置为当前聊天
      currentChat = chat;

      console.log('Created chat:', chat);
      return chat;
    } catch (error) {
      chatError = error instanceof Error ? error.message : '创建聊天失败';
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 切换到指定聊天
   */
  async switchToChat(chatId: UUID): Promise<void> {
    try {
      isLoading = true;

      // 确保模型数据已经加载完成
      if (providerState.providersWithModels.length === 0) {
        await providerActions.loadProvidersWithModels();
      }

      const chat = await chatApi.getChat(chatId);
      currentChat = chat;
    } catch (error) {
      chatError = error instanceof Error ? error.message : '切换聊天失败';
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    chatError = null;
  },

  /**
   * 初始化状态
   */
  async initialize(): Promise<void> {
    // 避免重复初始化
    if (isInitialized || isInitializing) {
      return;
    }

    try {
      isInitializing = true;

      await Promise.all([
        providerActions.loadProvidersWithModels(),
        chatActions.loadChats()
      ]);

      isInitialized = true;
      console.log('Chat state initialized successfully');
    } catch (error) {
      console.error('Failed to initialize chat state:', error);
    } finally {
      isInitializing = false;
    }
  },

  /**
   * 删除聊天
   */
  async deleteChat(chatId: UUID): Promise<void> {
    try {
      isLoading = true;

      // 调用后端删除聊天
      await chatApi.deleteChat(chatId);

      // 从本地状态中移除聊天
      chats = chats.filter(chat => chat.id !== chatId);

      // 如果删除的是当前聊天，清空当前聊天状态
      if (currentChat?.id === chatId) {
        currentChat = null;
      }

      console.log('Chat deleted:', chatId);
    } catch (error) {
      chatError = error instanceof Error ? error.message : '删除聊天失败';
      throw error;
    } finally {
      isLoading = false;
    }
  },

  /**
   * 重命名聊天（包括手动重命名和自动标题生成）
   */
  async renameChat(chatId: UUID, newName: string): Promise<void> {
    try {
      // 调用后端更新聊天名称
      const updatedChat = await chatApi.updateChat(chatId, { name: newName });

      // 更新本地状态
      const chatIndex = chats.findIndex(chat => chat.id === chatId);
      if (chatIndex !== -1) {
        chats[chatIndex] = updatedChat;
      }

      // 如果是当前聊天，也更新当前聊天状态
      if (currentChat?.id === chatId) {
        currentChat = updatedChat;
      }

      console.log('Chat renamed:', chatId, newName);
    } catch (error) {
      chatError = error instanceof Error ? error.message : '重命名聊天失败';
      throw error;
    }
  },

  /**
   * 重置所有状态
   */
  reset(): void {
    currentChat = null;
    chats = [];
    isLoading = false;
    chatError = null;

    isInitialized = false;
    isInitializing = false;
  }
};
