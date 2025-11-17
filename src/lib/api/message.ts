/**
 * 消息相关 API 封装
 */

import { apiCall } from './index';
import { listen } from '@tauri-apps/api/event';
import type { MessageRequest, MessageResponse, Message, UUID, MessageStreamEvent, ToolExecutionStatus, UserMessageSendRequest } from '../types';

/**
 * 发送消息
 */
export async function sendUserMessage(request: MessageRequest): Promise<MessageResponse> {
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
  
  return await apiCall<any>('message_user_send', payload);
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
 * 流式重新生成助手消息 - 删除当前消息，根据本轮消息重新生成
 */
export async function regenerateAssistantMessageStream(messageId: UUID): Promise<void> {
  await apiCall<void>('message_assistant_regenerate_stream', { messageId: messageId });
}

/**
 * 流式重发用户消息 - 删除该消息之后的所有消息，然后重新发送（流式）
 * @param messageId 消息ID
 * @param content 可选的新消息内容，如果提供则更新消息内容后重新发送
 */
export async function resendUserMessageStream(messageId: UUID, content?: string): Promise<void> {
  await apiCall<void>('message_user_resend_stream', {
    messageId: messageId,
    content: content
  });
}

/**
 * 发送流式消息
 */
export async function sendUserMessageStream(request: UserMessageSendRequest): Promise<void> {
  // Tauri 命令期望参数名与函数参数名匹配
  const payload = {
    request: {
      chat_id: request.chatId,
      content: request.content,
      temp_user_message_id: request.tempUserMessageId,
      attachments: request.attachments
    }
  };

  await apiCall<void>('message_user_send_stream', payload);
}

/**
 * 监听流式消息事件
 */
export interface StreamEventHandlers {
  onStart?: (data: { streamId: string; messageId: string }) => void;
  onChunk?: (data: { streamId: string; content: string; reasoning?: string; toolCalls?: any[]; chunk: string; index: number }) => void;
  onEnd?: (data: { streamId: string; finalContent: string; finalReasoning?: string; chatId: string; modelId: string; providerId: string; toolCalls?: any[]; messageId?: string }) => void;
  onError?: (error: any) => void;
  onToolExecute?: (data: { messageId: string; toolCallIds: string[]; status: ToolExecutionStatus }) => void;
  onMessagesDelete?: (data: { chatId: string; messageIds: string[] }) => void;
  onUserMessageSaved?: (data: { tempMessageId: string; savedMessageId: string }) => void;
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

  // 如果提供了 onMessagesDelete 处理器，添加消息删除事件监听
  if (handlers.onMessagesDelete) {
    console.log('[listenToStreamEvents] 注册 messages_deleted 事件监听器');
    listeners.push(
      listen('messages_deleted', (event) => {
        console.log('[messages_deleted 事件] 收到事件:', event.payload);
        handlers.onMessagesDelete?.(event.payload as any);
      })
    );
  }

  // 如果提供了 onUserMessageSaved 处理器，添加用户消息保存事件监听
  if (handlers.onUserMessageSaved) {
    console.log('[listenToStreamEvents] 注册 user_message_saved 事件监听器');
    listeners.push(
      listen('user_message_saved', (event) => {
        console.log('[user_message_saved 事件] 收到事件:', event.payload);
        handlers.onUserMessageSaved?.(event.payload as any);
      })
    );
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

