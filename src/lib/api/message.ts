/**
 * 消息相关 API 封装
 */

import { apiCall } from './index';
import type { ChatRequest, ChatResponse, Message, UUID } from '../types';

/**
 * 发送消息
 */
export async function sendMessage(request: ChatRequest): Promise<ChatResponse> {
  // Tauri 命令期望参数名与函数参数名匹配
  const payload = {
    request: {
      chat_id: request.chatId,
      artifact_id: request.artifactId,
      model_id: request.modelId,
      provider_id: request.providerId,
      parameters: request.parameters,
      messages: request.messages,
      attachments: request.attachments
    }
  };
  
  return await apiCall<any>('message_send', payload);
}

/**
 * 获取聊天下的消息列表
 */
export async function getMessages(
  chatId: UUID,
  limit?: number,
  offset?: number
): Promise<Message[]> {
  const list = await apiCall<any[]>('message_list', { chatId: chatId, limit, offset });
  console.log('getMessages >>> :', list);
  return list || [];
}

/**
 * 获取单条消息
 */
export async function getMessage(messageId: UUID): Promise<Message> {
  const r = await apiCall<any>('message_get', { messageId: messageId });
  return r;
}

/**
 * 更新消息
 */
export async function updateMessage(
  messageId: UUID,
  content: string
): Promise<Message> {
  const r = await apiCall<any>('message_update', { messageId: messageId, content });
  return r;
}

/**
 * 删除消息
 */
export async function deleteMessage(messageId: UUID): Promise<void> {
  return apiCall<void>('message_delete', { messageId: messageId });
}

/**
 * 重新生成助手消息
 */
export async function regenerateMessage(messageId: UUID): Promise<ChatResponse> {
  return await apiCall<any>('message_regenerate', { messageId: messageId });
}


