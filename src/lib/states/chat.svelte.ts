/**
 * 聊天相关状态管理 - Svelte 5 状态管理
 */

import type {
  Chat,
  UUID,
  McpServerConfig
} from '../types';
import type { ModelWithProvider } from '../types/provider';
import * as chatApi from '../api/chat';
import { providerActions, getAllModels, providerState } from './provider.svelte';

// 聊天状态 - 使用 Svelte 5 runes
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
   * 切换模型收藏状态（委托给 providerActions）
   */
  async toggleModelFavorite(providerId: string, modelId: string, favorite: boolean): Promise<void> {
    return providerActions.toggleModelFavorite(providerId, modelId, favorite);
  },

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
        mcpServers: currentChat.mcpServers
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
    maxTokens?: number;
    stream?: boolean;
    contextLength?: number;
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
          mcpServers: currentChat.mcpServers
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