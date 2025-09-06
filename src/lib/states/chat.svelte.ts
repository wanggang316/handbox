/**
 * 聊天相关状态管理 - Svelte 5 状态管理
 */

import type { 
  Chat, 
  Message, 
  ChatStreamEvent,
  UUID 
} from '../types';
import type { Model, ProviderWithModels, ModelWithProvider } from '../types/provider';
import * as chatApi from '../api/chat';
import * as messageApi from '../api/message';
import * as providerApi from '../api/provider';

// 聊天状态类
class ChatState {
  // 当前活跃聊天
  currentChat = $state<Chat | null>(null);
  
  // 聊天列表
  chats = $state<Chat[]>([]);
  
  // 当前聊天的消息列表
  messages = $state<Message[]>([]);
  
  // 加载状态
  isLoading = $state(false);
  
  // 流式输入状态
  isStreaming = $state(false);
  
  // 当前流式消息内容
  streamingContent = $state<string>('');
  
  // 错误状态
  chatError = $state<string | null>(null);

  // 模型和供应商相关状态
  providers = $state<ProviderWithModels[]>([]);
  selectedModel = $state<Model | null>(null);
  isLoadingProviders = $state(false);
  providerError = $state<string | null>(null);

  // 派生状态：是否有活跃聊天
  get hasActiveChat() {
    return this.currentChat !== null;
  }

  // 派生状态：当前聊天消息数量
  get messageCount() {
    return this.messages.length;
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
   * 选择模型
   */
  selectModel(model: Model): void {
    this.selectedModel = model;
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
  async createChat(name?: string): Promise<Chat> {
    console.log('Creating new chat:', name);
    console.log('Selected model:', this.selectedModel);
    try {
      this.isLoading = true;
      
      // 简化创建，暂时不传配置
      const chat = await chatApi.createChat(name ?? '未命名');
      
      // 更新聊天列表（归一化为数组后再拼接，避免展开不可迭代对象）
      const currentChats = Array.isArray(this.chats) ? this.chats : [];
      this.chats = [chat, ...currentChats];
      
      // 设置为当前聊天
      this.currentChat = chat;
      this.messages = [];
      
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
      
      const [chat, chatMessages] = await Promise.all([
        chatApi.getChat(chatId),
        messageApi.getMessages(chatId)
      ]);
      
      this.currentChat = chat;
      this.messages = chatMessages;
      
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '切换聊天失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 发送消息
   */
  async sendMessage(content: string): Promise<void> {
    const chat = this.currentChat;
    if (!chat) {
      throw new Error('没有活跃的聊天');
    }

    if (!this.selectedModel) {
      throw new Error('请先选择模型。如果供应商列表为空，请先配置AI供应商。');
    }

    console.log('Sending message to backend:', {
      chatId: chat.id,
      modelId: this.selectedModel!.id,
      providerId: this.selectedModel!.provider_id,
      messages: [{ role: 'user', content }]
    });

    try {
      this.isLoading = true;
      this.isStreaming = true;
      this.streamingContent = '';
      this.chatError = null;

      // 添加用户消息到本地状态
      const userMessage: Message = {
        id: crypto.randomUUID(),
        chatId: chat.id,
        role: 'user',
        content,
        modelId: this.selectedModel!.id,
        providerId: this.selectedModel!.provider_id,
        stream: true,
        createdAt: Date.now(),
        updatedAt: Date.now()
      };
      
      // 归一化为数组后追加，避免展开不可迭代对象
      const currentMessages = Array.isArray(this.messages) ? this.messages : [];
      this.messages = [...currentMessages, userMessage];

      // 发送消息到后端 - 简化请求
      if (this.selectedModel) {
        console.log('Sending message to backend:', {
          chatId: chat.id,
          modelId: this.selectedModel!.id,
          providerId: this.selectedModel!.provider_id,
          messages: [{ role: 'user', content }]
        });
        await messageApi.sendMessage({
          chatId: chat.id,
          modelId: this.selectedModel!.id,
          providerId: this.selectedModel!.provider_id,
          messages: [{ role: 'user', content }],
          attachments: []
        });
      } else {
        throw new Error('请先选择模型');
      }
      
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '发送消息失败';
      this.isStreaming = false;
      throw error;
    } finally {
      this.isLoading = false;
    }
  }


  /**
   * 删除消息
   */
  async deleteMessage(messageId: UUID): Promise<void> {
    try {
      await messageApi.deleteMessage(messageId);
      this.messages = this.messages.filter(msg => msg.id !== messageId);
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '删除消息失败';
      throw error;
    }
  }

  /**
   * 重新生成消息
   */
  async regenerateMessage(messageId: UUID): Promise<void> {
    try {
      this.isStreaming = true;
      await messageApi.regenerateMessage(messageId);
      // 流式响应会通过事件处理
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '重新生成失败';
      this.isStreaming = false;
      throw error;
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
    try {
      await Promise.all([
        this.loadProviders(),
        this.loadChats()
      ]);
    } catch (error) {
      console.error('Failed to initialize chat state:', error);
    }
  }

  /**
   * 重置所有状态
   */
  reset(): void {
    this.currentChat = null;
    this.chats = [];
    this.messages = [];
    this.isLoading = false;
    this.isStreaming = false;
    this.streamingContent = '';
    this.chatError = null;
    
    this.providers = [];
    this.selectedModel = null;
    this.isLoadingProviders = false;
    this.providerError = null;
  }
}

// 创建全局状态实例
export const chatState = new ChatState();