/**
 * 消息相关 API 封装
 */

import { apiCall } from './index';
import { listen } from '@tauri-apps/api/event';
import type { MessageRequest, MessageResponse, Message, UUID, MessageStreamEvent } from '../types';

/**
 * 发送消息
 */
export async function sendMessage(request: MessageRequest): Promise<MessageResponse> {
  // Tauri 命令期望参数名与函数参数名匹配
  const payload = {
    request: {
      chat_id: request.chatId,
      model_id: request.modelId,
      provider_id: request.providerId,
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
export async function regenerateMessage(messageId: UUID): Promise<MessageResponse> {
  return await apiCall<any>('message_regenerate', { messageId: messageId });
}

/**
 * 发送流式消息
 */
export async function sendStreamMessage(request: MessageRequest): Promise<string> {
  // Tauri 命令期望参数名与函数参数名匹配
  const payload = {
    request: {
      chat_id: request.chatId,
      model_id: request.modelId,
      provider_id: request.providerId,
      messages: request.messages,
      attachments: request.attachments
    }
  };
  
  return await apiCall<string>('message_send_stream', payload);
}

/**
 * 监听流式消息事件
 */
export interface StreamEventHandlers {
  onStart?: (data: { streamId: string; messageId: string }) => void;
  onChunk?: (data: { streamId: string; content: string; reasoning?: string; chunk: string; index: number }) => void;
  onEnd?: (data: { streamId: string; finalContent: string; finalReasoning?: string; chatId: string; modelId: string; providerId: string; pendingMcpCall?: any; messageId?: string }) => void;
  onError?: (error: any) => void;
}

export async function listenToStreamEvents(handlers: StreamEventHandlers) {
  const unlisten = await Promise.all([
    listen('message_stream_start', (event) => {
      handlers.onStart?.(event.payload as any);
    }),
    
    listen('message_stream_chunk', (event) => {
      handlers.onChunk?.(event.payload as any);
    }),
    
    listen('message_stream_end', (event) => {
      handlers.onEnd?.(event.payload as any);
    })
  ]);

  // 返回取消监听的函数
  return () => {
    unlisten.forEach(fn => fn());
  };
}

export async function executePendingMcpCall(pendingId: string): Promise<MessageResponse> {
  return await apiCall<MessageResponse>('message_execute_mcp_call', { pendingId: pendingId });
}

export async function executeToolCalls(messageId: string, toolCallDeltas: any[]): Promise<MessageResponse> {
  return await apiCall<MessageResponse>('message_execute_tool_calls', {
    messageId: messageId,
    toolCallDeltas: toolCallDeltas
  });
}

