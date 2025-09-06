/**
 * 消息相关 API 封装
 */

import { apiCall } from './index';
import type { ChatRequest, ChatResponse, Message, UUID } from '../types';

// 将后端返回的 snake_case 字段映射为前端使用的 camelCase
function mapMessage(r: any): Message {
  return {
    id: r.id,
    chatId: r.chat_id,
    role: r.role,
    content: r.content,
    // 强制保证这些字段存在（如果后端返回缺失，应在后端修复；这里做兼容）
    modelId: r.model_id,
    providerId: r.provider_id,
    // 参数相关（若后端返回则映射）
    temperature: r.temperature,
    topP: r.top_p,
    maxTokens: r.max_tokens,
    stream: r.stream,
    // 统计与时序
    inputTokens: r.input_tokens,
    outputTokens: r.output_tokens,
    totalTokens: r.total_tokens,
    startTime: r.start_time,
    endTime: r.end_time,
    duration: r.duration,
    createdAt: r.created_at,
    updatedAt: r.updated_at
  } as Message;
}

function mapChatResponse(r: any): ChatResponse {
  return {
    chatId: r.chat_id,
    messageId: r.message_id,
    content: r.content,
    modelId: r.model_id,
    providerId: r.provider_id,
    inputTokens: r.input_tokens,
    outputTokens: r.output_tokens,
    totalTokens: r.total_tokens,
    duration: r.duration
  } as ChatResponse;
}

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
  
  const r = await apiCall<any>('message_send', payload);
  return mapChatResponse(r);
}

/**
 * 获取聊天下的消息列表
 */
export async function getMessages(
  chatId: UUID,
  limit?: number,
  offset?: number
): Promise<Message[]> {
  const list = await apiCall<any[]>('message_list', { chat_id: chatId, limit, offset });
  return (list || []).filter(Boolean).map(mapMessage);
}

/**
 * 获取单条消息
 */
export async function getMessage(messageId: UUID): Promise<Message> {
  const r = await apiCall<any>('message_get', { message_id: messageId });
  return mapMessage(r);
}

/**
 * 更新消息
 */
export async function updateMessage(
  messageId: UUID,
  content: string
): Promise<Message> {
  const r = await apiCall<any>('message_update', { message_id: messageId, content });
  return mapMessage(r);
}

/**
 * 删除消息
 */
export async function deleteMessage(messageId: UUID): Promise<void> {
  return apiCall<void>('message_delete', { message_id: messageId });
}

/**
 * 重新生成助手消息
 */
export async function regenerateMessage(messageId: UUID): Promise<ChatResponse> {
  const r = await apiCall<any>('message_regenerate', { message_id: messageId });
  return mapChatResponse(r);
}


