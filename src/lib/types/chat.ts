/**
 * 聊天相关类型定义 - 匹配后端 Rust 架构
 */

import type { BaseEntity, UUID, Timestamp } from "./index";

// 消息角色
export type MessageRole = "user" | "assistant" | "system";

// MCP 服务器配置
export interface McpServerConfig {
  serverId: string;
  executionMode: "auto" | "manual";
  enabledTools: string[]; // List of enabled tool names for this server
}

// 消息配置 - 每条消息可以有独立的配置参数
export interface MessageConfig {
  temperature?: number;
  topP?: number;
  topK?: number;
  maxTokens?: number;
  stream?: boolean;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  mcpServers?: McpServerConfig[];
  turnCount?: number; // 对话回合数 - 用于限制上下文中包含的历史对话轮数
}

// 消息类型
export interface Message extends BaseEntity {
  chatId: UUID;
  role: MessageRole;
  content: string;
  reasoning?: string; // 推理过程内容

  // 每条消息的配置参数
  config?: MessageConfig;

  // 工具调用数据
  toolCalls?: ToolCall[];

  // 附件
  attachments?: MessageAttachment[];

  // 使用统计和时序信息
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
  startTime?: Timestamp;
  endTime?: Timestamp;
  duration?: number;
}

// 消息附件
export interface MessageAttachment {
  id: UUID;
  name: string;
  mimeType: string;
  size: number;
  path: string;
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

// 聊天实体
export interface Chat extends BaseEntity {
  name: string;
  lastMessageAt?: Timestamp;
  messageCount: number;

  // Chat-level configuration (default values)
  temperature?: number;
  topP?: number;
  topK?: number;
  maxTokens?: number;
  stream?: boolean;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  mcpServers: McpServerConfig[];
  turnCount?: number; // 对话回合数 - 用于限制上下文中包含的历史对话轮数

  artifactId?: UUID;
  reasoning?: ChatReasoningConfig | null;
}

// 模型参数
export interface ModelParameters {
  temperature?: number;
  topP?: number;
  topK?: number;
  maxTokens?: number;
  turnCount?: number; // 对话回合数 - 用于限制上下文中包含的历史对话轮数
  stream?: boolean;
}

// 消息请求
export interface MessageRequest {
  chatId?: UUID;
  modelId: string;
  providerId: string;
  messages: ChatMessage[];
  tempUserMessageId?: string;
  attachments?: ChatAttachment[];
}

// 简化的发送消息请求
export interface UserMessageSendRequest {
  chatId: UUID;
  content: string;
  tempUserMessageId: string;
  attachments?: ChatAttachment[];
}

// 聊天消息（请求中使用）
export interface ChatMessage {
  role: MessageRole;
  content: string;
  reasoning?: string; // 推理过程内容
  id?: string; // 临时消息ID，仅用于前端发送消息时标识用户消息
}

// 聊天附件（请求中使用）
export interface ChatAttachment {
  name: string;
  mimeType: string;
  data: Uint8Array;
}

// 消息响应
export interface MessageResponse {
  chatId: UUID;
  messageId: UUID;
  content: string;
  reasoning?: string;
  toolCalls?: ToolCall[];
  modelId: string;
  providerId: string;
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
  duration?: number;
}

// 工具调用执行模式
export type ToolExecutionMode = "auto" | "manual";

// 工具调用执行状态
export type ToolExecutionStatus =
  | "pending"
  | "executing"
  | "completed"
  | "failed";

// 工具函数信息
export interface ToolFunction {
  name: string;
  arguments: string;
}

// 工具调用数据
export interface ToolCall {
  index: number;
  id?: string;
  toolType?: string;
  function?: ToolFunction;
  executionMode?: ToolExecutionMode;
  executionStatus?: ToolExecutionStatus;
  result?: string;
}

// 流式消息事件
export type MessageStreamEvent =
  | {
      type: "delta";
      data: { content: string; reasoning?: string; tokens?: number };
    }
  | { type: "done"; data: MessageResponse }
  | { type: "error"; data: { error: string; code?: string } };
