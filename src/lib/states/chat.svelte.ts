/**
 * 聊天相关状态管理 - Svelte 5 状态管理
 */

import type { 
  ChatSession, 
  Message, 
  ChatConfig,
  ChatStreamEvent,
  UUID 
} from '../types';
import type { Model, ProviderWithModels, ModelWithProvider } from '../types/provider';
import * as chatApi from '../api/chat';
import * as providerApi from '../api/provider';

// 聊天状态类
class ChatState {
  // 当前活跃会话
  currentSession = $state<ChatSession | null>(null);
  
  // 会话列表
  sessions = $state<ChatSession[]>([]);
  
  // 当前会话的消息列表
  messages = $state<Message[]>([]);
  
  // 聊天配置
  chatConfig = $state<ChatConfig | null>(null);
  
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

  // 派生状态：是否有活跃会话
  get hasActiveSession() {
    return this.currentSession !== null;
  }

  // 派生状态：当前会话消息数量
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
      
      // 收藏模型列表通过派生状态自动更新
      
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
    
    // 如果有活跃会话，更新会话配置
    if (this.currentSession && this.chatConfig) {
      this.updateConfig({
        model: model.id,
        provider: model.provider_id
      });
    }
  }

  /**
   * 加载会话列表
   */
  async loadSessions(): Promise<void> {
    try {
      this.isLoading = true;
      const sessionList = await chatApi.getChatSessions();
      this.sessions = sessionList;
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '加载会话列表失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 创建新会话
   */
  async createSession(name?: string, config?: Partial<ChatConfig>): Promise<ChatSession> {
    try {
      this.isLoading = true;
      
      // 如果有选中的模型，使用该模型配置
      const sessionConfig = config || {};
      if (this.selectedModel) {
        sessionConfig.model = this.selectedModel.id;
        sessionConfig.provider = this.selectedModel.provider_id;
      }
      
      const session = await chatApi.createChatSession(name, sessionConfig);
      
      // 更新会话列表
      this.sessions = [session, ...this.sessions];
      
      // 设置为当前会话
      this.currentSession = session;
      this.messages = [];
      this.chatConfig = session.config;
      
      return session;
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '创建会话失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 切换到指定会话
   */
  async switchToSession(sessionId: UUID): Promise<void> {
    try {
      this.isLoading = true;
      
      const [session, sessionMessages] = await Promise.all([
        chatApi.getChatSession(sessionId),
        chatApi.getChatMessages(sessionId)
      ]);
      
      this.currentSession = session;
      this.messages = sessionMessages;
      this.chatConfig = session.config;
      
      // 根据会话配置设置选中的模型
      if (session.config.model) {
        const model = this.allModels.find(m => 
          m.id === session.config.model && m.provider_id === session.config.provider
        );
        if (model) {
          this.selectedModel = model;
        }
      }
      
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '切换会话失败';
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 发送消息
   */
  async sendMessage(content: string, attachments?: File[]): Promise<void> {
    const session = this.currentSession;
    if (!session) {
      throw new Error('没有活跃的会话');
    }

    try {
      this.isLoading = true;
      this.isStreaming = true;
      this.streamingContent = '';
      this.chatError = null;

      // 添加用户消息到本地状态
      const userMessage: Message = {
        id: crypto.randomUUID(),
        sessionId: session.id,
        role: 'user',
        content,
        createdAt: Date.now(),
        updatedAt: Date.now()
      };
      
      this.messages = [...this.messages, userMessage];

      // 监听流式响应
      const unlisten = await chatApi.listenChatStream(session.id, (event: ChatStreamEvent) => {
        switch (event.type) {
          case 'delta':
            this.streamingContent += event.data.content;
            break;
            
          case 'done':
            // 添加完整的助手消息
            const assistantMessage: Message = {
              id: event.data.messageId,
              sessionId: event.data.sessionId,
              role: 'assistant',
              content: event.data.content,
              metadata: event.data.metadata,
              createdAt: Date.now(),
              updatedAt: Date.now()
            };
            
            this.messages = [...this.messages, assistantMessage];
            this.streamingContent = '';
            this.isStreaming = false;
            unlisten();
            break;
            
          case 'error':
            this.chatError = event.data.error;
            this.isStreaming = false;
            unlisten();
            break;
        }
      });

      // 发送消息到后端
      await chatApi.sendChatMessage({
        sessionId: session.id,
        messages: [{ role: 'user', content }],
        attachments
      });
      
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '发送消息失败';
      this.isStreaming = false;
      throw error;
    } finally {
      this.isLoading = false;
    }
  }

  /**
   * 更新聊天配置
   */
  async updateConfig(newConfig: Partial<ChatConfig>): Promise<void> {
    const session = this.currentSession;
    if (!session) return;

    try {
      const updatedSession = await chatApi.updateChatSession(session.id, {
        config: { ...session.config, ...newConfig }
      });
      
      this.currentSession = updatedSession;
      this.chatConfig = updatedSession.config;
    } catch (error) {
      this.chatError = error instanceof Error ? error.message : '更新配置失败';
      throw error;
    }
  }

  /**
   * 删除消息
   */
  async deleteMessage(messageId: UUID): Promise<void> {
    try {
      await chatApi.deleteMessage(messageId);
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
      await chatApi.regenerateMessage(messageId);
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
        this.loadSessions()
      ]);
    } catch (error) {
      console.error('Failed to initialize chat state:', error);
    }
  }

  /**
   * 重置所有状态
   */
  reset(): void {
    this.currentSession = null;
    this.sessions = [];
    this.messages = [];
    this.chatConfig = null;
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