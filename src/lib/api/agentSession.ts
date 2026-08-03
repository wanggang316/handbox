/**
 * Params are passed with camelCase keys; Tauri maps them onto the backend's
 * snake_case arguments.
 */

import { apiCall } from "./index";
import { listen } from "@tauri-apps/api/event";
import type { McpServerConfig } from "../types/llm";
import type { AgentMessage } from "../types/agentSession";
import type {
  UUID,
  AgentSession,
  AgentSessionMessage,
  CreateAgentSessionRequest,
  InstantiateAgentSessionRequest,
  AgentRunAttachment,
  AgentStreamEventPayload,
  AgentStreamErrorPayload,
  AgentStreamClosedPayload,
  AgentSessionLifecyclePayload,
  AgentApprovalRequest,
  ApprovalDecision,
} from "../types";

export async function createAgentSession(
  request: CreateAgentSessionRequest,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_create", { request });
}

/**
 * Instantiates a session from an AgentDefinition: the backend snapshots the
 * definition's capability set (builtin tools, MCP servers) and working-dir
 * policy, then applies `overrides` (name/project/workingDir/model/provider).
 * workingDirMode "none" or a missing directory degrades to plain chat.
 */
export async function createSessionFromDefinition(
  definitionId: UUID,
  overrides?: InstantiateAgentSessionRequest,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_create_from_definition", {
    definitionId,
    overrides,
  });
}

/**
 * Repoints an existing session to another AgentDefinition in place —
 * re-snapshots the capability set and rewrites provenance while keeping the
 * session id. Only valid while the session has no messages yet.
 */
export async function reinstantiateSessionFromDefinition(
  sessionId: UUID,
  definitionId: UUID,
  overrides?: InstantiateAgentSessionRequest,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_reinstantiate_from_definition", {
    sessionId,
    definitionId,
    overrides,
  });
}

export async function getAgentSessions(
  limit?: number,
  offset?: number,
): Promise<AgentSession[]> {
  const list = await apiCall<AgentSession[]>("agent_session_list", {
    limit,
    offset,
  });
  return list || [];
}

export async function getAgentSession(sessionId: UUID): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_get", { sessionId });
}

export async function renameAgentSession(
  sessionId: UUID,
  name: string,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_rename", { sessionId, name });
}

/** What a generated title is distilled from (backend `TitleScope`). */
export type TitleScope = "firstMessage" | "conversation";

/**
 * One-shot LLM completion using the session's own model/provider; the backend
 * persists the new name. `scope` picks the source text — the first user message
 * (default) or the conversation so far, for re-titling an evolving session.
 */
export async function generateAgentSessionTitle(
  sessionId: UUID,
  scope?: TitleScope,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_generate_title", {
    sessionId,
    scope,
  });
}

/** Field names accepted by `agent_session_update_field` (matched verbatim by the backend). */
export type AgentSessionField =
  | "name"
  | "modelId"
  | "providerId"
  | "systemPrompt"
  | "thinkingLevel"
  | "temperature"
  | "maxTokens"
  | "workingDir"
  | "enabledTools"
  | "mcpServers"
  | "toolExecutionMode";

/** Updates a single session field; `value: null` clears it. */
export async function updateAgentSessionField(
  sessionId: UUID,
  fieldName: AgentSessionField,
  value: string | number | string[] | McpServerConfig[] | null,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_update_field", {
    sessionId,
    fieldName,
    value,
  });
}

export async function deleteAgentSession(sessionId: UUID): Promise<void> {
  return apiCall<void>("agent_session_delete", { sessionId });
}

export async function getAgentSessionMessages(
  sessionId: UUID,
): Promise<AgentSessionMessage[]> {
  const list = await apiCall<AgentSessionMessage[]>("agent_session_messages", {
    sessionId,
  });
  return list || [];
}

/**
 * Starts a streaming run; returns immediately. Output arrives asynchronously
 * via `agent_stream_event` / `agent_stream_closed` / `agent_stream_error`.
 * `forcedSkills`: skill bodies injected into this turn's system prompt in list
 * order — single turn only, not persisted.
 */
export async function runAgentStream(
  sessionId: UUID,
  input: string,
  attachments: AgentRunAttachment[] = [],
  forcedSkills: string[] = [],
): Promise<void> {
  await apiCall<void>("agent_run_stream", {
    request: { sessionId, input, attachments, forcedSkills },
  });
}

/**
 * Queues a steering message for the session's active run; the queue is drained
 * at turn boundaries. Blank text or no active run is a clean no-op.
 */
export async function steerAgentRun(
  sessionId: UUID,
  text: string,
): Promise<void> {
  await apiCall<void>("agent_run_steer", { sessionId, text });
}

/** Aborts the session's active run; no-op when there is none. */
export async function abortAgentRun(sessionId: UUID): Promise<void> {
  await apiCall<void>("agent_run_abort", { sessionId });
}

