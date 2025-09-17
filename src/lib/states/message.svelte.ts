/**
 * 消息状态管理 - 使用 Svelte 5 响应式最佳实践
 */

import type { Message, MessageResponse, MessageRequest, ChatAttachment, ChatMessage } from '$lib/types/chat';
import type { FrontendProviderConfig } from '$lib/types';
import * as messageApi from '$lib/api/message';
import { getProviderConfigById, getProviderIconById } from './provider.svelte';
import { chatState } from './chat.svelte';

interface MessageState {
  // 按 chatId 组织消息
  messagesByChat: Record<string, Message[]>;
  // providerId 到 providerConfig 的映射字典（用于快速获取 provider 图标等信息）
  providerConfigsCache: Record<string, FrontendProviderConfig>;
  isLoading: boolean;
  isSending: boolean;
  error: string | null;
  // 流式响应状态
  streamingMessageId: string | null;
  streamingContent: string;
  streamingReasoning: string;
}

class MessageStore {
  private state = $state<MessageState>({
    messagesByChat: {},
    providerConfigsCache: {},
    isLoading: false,
    isSending: false,
    error: null,
    streamingMessageId: null,
    streamingContent: '',
    streamingReasoning: '',
  });

  // 当前流式事件监听器的清理函数
  private currentStreamUnlisten: (() => void) | null = null;

  // Getters
  get isLoading() {
    return this.state.isLoading;
  }

  get isSending() {
    return this.state.isSending;
  }

  get error() {
    return this.state.error;
  }

  get streamingMessageId() {
    return this.state.streamingMessageId;
  }

  get streamingContent() {
    return this.state.streamingContent;
  }

  get streamingReasoning() {
    return this.state.streamingReasoning;
  }

  // 判断是否正在推理中（有推理内容但还没有最终内容）
  get isReasoning() {
    return this.state.streamingReasoning && !this.state.streamingContent;
  }

  // 判断是否在等待消息响应（发送中但还没有任何流式内容）
  get isMessageLoading() {
    return this.state.isSending && !this.state.streamingReasoning && !this.state.streamingContent;
  }

  // 响应式getter用于UI绑定 - 直接返回内部状态以确保响应性
  getMessagesReactive(chatId: string) {
    return this.state.messagesByChat[chatId] || [];
  }

  // 获取当前聊天的消息（通过外部传入 chatId）
  getCurrentMessages(currentChatId: string | undefined): Message[] {
    return currentChatId ? this.getMessages(currentChatId) : [];
  }

  // 根据 providerId 获取 providerConfig（带缓存）
  getProviderConfig(providerId: string): FrontendProviderConfig | undefined {
    // 先从缓存中查找
    if (this.state.providerConfigsCache[providerId]) {
      return this.state.providerConfigsCache[providerId];
    }

    // 缓存中没有，从 providerState 中获取
    const config = getProviderConfigById(providerId);
    if (config) {
      // 缓存结果
      this.state.providerConfigsCache[providerId] = config;
      return config;
    }

    return undefined;
  }

  // 根据 providerId 获取 provider 图标
  getProviderIcon(providerId: string): string | undefined {
    const config = this.getProviderConfig(providerId);
    return config?.icon || undefined;
  }

  // 批量缓存 providerConfigs（在加载消息时调用）
  private cacheProviderConfigs(messages: Message[]): void {
    const providerIds = new Set(messages.map(m => m.config?.providerId).filter(Boolean) as string[]);
    
    for (const providerId of providerIds) {
      if (!this.state.providerConfigsCache[providerId]) {
        const config = getProviderConfigById(providerId);
        if (config) {
          this.state.providerConfigsCache[providerId] = config;
        }
      }
    }
  }

  // 获取指定聊天的消息
  getMessages(chatId: string): Message[] {
    return this.state.messagesByChat[chatId] || [];
  }

  // 获取指定消息
  getMessage(chatId: string, messageId: string): Message | null {
    const messages = this.getMessages(chatId);
    return messages.find(m => m.id === messageId) || null;
  }

  // Actions
  setLoading(loading: boolean) {
    this.state.isLoading = loading;
  }

  setSending(sending: boolean) {
    this.state.isSending = sending;
  }

  setError(error: string | null) {
    this.state.error = error;
  }

