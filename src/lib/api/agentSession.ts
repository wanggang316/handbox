/**
 * Agent Session 相关 API 封装
 *
 * 镜像 `api/chat.ts` / `api/message.ts` 的形态：每个函数经 `apiCall(...)` 调用
 * 对应的 snake_case Tauri 命令，参数以 Tauri 期望的 camelCase key 传入。
 * `listenToAgentStreamEvents` 镜像 `api/message.ts:listenToStreamEvents`。
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

/**
 * 创建新的 Agent Session
 * 后端签名: agent_session_create(request: CreateAgentSessionRequest)
 */
export async function createAgentSession(
  request: CreateAgentSessionRequest,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_create", { request });
}

/**
 * 从一个 AgentDefinition 实例化 Agent Session。
 *
 * 统一的「用此 Agent」入口：后端 `agent_session_create_from_definition` 快照
 * definition 的能力集（enabledTools←builtinTools、mcpServers←definition）、按其
 * workingDirMode 裁决工作目录策略，再以 `overrides` 覆盖（name/project/workingDir/
 * model/provider）。chat-class（workingDirMode="none" 或无目录）退化为纯对话。
 * 后端签名: agent_session_create_from_definition(definitionId, overrides?)
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
 * 将一个**已存在**会话就地重指到另一个 AgentDefinition —— 不新建会话行。
 *
 * 仅在会话**尚无任何消息**时调用：用户在输入框切换 Agent 而当前会话「一句话
 * 都没说过」，直接把它重指到新定义（重新快照能力集、改写 provenance），保留
 * 会话 id 与 transcript。语义同 `createSessionFromDefinition`，但复用现有 id。
 * 后端签名: agent_session_reinstantiate_from_definition(sessionId, definitionId, overrides?)
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

/**
 * 获取 Agent Session 列表
 */
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

/**
 * 获取 Agent Session 详情
 */
export async function getAgentSession(sessionId: UUID): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_get", { sessionId });
}

/**
 * 重命名 Agent Session
 */
export async function renameAgentSession(
  sessionId: UUID,
  name: string,
): Promise<AgentSession> {
  return apiCall<AgentSession>("agent_session_rename", { sessionId, name });
}

/** `agent_session_update_field` 可更新的字段名（camelCase，后端 match 即用此键）。 */
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

/**
 * 更新 Agent Session 单个字段
 * @param sessionId Session ID
 * @param fieldName 字段名（camelCase）
 * @param value 字段值，null 表示清空
 */
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

/**
 * 删除 Agent Session
 */
export async function deleteAgentSession(sessionId: UUID): Promise<void> {
  return apiCall<void>("agent_session_delete", { sessionId });
}

/**
 * 获取 Agent Session 的 transcript
 */
export async function getAgentSessionMessages(
  sessionId: UUID,
): Promise<AgentSessionMessage[]> {
  const list = await apiCall<AgentSessionMessage[]>("agent_session_messages", {
    sessionId,
  });
  return list || [];
}

/**
 * 启动一次 Agent run（流式）。
 *
 * 立即返回；真实输出经 `agent_stream_event` / `agent_stream_closed`
 * （以及 run-level 错误的 `agent_stream_error`）异步抵达。
 * 后端签名: agent_run_stream(request: AgentRunRequest { sessionId, input, attachments, forcedSkills })
 *
 * `attachments` 为可选图片附件；缺省时后端走纯文本路径。
 * `forcedSkills` 为本回合强制加载的 skill 名（顺序即注入序）；后端按此 list
 * 把每个存活 skill 的 body 逐字注入装配期 system_prompt（单回合，不持久化）。
 * 缺省空数组即旧三字段行为（serde default，后端 `forced_skills` 为空）。
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
 * 向某个会话进行中的 run 注入一条 steering 消息。
 *
 * 后端 `agent_run_steer(sessionId, text)` 把消息压入活跃 run 的 steering 队列，
 * 在 turn 边界 drain；空/纯空白文本与无活跃 run 均为干净 no-op。
 * 不起第二个 run（run 进行中调 `agent_run_stream` 会得到 AGENT_RUN_ALREADY_ACTIVE）。
 */
export async function steerAgentRun(
  sessionId: UUID,
  text: string,
): Promise<void> {
  await apiCall<void>("agent_run_steer", { sessionId, text });
}

/**
 * 中止某个 Agent 会话的活跃 run（对无活跃 run 为干净 no-op）。
 */
export async function abortAgentRun(sessionId: UUID): Promise<void> {
  await apiCall<void>("agent_run_abort", { sessionId });
}