/**
 * Answers a pending tool-approval request. `deny` cancels the tool call,
 * `allow_once` allows it once, `allow_always` also whitelists the tool for the
 * rest of this session (backend in-memory, keyed by sessionId — not persisted
 * across sessions or restarts). Duplicate or unknown `requestId`s are
 * idempotent no-ops, so racy duplicate answers are safe.
 */
export async function respondAgentApproval(
  requestId: string,
  decision: ApprovalDecision,
): Promise<void> {
  await apiCall<void>("agent_approval_respond", { requestId, decision });
}

export interface AgentStreamEventHandlers {
  onEvent?: (payload: AgentStreamEventPayload) => void;
  onError?: (payload: AgentStreamErrorPayload) => void;
  onClosed?: (payload: AgentStreamClosedPayload) => void;
  /**
   * Session lifecycle signals (compaction / session-info). Independent of the
   * run channels — never enters the run reducer, so closed-once is unaffected.
   */
  onLifecycle?: (payload: AgentSessionLifecyclePayload) => void;
  /**
   * Tool-approval requests for dangerous tools (write/edit/bash). Also
   * independent of the run channels; answer via `respondAgentApproval`.
   */
  onApprovalRequest?: (payload: AgentApprovalRequest) => void;
}

/** Subscribes to all agent event channels; returns a function that removes every listener. */
export async function listenToAgentStreamEvents(
  handlers: AgentStreamEventHandlers,
): Promise<() => void> {
  const listeners = [
    listen<AgentStreamEventPayload>("agent_stream_event", (event) => {
      handlers.onEvent?.(event.payload);
    }),
    listen<AgentStreamErrorPayload>("agent_stream_error", (event) => {
      handlers.onError?.(event.payload);
    }),
    listen<AgentStreamClosedPayload>("agent_stream_closed", (event) => {
      handlers.onClosed?.(event.payload);
    }),
    listen<AgentSessionLifecyclePayload>("agent_session_lifecycle", (event) => {
      handlers.onLifecycle?.(event.payload);
    }),
    listen<AgentApprovalRequest>("agent_approval_request", (event) => {
      handlers.onApprovalRequest?.(event.payload);
    }),
  ];

  const unlisten = await Promise.all(listeners);

  return () => {
    unlisten.forEach((fn) => fn());
  };
}

/**
 * Extracts a message's plain text by concatenating its text blocks, ignoring
 * thinking / toolcall / image blocks. toolResult messages yield "".
 */
export function agentMessageText(message: AgentMessage): string {
  if (message.role === "user") {
    if (typeof message.content === "string") return message.content;
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }
  if (message.role === "assistant") {
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }
  return "";
}

/**
 * Runs one turn and resolves with the full assistant reply (single Q&A helper
 * for non-interactive consumers such as translation). Deltas stream out via
 * `onDelta`; resolves with the message_end text, falling back to the
 * accumulated deltas.
 *
 * The session must have a plain-chat capability set (no builtin tools / MCP),
 * otherwise the model may enter a tool loop instead of answering directly.
 */
export async function runAgentTextTurn(
  sessionId: UUID,
  input: string,
  onDelta?: (text: string) => void,
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    let accumulated = "";
    let finalText: string | null = null;
    let settled = false;
    let unlisten: (() => void) | null = null;

    const cleanup = () => {
      if (unlisten) {
        unlisten();
        unlisten = null;
      }
    };
    const finish = (run: () => void) => {
      if (settled) return;
      settled = true;
      cleanup();
      run();
    };

    listenToAgentStreamEvents({
      onEvent: (payload) => {
        if (payload.sessionId !== sessionId) return;
        const { event } = payload;
        if (
          event.type === "message_update" &&
          event.assistantMessageEvent.type === "text_delta"
        ) {
          accumulated += event.assistantMessageEvent.delta;
          onDelta?.(accumulated);
        } else if (
          event.type === "message_end" &&
          event.message.role === "assistant"
        ) {
          finalText = agentMessageText(event.message);
        }
      },
      onError: (payload) => {
        if (payload.sessionId !== sessionId) return;
        finish(() =>
          reject(new Error(payload.error?.message ?? "Agent run error")),
        );
      },
      onClosed: (payload) => {
        if (payload.sessionId !== sessionId) return;
        finish(() => resolve(finalText ?? accumulated));
      },
    })
      .then((fn) => {
        if (settled) {
          // Run settled before the listener resolved; just unbind.
          fn();
          return;
        }
        unlisten = fn;
        // Start the run only after the listener is ready, so early events are not missed.
        runAgentStream(sessionId, input).catch((error) => {
          finish(() => reject(error));
        });
      })
      .catch((error) => {
        finish(() => reject(error));
      });
  });
}
