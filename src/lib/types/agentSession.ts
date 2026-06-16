/**
 * Agent 模式类型定义 - 镜像后端 Rust 形状
 *
 * 三个来源严格对齐（field names / discriminator values 必须逐字一致，
 * 否则下游 timeline reducer 解析会失配）：
 *  - `storage/types/agent_session.rs`（`#[serde(rename_all = "camelCase")]`）
 *  - hand-agent `AgentEvent`（`tag = "type"`, snake_case variants, camelCase fields）
 *  - hand-agent / model `Message` / `AssistantContentBlock` / `Usage`
 *    （`Message` 以 `role` 标签；`AssistantContentBlock` 以 `type` 标签且为 lowercase）
 */

import type { UUID, Timestamp } from "./index";

// ---------------------------------------------------------------------------
// hand-agent / model Message（payload 的实际形状）
// ---------------------------------------------------------------------------

/**
 * Token 用量与成本（model crate `Usage`）。
 * 字段经 serde rename 为 camelCase（`cacheRead`/`cacheWrite`/`totalTokens`）。
 */
export interface UsageCost {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  total: number;
}

export interface Usage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  totalTokens: number;
  cost: UsageCost;
}

/** 助手停止原因（model crate `StopReason`，`rename_all = "camelCase"`）。 */
export type StopReason = "stop" | "length" | "toolUse" | "error" | "aborted";

/**
 * 文本内容块（model crate `TextContent`）。
 * `content_type` 在 Rust 侧 `#[serde(skip)]`，外层 tag 已携带 `type`，故不在 wire 上。
 */
export interface TextContent {
  text: string;
  textSignature?: string;
}

/** 思考内容块（model crate `ThinkingContent`）。 */
export interface ThinkingContent {
  thinking: string;
  thinkingSignature?: string;
  redacted?: boolean;
}

/**
 * 工具调用内容块（model crate `ToolCall`）。
 *
 * 命名为 `AgentToolCall` 以避免与 chat 模式的 `ToolCall`（types/chat.ts）在
 * 共享 barrel 中重名冲突 —— 二者形状不同（此为 agent/model 的内容块）。
 */
export interface AgentToolCall {
  id: string;
  name: string;
  arguments: unknown;
  thoughtSignature?: string;
}

/**
 * 助手内容块判别联合（model crate `AssistantContentBlock`，
 * `#[serde(tag = "type", rename_all = "lowercase")]`）。
 *
 * 注意 `rename_all = "lowercase"` 把变体 `ToolCall` 序列化为 `"toolcall"`
 * （全小写、无分隔符）—— 与 hand-ai web-ui 观察到的 server 形状一致。
 */
export type AssistantContentBlock =
  | ({ type: "text" } & TextContent)
  | ({ type: "thinking" } & ThinkingContent)
  | ({ type: "toolcall" } & AgentToolCall);

/**
 * 用户消息内容（model crate `UserContent`，`#[serde(untagged)]`）：
 * 纯文本字符串，或内容块数组。
 */
export interface ImageContent {
  data: string;
  mimeType: string;
}

export type UserContentBlock =
  | ({ type: "text" } & TextContent)
  | ({ type: "image" } & ImageContent);

export type UserContent = string | UserContentBlock[];

/** 工具结果内容块（model crate `ToolResultContent`，`tag = "type"`, lowercase）。 */
export type ToolResultContent =
  | ({ type: "text" } & TextContent)
  | ({ type: "image" } & ImageContent);

/** 用户消息（model crate `UserMessage`）。`role` 在 Rust 侧 skip，由外层 Message tag 提供。 */
export interface UserMessage {
  content: UserContent;
  timestamp: number;
}

/** 助手消息（model crate `AssistantMessage`）。 */
export interface AssistantMessage {
  content: AssistantContentBlock[];
  api: string;
  provider: string;
  model: string;
  usage: Usage;
  stopReason: StopReason;
  errorMessage?: string;
  timestamp: number;
  responseModel?: string;
  responseId?: string;
  diagnostics?: unknown[];
}