/**
 * 回灌一次工具审批决策（含作用域），唤醒后端正在 await 的 `PermissionExtension` 钩子。
 *
 * 危险工具（write/edit/bash）调用时后端 emit `agent_approval_request` 并 await 一个
 * 以 `requestId` 为键的 oneshot；弹窗回答后经本封装调 `agent_approval_respond`，
 * `decision` 三态（作用域显式）：
 *  - `"deny"` → 工具被 Cancel、模型收被拒结果、对话继续不中断。
 *  - `"allow_once"` → 本次允许（Continue），不记忆；同工具下次仍弹窗。
 *  - `"allow_always"` → 本次允许且**本会话**始终允许该工具，同会话同工具后续调用
 *    不再弹窗、直接执行（后端进程内存集，按 sessionId 键控、不落 DB/文件 →
 *    不跨会话、不跨重启）。
 *
 * 重复 / 未知 `requestId` 在后端是幂等 no-op，故前端竞态重复回答安全。
 */
export async function respondAgentApproval(
  requestId: string,
  decision: ApprovalDecision,
): Promise<void> {
  await apiCall<void>("agent_approval_respond", { requestId, decision });
}

/**
 * Agent 流式事件处理器集合。
 */
export interface AgentStreamEventHandlers {
  onEvent?: (payload: AgentStreamEventPayload) => void;
  onError?: (payload: AgentStreamErrorPayload) => void;
  onClosed?: (payload: AgentStreamClosedPayload) => void;
  /**
   * 会话生命周期信号（compaction / session-info）。与 run 三通道并列、独立——
   * 不进 run reducer，故不影响 closed-once。compaction 用于「整理上下文中」指示，
   * session-info 用于侧栏标题即时更新。
   */
  onLifecycle?: (payload: AgentSessionLifecyclePayload) => void;
  /**
   * 工具审批请求（危险工具 write/edit/bash 调用时后端 emit 并 await 决策）。与
   * lifecycle 同属并列、独立通道——不进 run reducer，不影响 closed-once；驱动审批
   * 弹窗弹出、对话暂停，决策经 `respondAgentApproval` 回灌。
   */
  onApprovalRequest?: (payload: AgentApprovalRequest) => void;
}

/**
 * 监听 Agent 流式事件。
 *
 * 订阅四个 Tauri 事件通道并分发到对应处理器；返回一个解除全部监听的函数。
 *  - `agent_stream_event`      -> `handlers.onEvent`
 *  - `agent_stream_error`      -> `handlers.onError`
 *  - `agent_stream_closed`     -> `handlers.onClosed`
 *  - `agent_session_lifecycle` -> `handlers.onLifecycle`
 *  - `agent_approval_request`  -> `handlers.onApprovalRequest`
 */
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
 * 提取一条 Agent 消息的纯文本（拼接所有 text 块；忽略 thinking / toolcall / image）。
 *
 * `user` 为纯字符串时直接返回；为内容块数组时拼接 text 块。`assistant` 拼接 text 块
 * （跳过 thinking / toolcall）。`toolResult` 不含可读译文，返回空串。供「一问一答」
 * 类消费（翻译、单词本历史）从 transcript / message_end 取助手回复正文。
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
 * 跑一回合 Agent 并把助手回复聚合成完整文本（「一问一答」便捷封装）。
 *
 * 用于翻译 / 划词等单轮、非交互式消费：它们不需要 timeline / 工具卡片 / 审批，只要
 * 「发一句、拿整段回复」。本封装在调用 `runAgentStream` **之前**先挂上一次性流式监听
 * （避免漏掉早到事件），按 `sessionId` 过滤本会话的事件，累积 `text_delta` 增量（经
 * `onDelta` 实时回流给调用方做流式展示），在 `agent_stream_closed` 以 message_end 的
 * 助手正文（缺失则回退到累积增量）resolve，在 `agent_stream_error` reject，无论成败都
 * 解除监听。
 *
 * 注意：会话须为纯对话能力集（无内置工具 / 无 MCP），否则模型可能走工具循环而非直接
 * 作答——翻译类 AgentDefinition 的 workingDirMode 退化为纯对话即满足。
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
          // run 在监听器解析前就已终结（极端时序）：直接解绑，prior finish 已 settle。
          fn();
          return;
        }
        unlisten = fn;
        // 监听就绪后再启动 run，避免漏掉早到的 text_delta / closed。
        runAgentStream(sessionId, input).catch((error) => {
          finish(() => reject(error));
        });
      })
      .catch((error) => {
        finish(() => reject(error));
      });
  });
}
