/**
 * 聊天相关 API 封装（仅 Chat 资源）
 */

import { apiCall } from "./index";
import type { Chat, UUID, McpServerConfig } from "../types";

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
  mcpServers?: McpServerConfig[],
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
  console.log("Creating chat:", payload);
  return apiCall<Chat>("chat_create", payload);
}

/**
 * 获取聊天列表
 */
export async function getChats(
  limit?: number,
  offset?: number,
): Promise<Chat[]> {
  return apiCall<Chat[]>("chat_list", { limit, offset });
}

/**
 * 获取聊天详情
 */
export async function getChat(chatId: UUID): Promise<Chat> {
  return apiCall<Chat>("chat_get", { chatId: chatId });
}

/**
 * 删除聊天
 */
export async function deleteChat(chatId: UUID): Promise<void> {
  return apiCall<void>("chat_delete", { chatId: chatId });
}

/**
 * 生成聊天标题
 */
export async function generateChatTitle(
  chatId: UUID,
): Promise<{ title: string }> {
  return apiCall<{ title: string }>("chat_generate_title", { chatId });
}

/**
 * 更新聊天单个字段
 * @param chatId 聊天 ID
 * @param fieldName 字段名 (temperature, topP, maxTokens, stream, systemPrompt, mcpServers)
 * @param value 字段值，null 表示清空
 */
export async function updateChatField(
  chatId: UUID,
  fieldName:
    | "temperature"
    | "topP"
    | "maxTokens"
    | "stream"
    | "systemPrompt"
    | "mcpServers",
  value: number | boolean | string | McpServerConfig[] | null,
): Promise<Chat> {
  return apiCall<Chat>("chat_update_field", {
    chatId,
    fieldName,
    value,
  });
}

/**
 * 更新聊天模型
 * @param chatId 聊天 ID
 * @param modelId 模型 ID
 * @param providerId 供应商 ID
 */
export async function updateChatModel(
  chatId: UUID,
  modelId: string,
  providerId: string,
): Promise<Chat> {
  return apiCall<Chat>("chat_update_model", {
    chatId,
    modelId,
    providerId,
  });
}

/**
 * 更新聊天名称
 * @param chatId 聊天 ID
 * @param name 新名称
 */
export async function updateChatName(
  chatId: UUID,
  name: string,
): Promise<Chat> {
  return apiCall<Chat>("chat_update_name", {
    chatId,
    name,
  });
}
