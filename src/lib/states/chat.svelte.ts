/**
 * 聊天相关状态管理 - Svelte 5 状态管理
 */

import type { 
  Chat, 
  UUID 
} from '../types';
import type { ProviderWithModels, ModelWithProvider } from '../types/provider';
import * as chatApi from '../api/chat';
import * as providerApi from '../api/provider';

// 聊天状态类
class ChatState {
  // 当前活跃聊天
  currentChat = $state<Chat | null>(null);
  
  // 聊天列表
  chats = $state<Chat[]>([]);
  
  // 加载状态
  isLoading = $state(false);
  
  // 错误状态
  chatError = $state<string | null>(null);

  // 模型和供应商相关状态
  providers = $state<ProviderWithModels[]>([]);
  isLoadingProviders = $state(false);
  providerError = $state<string | null>(null);

  // 初始化状态
  isInitialized = $state(false);
  isInitializing = $state(false);

  // 派生状态：是否有活跃聊天
  get hasActiveChat() {
    return this.currentChat !== null;
  }


  // 派生状态：所有可用模型（带供应商信息）
  get allModels(): ModelWithProvider[] {
    return this.providers.flatMap(provider => 
      provider.models.map(model => ({
        ...model,
        providerName: provider.name,
        providerType: provider.provider_type
      }))
    );
  }

  // 派生状态：收藏模型
  get favoriteModels(): ModelWithProvider[] {
    return this.allModels.filter(model => model.favorite);
  }

  // 派生状态：当前聊天的模型信息（直接从 chat 获取）
  get currentChatModel(): { model?: ModelWithProvider } {
    if (!this.currentChat) {
      return {};
    }

    const modelId = this.currentChat.modelId;
    const providerId = this.currentChat.providerId;
    
    if (!modelId || !providerId) {
      return {};
    }

    const model = this.allModels.find(m => m.id === modelId && m.provider_id === providerId);

    return {
      model
    };
  }

  /**
   * 加载所有供应商和模型
   */
  async loadProviders(forceRefresh = false): Promise<void> {
    try {
      this.isLoadingProviders = true;
      this.providerError = null;
      
      const providersWithModels = await providerApi.getProvidersWithModels(forceRefresh);
      this.providers = providersWithModels;
      
    } catch (error) {
      this.providerError = error instanceof Error ? error.message : '加载供应商列表失败';
      throw error;
    } finally {
      this.isLoadingProviders = false;
    }
  }

  /**
   * 切换模型收藏状态
   */
  async toggleModelFavorite(providerId: string, modelId: string, favorite: boolean): Promise<void> {
    try {
      await providerApi.toggleModelFavorite(providerId, modelId, favorite);
      
      // 更新本地状态
      const provider = this.providers.find(p => p.id === providerId);
      if (provider) {
        const model = provider.models.find(m => m.id === modelId);
        if (model) {
          model.favorite = favorite;
          // favoriteModels 是派生状态，会自动更新，无需手动赋值
        }
      }
    } catch (error) {
      this.providerError = error instanceof Error ? error.message : '更新收藏状态失败';
      throw error;
    }
  }

  /**
   * 更新当前聊天的模型信息并保存到后端
   */
  async updateChatModel(modelId: string, providerId: string): Promise<void> {
    if (!this.currentChat) {
      throw new Error('No current chat selected');
    }

    try {
      // 更新本地状态
      this.currentChat.modelId = modelId;
      this.currentChat.providerId = providerId;

      // 保存到后端
      await chatApi.updateChat(this.currentChat.id, {
        name: this.currentChat.name,
        modelId,
        providerId,
        temperature: this.currentChat.temperature,
        topP: this.currentChat.topP,
        maxTokens: this.currentChat.maxTokens,
        stream: this.currentChat.stream,
        systemPrompt: this.currentChat.systemPrompt,
        mcpServers: this.currentChat.mcpServers
      });
    } catch (error) {
      // 回滚本地状态
      this.loadChats(); // 重新加载以恢复状态
      throw error;
    }
  }

  /**
   * 加载聊天列表
   */
  async loadChats(): Promise<void> {
    try {
      this.isLoading = true;
      const chatList = await chatApi.getChats();
      this.chats = chatList;
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '加载聊天列表失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 创建新聊天
   */
  async createChat(name?: string, modelId?: string, providerId?: string): Promise<Chat> {
    console.log('Creating new chat:', name, 'with model:', modelId, providerId);
    try {
      this.isLoading = true;
      
      // 简化创建，暂时不传配置
      const chat = await chatApi.createChat(name ?? '未命名');
      
      // 设置模型信息到聊天
      if (modelId && providerId) {
        chat.modelId = modelId;
        chat.providerId = providerId;
      }
      
      // 更新聊天列表（归一化为数组后再拼接，避免展开不可迭代对象）
      const currentChats = Array.isArray(this.chats) ? this.chats : [];
      this.chats = [chat, ...currentChats];
      
      // 设置为当前聊天
      this.currentChat = chat;
            
      console.log('Created chat:', chat);
      console.log('Current chat:', this.currentChat);
      console.log('chats:', this.chats);

      return chat;
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '创建聊天失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 切换到指定聊天
   */
  async switchToChat(chatId: UUID): Promise<void> {
    try {
      this.isLoading = true;
      
      const chat = await chatApi.getChat(chatId);
      this.currentChat = chat;
            
      console.log('Current chat >>> :', this.currentChat);
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '切换聊天失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }


  /**
   * 清除错误状态
   */
  clearError(): void {
    this.chatError = null;
    this.providerError = null;
  }

  /**
   * 初始化状态
   */
  async initialize(): Promise<void> {
    // 避免重复初始化
    if (this.isInitialized || this.isInitializing) {
      return;
    }

    try {
      this.isInitializing = true;
      
      await Promise.all([
        this.loadProviders(),
        this.loadChats()
      ]);
      
      this.isInitialized = true;
      console.log('Chat state initialized successfully');
    } catch (error) {
      console.error('Failed to initialize chat state:', error);
    } finally {
      this.isInitializing = false;
    }
  }

  /**
   * 重置所有状态
   */
  reset(): void {
    this.currentChat = null;
    this.chats = [];
    this.isLoading = false;
    this.chatError = null;
    
    this.providers = [];
    this.isLoadingProviders = false;
    this.providerError = null;
    
    this.isInitialized = false;
    this.isInitializing = false;
  }
}

// 创建全局状态实例
export const chatState = new ChatState();