/**
 * Agent 相关 API 封装
 */

import { apiCall } from "./index";
import type {
  Agent,
  UUID,
  McpServerConfig,
  AgentReasoningConfig,
} from "../types";

/**
 * 创建新的 Agent
 * 后端签名: agent_create(request: AgentCreateRequest)
 */
export async function createAgent(
  name: string,
  model?: string,
  temperature?: number,
  topP?: number,
  topK?: number,
  reasoning?: AgentReasoningConfig,
  maxTokens?: number,
  systemPrompt?: string,
  mcpServers?: McpServerConfig[],
  skills?: string[],
  generativeUi?: boolean,
): Promise<Agent> {
  const request = {
    name,
    model,
    temperature,
    top_p: topP,
    top_k: topK,
    reasoning,
    max_tokens: maxTokens,
    system_prompt: systemPrompt,
    mcp_servers: mcpServers,
    skills,
    generative_ui: generativeUi,
  };
  console.log("Creating agent:", request);
  return apiCall<Agent>("agent_create", { request });
}

/**
 * 获取 Agent 列表
 */
export async function getAgents(
  limit?: number,
  offset?: number,
): Promise<Agent[]> {
  return apiCall<Agent[]>("agent_list", { limit, offset });
}

/**
 * 获取 Agent 详情
 */
export async function getAgent(agentId: UUID): Promise<Agent> {
  return apiCall<Agent>("agent_get", { agentId: agentId });
}

/**
 * 删除 Agent
 */
export async function deleteAgent(agentId: UUID): Promise<void> {
  return apiCall<void>("agent_delete", { agentId: agentId });
}

/**
 * 更新 Agent 单个字段
 * @param agentId Agent ID
 * @param fieldName 字段名
 * @param value 字段值，null 表示清空
 */
export async function updateAgentField(
  agentId: UUID,
  fieldName:
    | "name"
    | "model"
    | "temperature"
    | "topP"
    | "topK"
    | "maxTokens"
    | "systemPrompt"
    | "mcpServers"
    | "skills"
    | "reasoning"
    | "generativeUi",
  value:
    | string
    | number
    | boolean
    | McpServerConfig[]
    | string[]
    | AgentReasoningConfig
    | null,
): Promise<Agent> {
  return apiCall<Agent>("agent_update_field", {
    agentId,
    fieldName,
    value,
  });
}

/**
 * 更新 Agent 名称
 * @param agentId Agent ID
 * @param name 新名称
 */
export async function updateAgentName(
  agentId: UUID,
  name: string,
): Promise<Agent> {
  return apiCall<Agent>("agent_update_name", {
    agentId,
    name,
  });
}
