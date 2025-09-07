/**
 * 消息状态管理
 */

import type { Message, ChatResponse, ChatRequest } from '$lib/types/chat';
import * as messageApi from '$lib/api/message';

interface MessageState {
  // 按 chatId 组织消息
  messagesByChat: Record<string, Message[]>;
  isLoading: boolean;
  isSending: boolean;
  error: string | null;
  // 流式响应状态
  streamingMessageId: string | null;
  streamingContent: string;
}

class MessageStore {
  private state = $state<MessageState>({
    messagesByChat: {},
    isLoading: false,
    isSending: false,
    error: null,
    streamingMessageId: null,
    streamingContent: '',
  });

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
    this.state.messagesByChat[chatId] = messages;
  }

  // 添加消息到指定聊天
  addMessage(chatId: string, message: Message) {
    if (!this.state.messagesByChat[chatId]) {
      this.state.messagesByChat[chatId] = [];
    }
    this.state.messagesByChat[chatId].push(message);
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
  }

  // 更新流式内容
  appendStreamingContent(content: string) {
    this.state.streamingContent = content; // 直接设置完整内容，因为后端发送的是累积内容
  }

  // 设置流式内容
  setStreamingContent(content: string) {
    this.state.streamingContent = content;
  }

  // 完成流式响应
  finishStreaming(chatId: string, response: ChatResponse) {
    // 更新或创建消息
    const messages = this.state.messagesByChat[chatId] || [];
    const existingIndex = messages.findIndex(m => m.id === response.messageId);
    
    if (existingIndex !== -1) {
      // 更新现有消息
      messages[existingIndex] = {
        ...messages[existingIndex],
        content: response.content,
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
   * 发送消息（使用流式响应）
   */
  async sendMessage(request: ChatRequest): Promise<void> {
    if (!request.chatId) {
      throw new Error('缺少聊天ID');
    }

    try {
      this.setSending(true);
      this.setError(null);
      
      // 添加用户消息到本地状态
      const userMessage: Message = {
        id: crypto.randomUUID(),
        chatId: request.chatId,
        role: 'user',
        content: request.messages[0]?.content || '',
        config: {
          modelId: request.modelId,
          providerId: request.providerId,
          stream: true,
        },
        createdAt: Date.now(),
        updatedAt: Date.now()
      };
      
      this.addMessage(request.chatId, userMessage);

      // 设置流式响应参数
      const streamRequest = { ...request, parameters: { ...request.parameters, stream: true } };

      // 设置流式事件监听器
      messageApi.listenToStreamEvents({
        onStart: (data) => {
          console.log('Stream started:', data);
          this.startStreaming(data.messageId);
        },
        onChunk: (data) => {
          console.log('Stream chunk:', data.content);
          this.setStreamingContent(data.content);
        },
        onEnd: (data) => {
          console.log('Stream ended:', data);
          // 创建响应对象
          const response: ChatResponse = {
            chatId: data.chatId,
            messageId: data.streamId, // 使用 streamId 作为 messageId
            content: data.finalContent,
            modelId: data.modelId,
            providerId: data.providerId,
          };
          this.finishStreaming(request.chatId!, response);
          this.setSending(false);
        },
        onError: (error) => {
          console.error('Stream error:', error);
          this.setError('流式响应错误');
          this.setSending(false);
        }
      });

      // 发送流式消息
      const streamId = await messageApi.sendStreamMessage(streamRequest);
      console.log('Stream ID:', streamId);

      // 稍后清理监听器（在流完成后）
      // unlistenPromise.then(unlisten => {
      //   // 监听器会在流结束后自动清理
      // });
      
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
    this.state.isLoading = false;
    this.state.isSending = false;
    this.state.error = null;
    this.state.streamingMessageId = null;
    this.state.streamingContent = '';
  }
}

// Export singleton instance
export const messageStore = new MessageStore();