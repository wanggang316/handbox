/**
 * Mirrors the backend Rust shapes. Field names and discriminator values must
 * match these sources verbatim, or the timeline reducer misparses:
 *  - `storage/types/agent_session.rs` (serde camelCase)
 *  - hand-agent `AgentEvent` (`tag = "type"`, snake_case variants, camelCase fields)
 *  - hand-agent / model `Message` / `AssistantContentBlock` / `Usage`
 *    (`Message` tagged by `role`; `AssistantContentBlock` tagged by `type`, lowercase)
 */

import type { UUID, Timestamp } from "./index";
import type { McpServerConfig } from "./llm";

/** Token usage and cost (model crate `Usage`); fields serde-renamed to camelCase. */
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

/** Model crate `StopReason` (`rename_all = "camelCase"`). */
export type StopReason = "stop" | "length" | "toolUse" | "error" | "aborted";

/**
 * Model crate `TextContent`. `content_type` is `#[serde(skip)]` in Rust —
 * the outer tag already carries `type`, so it is not on the wire.
 */
export interface TextContent {
  text: string;
  textSignature?: string;
}

/** Model crate `ThinkingContent`. */
export interface ThinkingContent {
  thinking: string;
  thinkingSignature?: string;
  redacted?: boolean;
}

/**
 * Model crate `ToolCall`. Named `AgentToolCall` rather than bare `ToolCall`
 * to stay unambiguous in the shared barrel.
 */
export interface AgentToolCall {
  id: string;
  name: string;
  arguments: unknown;
  thoughtSignature?: string;
}

/**
 * Model crate `AssistantContentBlock` (`#[serde(tag = "type", rename_all =
 * "lowercase")]`). Note `lowercase` serializes the `ToolCall` variant as
 * `"toolcall"` — all lowercase, no separator.
 */
export type AssistantContentBlock =
  | ({ type: "text" } & TextContent)
  | ({ type: "thinking" } & ThinkingContent)
  | ({ type: "toolcall" } & AgentToolCall);

/**
 * User message content (model crate `UserContent`, `#[serde(untagged)]`):
 * a plain string or an array of content blocks.
 */
export interface ImageContent {
  data: string;
  mimeType: string;
}

export type UserContentBlock =
  | ({ type: "text" } & TextContent)
  | ({ type: "image" } & ImageContent);

export type UserContent = string | UserContentBlock[];

/** Model crate `ToolResultContent` (`tag = "type"`, lowercase). */
export type ToolResultContent =
  | ({ type: "text" } & TextContent)
  | ({ type: "image" } & ImageContent);

/** Model crate `UserMessage`; `role` is skipped in Rust — the outer `Message` tag provides it. */
export interface UserMessage {
  content: UserContent;
  timestamp: number;
}

/** Model crate `AssistantMessage`. */
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

/** Model crate `ToolResultMessage`. */
export interface ToolResultMessage {
  toolCallId: string;
  toolName: string;
  content: ToolResultContent[];
  details?: unknown;
  isError: boolean;
  timestamp: number;
}

/**
 * Model crate `Message` (`#[serde(tag = "role", rename_all = "camelCase")]`)
 * — the exact type of `AgentSessionMessage.payload`.
 */
export type AgentMessage =
  | ({ role: "user" } & UserMessage)
  | ({ role: "assistant" } & AssistantMessage)
  | ({ role: "toolResult" } & ToolResultMessage);

/**
 * Streaming deltas carried by `message_update` (model crate
 * `AssistantMessageEvent`, `tag = "type"`, snake_case variants, camelCase
 * fields). Every variant includes `partial`, the assistant message
 * accumulated so far.
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

/** hand-agent `ToolResult` — payload of `tool_execution_end` / `_update`. */
export interface ToolResult {
  content: ToolResultContent[];
  details?: unknown;
  terminate?: boolean;
}

/**
 * hand-agent `AgentEvent` (`#[serde(tag = "type", rename_all = "snake_case",
 * rename_all_fields = "camelCase")]`); discriminate on `type`.
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

/** Mirrors `storage/types/agent_session.rs`. */
export interface AgentSession {
  id: UUID;
  projectId?: UUID;
  /**
   * Provenance link to the AgentDefinition this session was instantiated
   * from; written once at creation, never rewritten by updates.
   */
  agentDefinitionId?: UUID;
  name: string;
  modelId?: string;
  providerId?: string;
  systemPrompt?: string;
  thinkingLevel?: string;
  temperature?: number;
  maxTokens?: number;
  workingDir?: string;
  enabledTools: string[];
  /** Per-session MCP server bindings injected into the agent loop as tools. */
  mcpServers: McpServerConfig[];
  toolExecutionMode?: string;
  messageCount: number;
  lastMessageAt?: Timestamp;
  createdAt: Timestamp;
  updatedAt: Timestamp;
}

/**
 * `payload` is the serialized hand-agent Message, typed as the `AgentMessage`
 * union so the timeline reducer can consume it without `any`.
 */
export interface AgentSessionMessage {
  id: UUID;
  sessionId: UUID;
  seq: number;
  role: string;
  payload: AgentMessage;
  createdAt: Timestamp;
}

export interface CreateAgentSessionRequest {
  name: string;
  /**
   * When provided, the backend overrides workingDir with project.path.
   * Missing project → NOT_FOUND; stale project directory → VALIDATION_ERROR;
   * neither writes a row.
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
  mcpServers?: McpServerConfig[];
  toolExecutionMode?: string;
}

/**
 * Overrides for instantiating a session from an AgentDefinition (mirrors the
 * backend request). All optional — the definition snapshot fills the gaps.
 * The backend resolves the working-dir policy from the definition's
 * `workingDirMode` ("none" forces plain chat, "required" errors without a
 * working context, "optional"/NULL passes through), then applies these fields.
 */
export interface InstantiateAgentSessionRequest {
  name?: string;
  projectId?: UUID;
  workingDir?: string;
  modelId?: string;
  providerId?: string;
}

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
  mcpServers?: McpServerConfig[];
  toolExecutionMode?: string;
}

