import { apiCall } from "./index";
import type {
  Agent,
  UUID,
  McpServerConfig,
  AgentReasoningConfig,
} from "../types";

export async function createAgent(
  name: string,
  temperature?: number,
  topP?: number,
  topK?: number,
  reasoning?: AgentReasoningConfig,
  maxTokens?: number,
  systemPrompt?: string,
  mcpServers?: McpServerConfig[],
  skills?: string[],
  generativeUi?: boolean,
  genuiId?: string,
): Promise<Agent> {
  const request = {
    name,
    temperature,
    top_p: topP,
    top_k: topK,
    reasoning,
    max_tokens: maxTokens,
    system_prompt: systemPrompt,
    mcp_servers: mcpServers,
    skills,
    generative_ui: generativeUi,
    genui_id: genuiId,
  };
  console.log("Creating agent:", request);
  return apiCall<Agent>("agent_create", { request });
}

export async function getAgents(
  limit?: number,
  offset?: number,
): Promise<Agent[]> {
  return apiCall<Agent[]>("agent_list", { limit, offset });
}

export async function getAgent(agentId: UUID): Promise<Agent> {
  return apiCall<Agent>("agent_get", { agentId: agentId });
}

export async function deleteAgent(agentId: UUID): Promise<void> {
  return apiCall<void>("agent_delete", { agentId: agentId });
}

/** Updates a single agent field; `value: null` clears it. */
export async function updateAgentField(
  agentId: UUID,
  fieldName:
    | "name"
    | "temperature"
    | "topP"
    | "topK"
    | "maxTokens"
    | "systemPrompt"
    | "mcpServers"
    | "skills"
    | "reasoning"
    | "generativeUi"
    | "genuiId"
    | "providerId"
    | "icon"
    | "description"
    | "builtinTools"
    | "workingDirMode"
    | "toolExecutionMode"
    | "thinkingLevel"
    | "starters",
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

export async function updateAgentName(
  agentId: UUID,
  name: string,
): Promise<Agent> {
  return apiCall<Agent>("agent_update_name", {
    agentId,
    name,
  });
}
