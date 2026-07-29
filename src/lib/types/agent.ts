/** Mirrors the backend Rust agent types. */

import type { BaseEntity, UUID, Timestamp } from "./index";
import type {
  McpServerConfig,
  ResponsesReasoningConfig,
  ReasoningEffortConfig,
  ThinkingConfig,
  OpenrouterReasoningConfig,
} from "./llm";

export interface AgentReasoningConfig {
  responses?: ResponsesReasoningConfig;
  reasoningEffort?: ReasoningEffortConfig;
  thinking?: ThinkingConfig;
  openrouter?: OpenrouterReasoningConfig;
}

/** A reusable AI assistant configuration. */
export interface Agent extends BaseEntity {
  name: string;
  temperature?: number;
  topP?: number;
  topK?: number;
  reasoning?: AgentReasoningConfig | null;
  maxTokens?: number;
  systemPrompt?: string;
  mcpServers: McpServerConfig[];
  skills: string[];
  generativeUi?: boolean;
  // Linked GenUI (named JSON-Render spec) id.
  genuiId?: string;
  providerId?: string | null;
  // Lucide icon name.
  icon?: string | null;
  description?: string | null;
  // Read-only: true for the built-in agents; not accepted on create/update.
  builtin: boolean;
  // Builtin tool registration names.
  builtinTools: string[];
  // "required" | "optional" | "none"
  workingDirMode?: string | null;
  // "auto" | "manual"
  toolExecutionMode?: string | null;
  thinkingLevel?: string | null;
  // Conversation starter prompts.
  starters: string[];
}

export interface CreateAgentRequest {
  name: string;
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
  // The backend's agent_create does not accept the fields below; write them
  // via agent_update_field after creation.
  providerId?: string | null;
  icon?: string | null;
  description?: string | null;
  builtinTools?: string[];
  workingDirMode?: string | null;
  toolExecutionMode?: string | null;
  thinkingLevel?: string | null;
  starters?: string[];
}

export interface UpdateAgentRequest {
  name?: string;
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
  // builtin is read-only and intentionally absent here.
  providerId?: string | null;
  icon?: string | null;
  description?: string | null;
  builtinTools?: string[];
  workingDirMode?: string | null;
  toolExecutionMode?: string | null;
  thinkingLevel?: string | null;
  starters?: string[];
}