/**
 * Image attachment sent with a turn's input (mirrors the backend
 * `AgentRunAttachment`). `data` is raw bytes (`number[]` → Rust `Vec<u8>`).
 * Only `image/*` mimes are assembled into `ImageContent` blocks; the frontend
 * pre-filters on selection and the backend defensively skips non-images.
 */
export interface AgentRunAttachment {
  name: string;
  mimeType: string;
  data: number[];
}

/** `agent_stream_event` payload: one AgentEvent tagged with its sessionId. */
export interface AgentStreamEventPayload {
  sessionId: UUID;
  event: AgentEvent;
}

/** `agent_stream_error` payload: run-level sanitized error envelope, emitted before closed. */
export interface AgentStreamErrorPayload {
  sessionId: UUID;
  error: {
    code: string;
    message: string;
    hint?: string;
  };
}

/** `agent_stream_closed` payload: run-termination signal, exactly once per run. */
export interface AgentStreamClosedPayload {
  sessionId: UUID;
}

/**
 * `agent_approval_request` payload, emitted when a dangerous tool
 * (write/edit/bash) is called; the backend then awaits a decision keyed by
 * `requestId` (uuid v4; duplicate or unknown ids are idempotent no-ops).
 * `callId` matches the transcript's toolCallId.
 *
 * `args` is the complete argument set about to execute (bash command string,
 * write/edit path + content). The approval dialog must render it in full —
 * what is shown is what runs, never truncated past judging its danger.
 */
export interface AgentApprovalRequest {
  sessionId: UUID;
  callId: string;
  toolName: string;
  args: unknown;
  requestId: string;
}

/**
 * Mirrors the backend `ApprovalDecision` (serde snake_case); wire values verbatim.
 *  - `"deny"`: reject this call; the tool is cancelled and the model receives
 *    a rejected result.
 *  - `"allow_once"`: allow this call only; the same tool prompts again next time.
 *  - `"allow_always"`: allow this call and whitelist the tool for the rest of
 *    this session (in-memory set keyed by sessionId — not persisted, so it
 *    does not survive across sessions or restarts).
 */
export type ApprovalDecision = "deny" | "allow_once" | "allow_always";

/**
 * How one `ask_question` question is answered; mirrors the backend
 * `QuestionKind` (serde snake_case), wire values verbatim.
 *  - `"single"`: pick exactly one option.
 *  - `"multiple"`: pick any number of options.
 *  - `"text"`: free-form reply, no options.
 */
export type AgentQuestionKind = "single" | "multiple" | "text";

/** One selectable option of a choice question. */
export interface AgentQuestionOption {
  label: string;
  /** Optional one-line elaboration rendered under the label. */
  description?: string;
}

/**
 * One question of an `ask_question` call, already validated and normalized
 * backend-side (`type` is always one of the three kinds; choice questions
 * always carry 2-8 options, `text` carries none).
 */
export interface AgentQuestion {
  /** Stable per-call id (`q0`, `q1`, …); answers are keyed by it. */
  id: string;
  /** Very short chip label. */
  header: string;
  question: string;
  type: AgentQuestionKind;
  options: AgentQuestionOption[];
  /**
   * The panel blocks submission until this one is answered. The model opts in
   * per question; everything else stays skippable.
   */
  required: boolean;
}

/**
 * `agent_question_request` payload, emitted when the model calls
 * `ask_question`; the backend then parks the tool call on a oneshot keyed by
 * `requestId` (uuid v4; duplicate or unknown ids are idempotent no-ops) until
 * the panel answers. `callId` matches the transcript's toolCallId.
 */
export interface AgentQuestionRequest {
  sessionId: UUID;
  callId: string;
  requestId: string;
  questions: AgentQuestion[];
}

/**
 * One question's answer: the selected option labels, or a single free-text
 * value. An omitted entry (or an empty `values`) is reported to the model as
 * explicitly unanswered, so a partial submission never reads as a full one.
 */
export interface AgentQuestionAnswer {
  questionId: string;
  values: string[];
}

/**
 * Mirrors the backend `QuestionResponse` (serde-tagged on `kind`); wire shape
 * verbatim.
 *  - `answered`: the answers become the tool result the model reads.
 *  - `dismissed`: the user chose to keep talking instead of answering; the
 *    model is told to continue without the answers rather than re-ask.
 */
export type AgentQuestionResponse =
  | { kind: "answered"; answers: AgentQuestionAnswer[] }
  | { kind: "dismissed" };

/**
 * `agent_session_lifecycle` payload, discriminated on `kind`. Independent of
 * the run channels — these are not run events and never enter the
 * `agent_stream_event` reducer, so closed-once is unaffected.
 *  - `compaction_start`: auto-compaction began; drives the "compacting" hint.
 *  - `compaction_end`: `summary` is the context digest — intentionally never
 *    rendered into the timeline, only used to clear the hint.
 *  - `session_info_changed`: session metadata (currently only name) changed;
 *    updates the sidebar title in place. `name` may be null (title cleared).
 */
export type AgentSessionLifecyclePayload =
  | { sessionId: UUID; kind: "compaction_start" }
  | { sessionId: UUID; kind: "compaction_end"; summary: string }
  | { sessionId: UUID; kind: "session_info_changed"; name: string | null };
