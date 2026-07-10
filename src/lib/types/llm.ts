/**
 * LLM 运行时共享类型 - 模型推理 / MCP 配置等中立类型。
 *
 * 这些类型既被统一的 Agent 链路（agent / agentSession）使用，也曾被旧 chat 链路
 * 使用。为在删除 chat 后端后仍然可用，从 `chat.ts` 抽出到此中立模块。
 */

// MCP 服务器配置（每会话绑定的工具命名空间 + 执行模式）
export interface McpServerConfig {
  serverId: string;
  executionMode: "auto" | "manual";
  enabledTools: string[]; // List of enabled tool names for this server
}

// Reasoning/thinking support
export type ReasoningEffort = "minimal" | "low" | "medium" | "high";
export type ReasoningSummary = "auto" | "concise" | "detailed";

export interface ResponsesReasoningConfig {
  effort?: ReasoningEffort | null;
  summary?: ReasoningSummary | null;
}

export interface ReasoningEffortConfig {
  effort?: ReasoningEffort | null;
  includeReasoning?: boolean | null;
}

export interface ThinkingConfig {
  includeThoughts?: boolean | null;
  thinkingBudget?: number | null;
}

export interface OpenrouterReasoningConfig {
  effort?: ReasoningEffort | null;
  maxTokens?: number | null;
  exclude?: boolean | null;
}

export interface ChatReasoningConfig {
  responses?: ResponsesReasoningConfig;
  reasoningEffort?: ReasoningEffortConfig;
  thinking?: ThinkingConfig;
  openrouter?: OpenrouterReasoningConfig;
}
