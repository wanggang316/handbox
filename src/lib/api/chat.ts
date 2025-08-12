/**
 * 聊天相关 API 封装
 */

import { listen } from '@tauri-apps/api/event';
import { apiCall } from './index';
import type { 
  ChatRequest, 
  ChatResponse, 
  ChatSession, 
  ChatConfig,
  ChatStreamEvent,
  Message,
  UUID 
} from '../types';

/**
 * 发送聊天消息
 */
export async function sendChatMessage(request: ChatRequest): Promise<ChatResponse> {
  return apiCall<ChatResponse>('chat_send', request);
}

/**
 * 监听流式聊天事件
 */
export async function listenChatStream(
  sessionId: UUID,
  callback: (event: ChatStreamEvent) => void
): Promise<() => void> {
  const unlisten = await listen<ChatStreamEvent>(`chat_stream_${sessionId}`, (event) => {
    callback(event.payload);
  });
  
  return unlisten;
}

/**
 * 创建新的聊天会话
 */
export async function createChatSession(
  name?: string,
  config?: Partial<ChatConfig>
): Promise<ChatSession> {
  return apiCall<ChatSession>('chat_create_session', { name, config });
}

/**
 * 获取聊天会话列表
 */
export async function getChatSessions(
  limit?: number,
  offset?: number
): Promise<ChatSession[]> {
  return apiCall<ChatSession[]>('chat_list_sessions', { limit, offset });
}

/**
 * 获取会话详情
 */
export async function getChatSession(sessionId: UUID): Promise<ChatSession> {
  return apiCall<ChatSession>('chat_get_session', { sessionId });
}

/**
 * 更新会话配置
 */
export async function updateChatSession(
  sessionId: UUID,
  updates: Partial<Omit<ChatSession, 'id' | 'createdAt' | 'updatedAt'>>
): Promise<ChatSession> {
  return apiCall<ChatSession>('chat_update_session', { sessionId, ...updates });
}

/**
 * 删除会话
 */
export async function deleteChatSession(sessionId: UUID): Promise<void> {
  return apiCall<void>('chat_delete_session', { sessionId });
}

/**
 * 获取会话消息
 */
export async function getChatMessages(
  sessionId: UUID,
  limit?: number,
  offset?: number
): Promise<Message[]> {
  return apiCall<Message[]>('chat_get_messages', { sessionId, limit, offset });
}

/**
 * 更新消息
 */
export async function updateMessage(
  messageId: UUID,
  content: string
): Promise<Message> {
  return apiCall<Message>('chat_update_message', { messageId, content });
}

/**
 * 删除消息
 */
export async function deleteMessage(messageId: UUID): Promise<void> {
  return apiCall<void>('chat_delete_message', { messageId });
}

/**
 * 重新生成助手消息
 */
export async function regenerateMessage(messageId: UUID): Promise<ChatResponse> {
  return apiCall<ChatResponse>('chat_regenerate_message', { messageId });
}