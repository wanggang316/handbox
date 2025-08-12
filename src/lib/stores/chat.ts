/**
 * 聊天相关状态管理
 */

import { writable, derived, get } from 'svelte/store';
import type { 
  ChatSession, 
  Message, 
  ChatConfig,
  ChatStreamEvent,
  UUID 
} from '../types';
import * as chatApi from '../api/chat';

// 当前活跃会话
export const currentSession = writable<ChatSession | null>(null);

// 会话列表
export const sessions = writable<ChatSession[]>([]);

// 当前会话的消息列表
export const messages = writable<Message[]>([]);

// 聊天配置
export const chatConfig = writable<ChatConfig | null>(null);

// 加载状态
export const isLoading = writable(false);

// 流式输入状态
export const isStreaming = writable(false);

// 当前流式消息内容
export const streamingContent = writable<string>('');

// 错误状态
export const chatError = writable<string | null>(null);

// 派生状态：是否有活跃会话
export const hasActiveSession = derived(
  currentSession,
  ($currentSession) => $currentSession !== null
);

// 派生状态：当前会话消息数量
export const messageCount = derived(
  messages,
  ($messages) => $messages.length
);

/**
 * 聊天操作
 */
export const chatActions = {
  /**
   * 加载会话列表
   */
  async loadSessions(): Promise<void> {
    try {
      isLoading.set(true);
      const sessionList = await chatApi.getChatSessions();
      sessions.set(sessionList);
    } catch (error) {
      chatError.set(error instanceof Error ? error.message : '加载会话列表失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 创建新会话
   */
  async createSession(name?: string, config?: Partial<ChatConfig>): Promise<ChatSession> {
    try {
      isLoading.set(true);
      const session = await chatApi.createChatSession(name, config);
      
      // 更新会话列表
      sessions.update(list => [session, ...list]);
      
      // 设置为当前会话
      currentSession.set(session);
      messages.set([]);
      chatConfig.set(session.config);
      
      return session;
    } catch (error) {
      chatError.set(error instanceof Error ? error.message : '创建会话失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 切换到指定会话
   */
  async switchToSession(sessionId: UUID): Promise<void> {
    try {
      isLoading.set(true);
      
      const [session, sessionMessages] = await Promise.all([
        chatApi.getChatSession(sessionId),
        chatApi.getChatMessages(sessionId)
      ]);
      
      currentSession.set(session);
      messages.set(sessionMessages);
      chatConfig.set(session.config);
    } catch (error) {
      chatError.set(error instanceof Error ? error.message : '切换会话失败');
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 发送消息
   */
  async sendMessage(content: string, attachments?: File[]): Promise<void> {
    const session = get(currentSession);
    if (!session) {
      throw new Error('没有活跃的会话');
    }

    try {
      isLoading.set(true);
      isStreaming.set(true);
      streamingContent.set('');

      // 添加用户消息到本地状态
      const userMessage: Message = {
        id: crypto.randomUUID(),
        sessionId: session.id,
        role: 'user',
        content,
        createdAt: Date.now(),
        updatedAt: Date.now()
      };
      
      messages.update(list => [...list, userMessage]);

      // 监听流式响应
      const unlisten = await chatApi.listenChatStream(session.id, (event: ChatStreamEvent) => {
        switch (event.type) {
          case 'delta':
            streamingContent.update(current => current + event.data.content);
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
            
            messages.update(list => [...list, assistantMessage]);
            streamingContent.set('');
            isStreaming.set(false);
            unlisten();
            break;
            
          case 'error':
            chatError.set(event.data.error);
            isStreaming.set(false);
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
      chatError.set(error instanceof Error ? error.message : '发送消息失败');
      isStreaming.set(false);
      throw error;
    } finally {
      isLoading.set(false);
    }
  },

  /**
   * 更新聊天配置
   */
  async updateConfig(newConfig: Partial<ChatConfig>): Promise<void> {
    const session = get(currentSession);
    if (!session) return;

    try {
      const updatedSession = await chatApi.updateChatSession(session.id, {
        config: { ...session.config, ...newConfig }
      });
      
      currentSession.set(updatedSession);
      chatConfig.set(updatedSession.config);
    } catch (error) {
      chatError.set(error instanceof Error ? error.message : '更新配置失败');
      throw error;
    }
  },

  /**
   * 删除消息
   */
  async deleteMessage(messageId: UUID): Promise<void> {
    try {
      await chatApi.deleteMessage(messageId);
      messages.update(list => list.filter(msg => msg.id !== messageId));
    } catch (error) {
      chatError.set(error instanceof Error ? error.message : '删除消息失败');
      throw error;
    }
  },

  /**
   * 重新生成消息
   */
  async regenerateMessage(messageId: UUID): Promise<void> {
    try {
      isStreaming.set(true);
      await chatApi.regenerateMessage(messageId);
      // 流式响应会通过事件处理
    } catch (error) {
      chatError.set(error instanceof Error ? error.message : '重新生成失败');
      isStreaming.set(false);
      throw error;
    }
  },

  /**
   * 清除错误状态
   */
  clearError(): void {
    chatError.set(null);
  },

  /**
   * 重置所有状态
   */
  reset(): void {
    currentSession.set(null);
    sessions.set([]);
    messages.set([]);
    chatConfig.set(null);
    isLoading.set(false);
    isStreaming.set(false);
    streamingContent.set('');
    chatError.set(null);
  }
};