  // 设置聊天的消息列表
  setMessages(chatId: string, messages: Message[]) {
    // 如果正在发送消息且本地已有消息，避免覆盖
    const existingMessages = this.state.messagesByChat[chatId] || [];
    if (this.isSending && existingMessages.length > 0 && messages.length === 0) {
      return;
    }

    this.state.messagesByChat[chatId] = messages;
    // 缓存消息中的 providerConfigs
    this.cacheProviderConfigs(messages);
  }

  // 添加消息到指定聊天
  addMessage(chatId: string, message: Message) {
    if (!this.state.messagesByChat[chatId]) {
      this.state.messagesByChat[chatId] = [];
    }
    this.state.messagesByChat[chatId].push(message);
    // 缓存新消息的 providerConfig
    this.cacheProviderConfigs([message]);
  }

  // 更新消息
  updateMessage(chatId: string, messageId: string, updates: Partial<Message>) {
    const messages = this.state.messagesByChat[chatId];
    if (messages) {
      const index = messages.findIndex(m => m.id === messageId);
      if (index !== -1) {
        messages[index] = { ...messages[index], ...updates };
      }
    }
  }

  // 删除消息
  deleteMessage(chatId: string, messageId: string) {
    const messages = this.state.messagesByChat[chatId];
    if (messages) {
      this.state.messagesByChat[chatId] = messages.filter(m => m.id !== messageId);
    }
  }

  // 开始流式响应
  startStreaming(messageId: string) {
    this.state.streamingMessageId = messageId;
    this.state.streamingContent = '';
    this.state.streamingReasoning = '';
  }

  // 更新流式内容
  appendStreamingContent(content: string) {
    this.state.streamingContent = content; // 直接设置完整内容，因为后端发送的是累积内容
  }

  // 设置流式内容
  setStreamingContent(content: string) {
    this.state.streamingContent = content;
  }

  // 设置流式推理过程
  setStreamingReasoning(reasoning: string) {
    this.state.streamingReasoning = reasoning;
  }

  // 完成流式响应
  finishStreaming(chatId: string, response: MessageResponse) {
    // 更新或创建消息
    const messages = this.state.messagesByChat[chatId] || [];
    const existingIndex = messages.findIndex(m => m.id === response.messageId);

    if (existingIndex !== -1) {
      // 更新现有消息
      messages[existingIndex] = {
        ...messages[existingIndex],
        content: response.content,
        reasoning: response.reasoning,
        inputTokens: response.inputTokens,
        outputTokens: response.outputTokens,
        totalTokens: response.totalTokens,
        duration: response.duration,
        updatedAt: Date.now(),
      };
    } else {
      // 创建新消息
      const newMessage: Message = {
        id: response.messageId,
        chatId: response.chatId,
        role: 'assistant',
        content: response.content,
        reasoning: response.reasoning,
        config: {
          modelId: response.modelId,
          providerId: response.providerId,
          stream: false,
        },
        inputTokens: response.inputTokens,
        outputTokens: response.outputTokens,
        totalTokens: response.totalTokens,
        duration: response.duration,
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };
      this.addMessage(chatId, newMessage);
    }

    // 清理流式状态
    this.state.streamingMessageId = null;
    this.state.streamingContent = '';
    this.state.streamingReasoning = '';
  }

  // 清理指定聊天的消息
  clearMessages(chatId: string) {
    delete this.state.messagesByChat[chatId];
  }

  // 清理所有消息
  clearAllMessages() {
    this.state.messagesByChat = {};
    this.state.streamingMessageId = null;
    this.state.streamingContent = '';
    this.state.streamingReasoning = '';
  }


  // API 操作方法
  
  /**
   * 加载指定聊天的消息
   */
  async loadMessages(chatId: string): Promise<void> {
    try {
      this.setLoading(true);
      this.setError(null);
      const messages = await messageApi.getMessages(chatId);
      this.setMessages(chatId, messages);

    } catch (error) {
      this.setError(error instanceof Error ? error.message : '加载消息失败');
      throw error;
    } finally {
      this.setLoading(false);
    }
  }