/** 工具结果消息（model crate `ToolResultMessage`）。 */
export interface ToolResultMessage {
  toolCallId: string;
  toolName: string;
  content: ToolResultContent[];
  details?: unknown;
  isError: boolean;
  timestamp: number;
}

/**
 * 任意会话消息判别联合（model crate `Message`，
 * `#[serde(tag = "role", rename_all = "camelCase")]`）。
 *
 * 这是 `AgentSessionMessage.payload` 的精确类型 —— 序列化后的 hand-agent Message。
 */
export type AgentMessage =
  | ({ role: "user" } & UserMessage)
  | ({ role: "assistant" } & AssistantMessage)
  | ({ role: "toolResult" } & ToolResultMessage);

/**
 * 流式增量事件（model crate `AssistantMessageEvent`，
 * `tag = "type"`, snake_case variants, camelCase fields）。
 *
 * `message_update` 携带的 delta。每个变体都带 `partial`（当前累积的助手消息）；
 * `*_delta` 变体额外带 `delta` 文本，`done`/`error` 带终结消息。
 */
export type AssistantMessageEvent =
  | { type: "start"; partial: AssistantMessage }
  | { type: "text_start"; contentIndex: number; partial: AssistantMessage }
  | {
      type: "text_delta";
      contentIndex: number;
      delta: string;
      partial: AssistantMessage;
    }
  | {
      type: "text_end";
      contentIndex: number;
      content: string;
      partial: AssistantMessage;
    }
  | { type: "thinking_start"; contentIndex: number; partial: AssistantMessage }
  | {
      type: "thinking_delta";
      contentIndex: number;
      delta: string;
      partial: AssistantMessage;
    }
  | {
      type: "thinking_end";
      contentIndex: number;
      content: string;
      partial: AssistantMessage;
    }
  | { type: "toolcall_start"; contentIndex: number; partial: AssistantMessage }
  | {
      type: "toolcall_delta";
      contentIndex: number;
      delta: string;
      partial: AssistantMessage;
    }
  | {
      type: "toolcall_end";
      contentIndex: number;
      toolCall: AgentToolCall;
      partial: AssistantMessage;
    }
  | { type: "done"; reason: StopReason; message: AssistantMessage }
  | { type: "error"; reason: StopReason; error: AssistantMessage };

// ---------------------------------------------------------------------------
// Agent 运行事件（hand-agent `AgentEvent`）
// ---------------------------------------------------------------------------

/**
 * 工具结果（hand-agent `ToolResult`）—— `ToolExecutionEnd`/`Update` 的 payload。
 */
export interface ToolResult {
  content: ToolResultContent[];
  details?: unknown;
  terminate?: boolean;
}

/**
 * Agent 运行期事件判别联合（hand-agent `AgentEvent`，
 * `#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]`）。
 *
 * 在 `type` 上判别：narrowing `type === "tool_execution_end"` 得到
 * `toolCallId`/`toolName`/`result`/`isError`。
 */
export type AgentEvent =
  | { type: "agent_start" }
  | { type: "agent_end"; messages: AgentMessage[] }
  | { type: "turn_start" }
  | {
      type: "turn_end";
      message: AgentMessage;
      toolResults: ToolResultMessage[];
    }
  | { type: "message_start"; message: AgentMessage }
  | {
      type: "message_update";
      message: AgentMessage;
      assistantMessageEvent: AssistantMessageEvent;
    }
  | { type: "message_end"; message: AgentMessage }
  | {
      type: "tool_execution_start";
      toolCallId: string;
      toolName: string;
      args: unknown;
    }
  | {
      type: "tool_execution_update";
      toolCallId: string;
      toolName: string;
      args: unknown;
      partialResult: ToolResult;
    }
  | {
      type: "tool_execution_end";
      toolCallId: string;
      toolName: string;
      result: ToolResult;
      isError: boolean;
    };

// ---------------------------------------------------------------------------
// Agent Session 实体（storage/types/agent_session.rs）
// ---------------------------------------------------------------------------

