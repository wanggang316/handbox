/**
 * 聊天相关 API 封装（仅 Chat 资源）
 */

import { apiCall } from "./index";
import type { Chat, UUID, McpServerConfig, ChatReasoningConfig } from "../types";

/**
 * 创建新的聊天
 * 后端签名: chat_create(request: ChatCreateRequest)
 */
export async function createChat(
  name: string,
  temperature?: number,
  topP?: number,
  topK?: number,
  maxTokens?: number,
  stream?: boolean,
  modelId?: string,
  providerId?: string,
  systemPrompt?: string,
  mcpServers?: McpServerConfig[],
): Promise<Chat> {
  const request = {
    name,
    temperature,
    topP,
    topK,
    maxTokens,
    stream,
    modelId,
    providerId,
    systemPrompt,
    mcpServers,
  };
  console.log("Creating chat:", request);
  return apiCall<Chat>("chat_create", { request });
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
 * @param fieldName 字段名 (temperature, topP, topK, maxTokens, stream, systemPrompt, mcpServers, turnCount)
 * @param value 字段值，null 表示清空
 */
export async function updateChatField(
  chatId: UUID,
  fieldName:
    | "temperature"
    | "topP"
    | "topK"
    | "maxTokens"
    | "stream"
    | "systemPrompt"
    | "mcpServers"
    | "turnCount"
    | "reasoning",
  value: number | boolean | string | McpServerConfig[] | ChatReasoningConfig | null,
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
 * 清空聊天的模型参数
 */
export async function clearModelParameters(chatId: UUID): Promise<Chat> {
  return apiCall<Chat>("chat_clear_model_parameters", { chatId });
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
