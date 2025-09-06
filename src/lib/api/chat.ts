/**
 * 聊天相关 API 封装（仅 Chat 资源）
 */

import { apiCall } from './index';
import type { 
  Chat, 
  UUID 
} from '../types';

/**
 * 创建新的聊天
 * 后端签名: chat_create(name, system_prompt?, mcp_servers?)
 */
export async function createChat(
  name: string,
  systemPrompt?: string,
  mcpServers?: string[]
): Promise<Chat> {
  const payload = {
    name,
    system_prompt: systemPrompt,
    mcp_servers: mcpServers,
  };
  console.log('Creating chat:', payload);
  return apiCall<Chat>('chat_create', payload);
}

/**
 * 获取聊天列表
 */
export async function getChats(
  limit?: number,
  offset?: number
): Promise<Chat[]> {
  return apiCall<Chat[]>('chat_list', { limit, offset });
}

/**
 * 获取聊天详情
 */
export async function getChat(chatId: UUID): Promise<Chat> {
  return apiCall<Chat>('chat_get', { chatId: chatId });
}

/**
 * 更新聊天
 * 后端签名: chat_update(chat_id, name?, system_prompt?, mcp_servers?)
 */
export async function updateChat(
  chatId: UUID,
  updates: Partial<Pick<Chat, 'name' | 'systemPrompt'>> & { mcpServers?: string[] }
): Promise<Chat> {
  const payload = {
    chat_id: chatId,
    name: updates.name,
    system_prompt: updates.systemPrompt,
    mcp_servers: updates.mcpServers,
  };
  return apiCall<Chat>('chat_update', payload);
}

/**
 * 删除聊天
 */
export async function deleteChat(chatId: UUID): Promise<void> {
  return apiCall<void>('chat_delete', { chat_id: chatId });
}