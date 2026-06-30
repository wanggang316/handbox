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
} from "./llm";

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
  generativeUi?: boolean;
  // 关联的 GenUI（具名 JSON-Render spec）id；未关联时为 undefined
  genuiId?: string;
  // ── 能力扩展字段（P2）：后端 agents 表新增列 ──
  // 选定的供应商；未设置时为 null
  providerId?: string | null;
  // Lucide 图标名
  icon?: string | null;
  // 一行简介
  description?: string | null;
  // 系统内建标记（只读）：两个内置 Agent 为 true；创建/更新时不可下发
  builtin: boolean;
  // coding-agent 内置工具名："read"/"write"/"edit"/"bash"/"grep"/"find"/"ls"
  builtinTools: string[];
  // 工作目录模式："required" | "optional" | "none"
  workingDirMode?: string | null;
  // 工具执行模式："auto" | "manual"
  toolExecutionMode?: string | null;
  thinkingLevel?: string | null;
  // 对话开场白
  starters: string[];
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
  generativeUi?: boolean;
  genuiId?: string;
  // ── 能力扩展字段（P2，可选）。注意：后端 agent_create 不接受这些字段，
  // 需在创建后通过 agent_update_field 逐项写入。──
  providerId?: string | null;
  icon?: string | null;
  description?: string | null;
  builtinTools?: string[];
  workingDirMode?: string | null;
  toolExecutionMode?: string | null;
  thinkingLevel?: string | null;
  starters?: string[];
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
  generativeUi?: boolean;
  genuiId?: string | null;
  // ── 能力扩展字段（P2，可选）。builtin 为只读，不在此列。──
  providerId?: string | null;
  icon?: string | null;
  description?: string | null;
  builtinTools?: string[];
  workingDirMode?: string | null;
  toolExecutionMode?: string | null;
  thinkingLevel?: string | null;
  starters?: string[];
}
