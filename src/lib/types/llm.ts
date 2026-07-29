/**
 * Provider-neutral LLM runtime types (model reasoning / MCP configuration)
 * shared across the agent chain.
 */

// Per-session MCP server binding: tool namespace plus execution mode.
export interface McpServerConfig {
  serverId: string;
  executionMode: "auto" | "manual";
  enabledTools: string[];
}

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