/** Agent Session 实体 - Agent 模式下的会话实例。 */
export interface AgentSession {
  id: UUID;
  /** 所属 Agent Project（可选；后端 `project_id: Option<UUID>` 序列化为 camelCase）。 */
  projectId?: UUID;
  name: string;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  thinkingLevel?: string;
  temperature?: number;
  maxTokens?: number;
  workingDir?: string;
  enabledTools: string[];
  toolExecutionMode?: string;
  messageCount: number;
  lastMessageAt?: Timestamp;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

/**
 * Agent Session 消息 - `payload` 是序列化后的 hand-agent Message。
 * 类型化为 `AgentMessage` 联合，使 timeline reducer 无需 `any` 即可消费。
 */
export interface AgentSessionMessage {
  id: UUID;
  sessionId: UUID;
  seq: number;
  role: string;
  payload: AgentMessage;
  createdAt: Timestamp;
}

/** 创建 Agent Session 请求。 */
export interface CreateAgentSessionRequest {
  name: string;
  /**
   * 可选：挂靠到某个 Agent Project（后端 `project_id: Option<UUID>`）。
   * 提供时后端以 project.path 覆盖 workingDir；项目不存在 → NOT_FOUND，
   * 项目目录已失效 → VALIDATION_ERROR，均不写入任何行。
   */
  projectId?: UUID;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  thinkingLevel?: string;
  temperature?: number;
  maxTokens?: number;
  workingDir?: string;
  enabledTools?: string[];
  toolExecutionMode?: string;
}

/** 更新 Agent Session 请求。 */
export interface UpdateAgentSessionRequest {
  name?: string;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  thinkingLevel?: string;
  temperature?: number;
  maxTokens?: number;
  workingDir?: string;
  enabledTools?: string[];
  toolExecutionMode?: string;
}

// ---------------------------------------------------------------------------
// Tauri 流式事件 payload（commands/agent_run.rs）
// ---------------------------------------------------------------------------

/**
 * 随本回合输入一并发送的图片附件（镜像后端 `AgentRunAttachment`）。
 *
 * `data` 是原始字节序列（`number[]`，serde 把 Rust `Vec<u8>` 反序列化自此）。
 * 仅 `image/*` mime 由后端装配成 `ImageContent` 块；前端在选图时已按 image/*
 * 过滤，后端再防御性跳过非图片。
 */
export interface AgentRunAttachment {
  name: string;
  mimeType: string;
  data: number[];
}

/** `agent_stream_event` 的 payload：每条 AgentEvent 携 sessionId 发出。 */
export interface AgentStreamEventPayload {
  sessionId: UUID;
  event: AgentEvent;
}

/** `agent_stream_error` 的 payload：run-level sanitized 错误 envelope（在 closed 之前）。 */
export interface AgentStreamErrorPayload {
  sessionId: UUID;
  error: {
    code: string;
    message: string;
    hint?: string;
  };
}

/** `agent_stream_closed` 的 payload：回合终结信号（每个 run 恰好一次）。 */
export interface AgentStreamClosedPayload {
  sessionId: UUID;
}

/**
 * `agent_session_lifecycle` 的 payload：会话生命周期信号，与三条 run 通道并列、
 * 独立——这些不是 run 事件，不进 `agent_stream_event` reducer，故不影响 closed-once。
 * 后端 `map_session_event` 把 coding-agent 的 `AgentSessionEvent::CompactionStart`/
 * `CompactionEnd`/`SessionInfoChanged` 映射到此判别联合（`kind` 判别）。
 *
 *  - `compaction_start`：自动压缩开始；前端据此展示「整理上下文中」指示。
 *  - `compaction_end`：压缩结束；`summary` 为上下文摘要，**有意不渲染进时间线**
 *    （仅用于关闭指示，去向稳定——VAL-CARUN-019），对话续行。
 *  - `session_info_changed`：会话元数据（当前仅 name/label）变更；前端据此即时
 *    更新侧栏该会话标题，无需重开（VAL-CARUN-020）。`name` 可为 null（清空标题）。
 */
export type AgentSessionLifecyclePayload =
  | { sessionId: UUID; kind: "compaction_start" }
  | { sessionId: UUID; kind: "compaction_end"; summary: string }
  | { sessionId: UUID; kind: "session_info_changed"; name: string | null };
