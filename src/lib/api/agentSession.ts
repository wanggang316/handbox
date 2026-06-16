/**
 * Agent Session 相关 API 封装
 *
 * 镜像 `api/chat.ts` / `api/message.ts` 的形态：每个函数经 `apiCall(...)` 调用
 * 对应的 snake_case Tauri 命令，参数以 Tauri 期望的 camelCase key 传入。
 * `listenToAgentStreamEvents` 镜像 `api/message.ts:listenToStreamEvents`。
 */

import { apiCall } from "./index";
import { listen } from "@tauri-apps/api/event";
import type {
  UUID,
  AgentSession,
  AgentSessionMessage,
  CreateAgentSessionRequest,
  AgentRunAttachment,
  AgentStreamEventPayload,
  AgentStreamErrorPayload,
  AgentStreamClosedPayload,
  AgentSessionLifecyclePayload,
  AgentApprovalRequest,
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
  value: string | number | string[] | null,
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
 * 回灌一次工具审批决策，唤醒后端正在 await 的 `PermissionExtension` 钩子。
 *
 * 危险工具（write/edit/bash）调用时后端 emit `agent_approval_request` 并 await 一个
 * 以 `requestId` 为键的 oneshot；弹窗回答后经本封装调 `agent_approval_respond`：
 * allow=true → 工具执行（Continue）、对话继续；false → 工具被 Cancel、模型收被拒
 * 结果、对话继续不中断。重复 / 未知 `requestId` 在后端是幂等 no-op，故前端竞态
 * 重复回答安全。
 */
export async function respondAgentApproval(
  requestId: string,
  allow: boolean,
): Promise<void> {
  await apiCall<void>("agent_approval_respond", { requestId, allow });
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
