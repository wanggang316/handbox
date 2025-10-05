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
 * 重发用户消息 - 删除该消息之后的所有消息，然后重新发送
 */
export async function resendMessage(messageId: UUID): Promise<MessageResponse> {
  return await apiCall<any>('message_resend', { messageId: messageId });
}

/**
 * 发送流式消息
 */
export async function sendStreamMessage(request: MessageRequest): Promise<void> {
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

  await apiCall<void>('message_send_stream', payload);
}

/**
 * 监听流式消息事件
 */
export interface StreamEventHandlers {
  onStart?: (data: { streamId: string; messageId: string }) => void;
  onChunk?: (data: { streamId: string; content: string; reasoning?: string; toolCalls?: any[]; chunk: string; index: number }) => void;
  onEnd?: (data: { streamId: string; finalContent: string; finalReasoning?: string; chatId: string; modelId: string; providerId: string; toolCalls?: any[]; messageId?: string }) => void;
  onError?: (error: any) => void;
  onToolExecute?: (data: { messageId: string; toolCallIds: string[]; status: 'executing' | 'finished' }) => void;
}

export async function listenToStreamEvents(handlers: StreamEventHandlers, eventPrefix: string = 'message_stream') {
  const listeners = [
    listen(`${eventPrefix}_start`, (event) => {
      handlers.onStart?.(event.payload as any);
    }),

    listen(`${eventPrefix}_chunk`, (event) => {
      handlers.onChunk?.(event.payload as any);
    }),

    listen(`${eventPrefix}_end`, (event) => {
      handlers.onEnd?.(event.payload as any);
    }),

    listen(`${eventPrefix}_error`, (event) => {
      handlers.onError?.(event.payload as any);
    })
  ];

  // 如果提供了 onToolExecute 处理器，添加工具执行事件监听
  if (handlers.onToolExecute) {
    console.log('[listenToStreamEvents] 注册 tool_execute 事件监听器');
    listeners.push(
      listen('tool_execute', (event) => {
        console.log('[tool_execute 事件] 收到事件:', event.payload);
        handlers.onToolExecute?.(event.payload as any);
      })
    );
  } else {
    console.warn('[listenToStreamEvents] 未提供 onToolExecute 处理器');
  }

  const unlisten = await Promise.all(listeners);

  // 返回取消监听的函数
  return () => {
    unlisten.forEach(fn => fn());
  };
}

export async function executeToolCalls(messageId: string, toolCallIds: string[]): Promise<void> {
  await apiCall<void>('message_execute_tool_calls', {
    messageId: messageId,
    toolCallIds: toolCallIds
  });
}

/**
 * 流式执行工具调用
 */
export async function executeToolCallsStream(messageId: string, toolCallIds: string[]): Promise<void> {
  await apiCall<void>('message_execute_tool_calls_stream', {
    messageId: messageId,
    toolCallIds: toolCallIds
  });
}

