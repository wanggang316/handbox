/**
 * 消息状态管理
 */

import type { Message, ChatResponse } from '$lib/types/chat';

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
    this.state.streamingContent += content;
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
        modelId: response.modelId,
        providerId: response.providerId,
        stream: false, // 响应消息不是流式的
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