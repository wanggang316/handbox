/**
 * 聊天相关 API 封装（仅 Chat 资源）
 */

import { apiCall } from './index';
import type {
  Chat,
  UUID,
  McpServerConfig
} from '../types';

/**
 * 创建新的聊天
 * 后端签名: chat_create(name, temperature?, top_p?, max_tokens?, stream?, model_id?, provider_id?, system_prompt?, mcp_servers?)
 */
export async function createChat(
  name: string,
  temperature?: number,
  topP?: number,
  maxTokens?: number,
  stream?: boolean,
  modelId?: string,
  providerId?: string,
  systemPrompt?: string,
  mcpServers?: McpServerConfig[]
): Promise<Chat> {
  const payload = {
    name,
    temperature,
    topP: topP,
    maxTokens: maxTokens,
    stream,
    modelId: modelId,
    providerId: providerId,
    systemPrompt: systemPrompt,
    mcpServers: mcpServers,
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
 * 后端签名: chat_update(chat_id, name?, temperature?, top_p?, max_tokens?, stream?, model_id?, provider_id?, system_prompt?, mcp_servers?)
 */
export async function updateChat(
  chatId: UUID,
  updates: Partial<Pick<Chat, 'name' | 'temperature' | 'topP' | 'maxTokens' | 'stream' | 'modelId' | 'providerId' | 'systemPrompt' | 'mcpServers'>>
): Promise<Chat> {
  const payload = {
    chatId: chatId,
    name: updates.name,
    temperature: updates.temperature,
    topP: updates.topP,
    maxTokens: updates.maxTokens,
    stream: updates.stream,
    modelId: updates.modelId,
    providerId: updates.providerId,
    systemPrompt: updates.systemPrompt,
    mcpServers: updates.mcpServers,
  };
  return apiCall<Chat>('chat_update', payload);
}

/**
 * 删除聊天
 */
export async function deleteChat(chatId: UUID): Promise<void> {
  return apiCall<void>('chat_delete', { chatId: chatId });
}

/**
 * 生成聊天标题
 */
export async function generateChatTitle(chatId: UUID): Promise<{ title: string }> {
  return apiCall<{ title: string }>('chat_generate_title', { chatId });
}