  /**
   * 发送消息（使用流式响应）- 简化版本，只需要内容和附件
   */
  async sendMessage(content: string, attachments: ChatAttachment[]): Promise<void> {
    // 获取当前聊天信息
    const currentChat = chatState.currentChat;
    if (!currentChat || !currentChat.id) {
      throw new Error('没有活跃的聊天');
    }

    if (!currentChat.modelId || !currentChat.providerId) {
      throw new Error('请先为当前聊天选择模型。如果供应商列表为空，请先配置AI供应商。');
    }

    try {
      this.setSending(true);
      this.setError(null);

      // 添加用户消息到本地状态
      const userMessage: Message = {
        id: crypto.randomUUID(),
        chatId: currentChat.id,
        role: 'user',
        content: content,
        config: {
          modelId: currentChat.modelId,
          providerId: currentChat.providerId,
          stream: true,
        },
        createdAt: Date.now(),
        updatedAt: Date.now()
      };

      this.addMessage(currentChat.id, userMessage);

      // 构建消息数组，如果有系统提示词则添加到开头
      const messages: ChatMessage[] = [];
      if (currentChat.systemPrompt && currentChat.systemPrompt.trim()) {
        messages.push({ role: 'system', content: currentChat.systemPrompt });
      }
      messages.push({ role: 'user', content: content });

      // 构建完整的消息请求
      const fullRequest: MessageRequest = {
        chatId: currentChat.id,
        modelId: currentChat.modelId,
        providerId: currentChat.providerId,
        messages: messages,
        attachments: attachments
      };

      // 设置流式响应参数
      const streamRequest = { ...fullRequest };

      // 清理之前的监听器（如果存在）
      if (this.currentStreamUnlisten) {
        this.currentStreamUnlisten();
        this.currentStreamUnlisten = null;
      }

      // 先设置流式事件监听器，确保在发送消息前完全就绪
      this.currentStreamUnlisten = await messageApi.listenToStreamEvents({
        onStart: (data) => {
          console.log('Stream started:', data);
          this.startStreaming(data.messageId);
        },
        onChunk: (data) => {
          this.setStreamingContent(data.content);
          if (data.reasoning) {
            // 累积推理过程内容，因为后端发送的是增量内容
            this.state.streamingReasoning += data.reasoning;
          }
        },
        onEnd: (data) => {
          console.log('Stream ended:', data);
          // 创建响应对象
          const response: MessageResponse = {
            chatId: data.chatId,
            messageId: data.streamId, // 使用 streamId 作为 messageId
            content: data.finalContent,
            reasoning: data.finalReasoning,
            modelId: data.modelId,
            providerId: data.providerId,
          };
          this.finishStreaming(currentChat.id!, response);
          this.setSending(false);
          // 流式完成后清理监听器
          if (this.currentStreamUnlisten) {
            this.currentStreamUnlisten();
            this.currentStreamUnlisten = null;
          }
        },
        onError: (error) => {
          console.error('Stream error:', error);
          this.setError('流式响应错误');
          this.setSending(false);
          // 错误时也清理监听器
          if (this.currentStreamUnlisten) {
            this.currentStreamUnlisten();
            this.currentStreamUnlisten = null;
          }
        }
      });

      // 事件监听器设置完成后，再发送流式消息
      const streamId = await messageApi.sendStreamMessage(streamRequest);
      console.log('Stream ID:', streamId);
      
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '发送消息失败');
      this.setSending(false);
      throw error;
    }
  }

  /**
   * 删除消息（API调用）
   */
  async removeMessage(chatId: string, messageId: string): Promise<void> {
    try {
      await messageApi.deleteMessage(messageId);
      this.deleteMessage(chatId, messageId);
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '删除消息失败');
      throw error;
    }
  }

  /**
   * 重新生成消息
   */
  async regenerateMessage(messageId: string): Promise<void> {
    try {
      this.setSending(true);
      await messageApi.regenerateMessage(messageId);
    } catch (error) {
      this.setError(error instanceof Error ? error.message : '重新生成失败');
      throw error;
    } finally {
      this.setSending(false);
    }
  }

  // 清理所有状态
  clear() {
    this.state.messagesByChat = {};
    this.state.providerConfigsCache = {};
    this.state.isLoading = false;
    this.state.isSending = false;
    this.state.error = null;
    this.state.streamingMessageId = null;
    this.state.streamingContent = '';
  }
}

// Export singleton instance
export const messageStore = new MessageStore();