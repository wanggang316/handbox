/**
 * Agent 相关类型定义 - 匹配后端 Rust 架构
 */

import type { BaseEntity, UUID, Timestamp } from "./index";
import type {
  McpServerConfig,
  ResponsesReasoningConfig,
  ReasoningEffortConfig,
  ThinkingConfig,
  OpenrouterReasoningConfig,
} from "./chat";

// Agent 推理配置
export interface AgentReasoningConfig {
  responses?: ResponsesReasoningConfig;
  reasoningEffort?: ReasoningEffortConfig;
  thinking?: ThinkingConfig;
  openrouter?: OpenrouterReasoningConfig;
}

// Agent 实体 - 可复用的 AI 助手配置
export interface Agent extends BaseEntity {
  name: string;
  model?: string;
  temperature?: number;
  topP?: number;
  topK?: number;
  reasoning?: AgentReasoningConfig | null;
  maxTokens?: number;
  systemPrompt?: string;
  mcpServers: McpServerConfig[];
  skills: string[];
}

// 创建 Agent 请求
export interface CreateAgentRequest {
  name: string;
  model?: string;
  temperature?: number;
  topP?: number;
  topK?: number;
  reasoning?: AgentReasoningConfig;
  maxTokens?: number;
  systemPrompt?: string;
  mcpServers?: McpServerConfig[];
  skills?: string[];
}

// 更新 Agent 请求
export interface UpdateAgentRequest {
  name?: string;
  model?: string;
  temperature?: number | null;
  topP?: number | null;
  topK?: number | null;
  reasoning?: AgentReasoningConfig | null;
  maxTokens?: number | null;
  systemPrompt?: string | null;
  mcpServers?: McpServerConfig[];
  skills?: string[];
}
