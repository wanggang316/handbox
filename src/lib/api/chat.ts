/**
 * Session 相关 API 封装（原 Chat 资源，现为 Session）
 * 注意：为了保持向后兼容，所有函数名仍使用 Chat 命名
 */

import { apiCall } from "./index";
import type {
  Chat,
  UUID,
  McpServerConfig,
  ChatReasoningConfig,
} from "../types";

/**
 * 创建新的 Session
 * 后端签名: session_create(request: SessionCreateRequest)
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
  console.log("Creating session:", request);
  return apiCall<Chat>("session_create", { request: request });
}

/**
 * 获取 Session 列表
 */
export async function getChats(
  limit?: number,
  offset?: number,
): Promise<Chat[]> {
  return apiCall<Chat[]>("session_list", { limit, offset });
}

/**
 * 获取 Session 详情
 */
export async function getChat(chatId: UUID): Promise<Chat> {
  return apiCall<Chat>("session_get", { chatId: chatId });
}

/**
 * 删除 Session
 */
export async function deleteChat(chatId: UUID): Promise<void> {
  return apiCall<void>("session_delete", { chatId: chatId });
}

/**
 * 生成 Session 标题
 */
export async function generateChatTitle(
  chatId: UUID,
): Promise<{ title: string }> {
  return apiCall<{ title: string }>("session_generate_title", { chatId });
}

/**
 * 更新 Session 单个字段
 * @param chatId Session ID
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
  value:
    | number
    | boolean
    | string
    | McpServerConfig[]
    | ChatReasoningConfig
    | null,
): Promise<Chat> {
  return apiCall<Chat>("session_update_field", {
    chatId,
    fieldName,
    value,
  });
}

/**
 * 更新 Session 模型
 * @param chatId Session ID
 * @param modelId 模型 ID
 * @param providerId 供应商 ID
 */
export async function updateChatModel(
  chatId: UUID,
  modelId: string,
  providerId: string,
): Promise<Chat> {
  return apiCall<Chat>("session_update_model", {
    chatId,
    modelId,
    providerId,
  });
}

/**
 * 清空 Session 的模型参数
 */
export async function clearModelParameters(chatId: UUID): Promise<Chat> {
  return apiCall<Chat>("session_clear_model_parameters", { chatId });
}

/**
 * 更新 Session 名称
 * @param chatId Session ID
 * @param name 新名称
 */
export async function updateChatName(
  chatId: UUID,
  name: string,
): Promise<Chat> {
  return apiCall<Chat>("session_update_name", {
    chatId,
    name,
  });
}

/**
 * 通过 Agent 创建 Session（新增）
 * 后端签名: session_create_from_agent(agentId: UUID)
 */
export async function createSessionFromAgent(agentId: UUID): Promise<Chat> {
  console.log("Creating session from agent:", agentId);
  return apiCall<Chat>("session_create_from_agent", { agentId });
}
