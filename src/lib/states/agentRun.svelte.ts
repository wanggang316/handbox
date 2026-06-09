/**
 * Agent 运行状态管理 - Svelte 5 runes
 *
 * 镜像 `states/message.svelte.ts` 的 reducer/listener 约定，但**按 sessionId 分键**：
 * 每个会话拥有独立的「已提交消息（transcript）」与「流式 view-model」，因此一个
 * 在后台流式的会话可以持续更新自身状态，而前台正在查看的是另一个会话（VAL-RUN-016）。
 *
 * 流式监听器在 store 单例构造时**一次性**建立（navigation-resilient）：store 单例在
 * 整个 app 生命周期只构造一次，监听器不随路由切换或 Chat<->Agent 模式切换而卸载
 * （VAL-MODE-006）。监听器只订阅 `agent_stream_*` 事件，与 chat 的 `message_stream_*`
 * 互不相干，因此切换不会影响 chat 流（VAL-MODE-007）。
 *
 * 工具调用（toolcall）在 M2 消费：`tool_execution_start/update/end` 事件按 `toolCallId`
 * 分键 reduce 成 live tool-call view-model（VAL-TOOLS-001/002/003/004），由 timeline
 * 渲染为工具卡片。已提交 transcript 里的 `toolcall` 内容块 + `toolResult` 消息（由
 * 持久化还原）与 live 状态调和：同一个 `toolCallId` 无论 live 还是 restored 都只呈现
 * 一张卡片。
 */

import type { UUID } from "$lib/types";
import type {
  AgentMessage,
  AgentSessionMessage,
  AgentEvent,
  AssistantMessageEvent,
  AgentStreamEventPayload,
  AgentStreamErrorPayload,
  AgentStreamClosedPayload,
  ToolResultContent,
} from "$lib/types/agentSession";
import {
  listenToAgentStreamEvents,
  getAgentSessionMessages,
  abortAgentRun,
} from "$lib/api/agentSession";
import { agentSessionActions } from "$lib/states/agentSession.svelte";

/**
 * 工具调用的归一化 view-model（卡片消费的统一形状）。
 *
 * live 路径（run 期间）由 `tool_execution_*` 事件 reduce 产生；restored 路径
 * （reload 后）由已提交的 `toolcall` 内容块 + 配对的 `toolResult` 消息归一化产生。
 * 两条路径都映射到此形状，使 `AgentToolCallCard` 无需区分来源即可渲染同一张卡片。
 *
 * `status`：`executing`（已 start、result 未到）/ `completed`（end 且非 error）/
 * `error`（end 且 isError，或 restored 的 `toolResult.isError`）。
 * `result` 为终态的工具结果内容块（text / image）；`executing` 时为 undefined。
 */
export type ToolCallStatus = "executing" | "completed" | "error";

export interface ToolCallView {
  toolCallId: string;
  toolName: string;
  args: unknown;
  status: ToolCallStatus;
  result?: ToolResultContent[];
}

/**
 * 单个会话的运行 view-model。
 *
 * `messages` 为已提交（finalized）消息序列；`streamingText` / `thinkingText` 为
 * 当前正在流式累积的助手文本/思考文本；`isRunning` 表示该会话存在活跃 run；
 * `error` 为该会话最近一次 run-level 错误（在 `agent_stream_closed` 之前抵达）。
 * `toolCalls` 为 live tool-call view-model，**按 `toolCallId` 分键**：同一调用
 * 的 start/update/end 落到同一条目，使卡片就地从 executing 翻转到终态（VAL-TOOLS-004）。
 */
export interface AgentRunState {
  messages: AgentMessage[];
  streamingText: string;
  thinkingText: string;
  isRunning: boolean;
  error: string | null;
  toolCalls: Record<string, ToolCallView>;
}

function createEmptyRunState(): AgentRunState {
  return {
    messages: [],
    streamingText: "",
    thinkingText: "",
    isRunning: false,
    error: null,
    toolCalls: {},
  };
}

class AgentRunStore {
  // 按 sessionId 分键的运行状态。每个会话独立，互不干扰。
  private states = $state<Record<string, AgentRunState>>({});

  // 一次性流式监听器的清理函数（store 生命周期内通常不调用）。
  private unlisten: (() => void) | null = null;

  // run 终结回调（VAL-PERSIST-011）：`agent_stream_closed` 抵达时触发。
  // 由侧栏状态层注册，用于刷新该会话的元数据 / 排序，而不让本 store 反向依赖
  // session 状态（保持单向：agentSession 不 import agentRun）。
  private onRunClosed: ((sessionId: string) => void) | null = null;

  constructor() {
    // 单例构造即建立监听器：navigation-resilient，跨路由与模式切换持续 reduce。
    void this.initListener();
  }

  /**
   * 建立全局 Agent 流式监听器（仅一次）。所有事件按 payload 的 sessionId 分发。
   */
  private async initListener(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listenToAgentStreamEvents({
        onEvent: (payload) => this.handleStreamEvent(payload),
        onError: (payload) => this.handleStreamError(payload),
        onClosed: (payload) => this.handleStreamClosed(payload),
      });
    } catch (error) {
      console.error("Failed to init agent stream listener:", error);
    }
  }

  /**
   * 取得（必要时初始化）某会话的可变运行状态。
   */
  private ensureState(sessionId: string): AgentRunState {
    if (!this.states[sessionId]) {
      this.states[sessionId] = createEmptyRunState();
    }
    return this.states[sessionId];
  }

  /**
   * 分发 `agent_stream_event`：按 sessionId 定位状态并 reduce 该会话的 AgentEvent。
   */
  private handleStreamEvent(payload: AgentStreamEventPayload): void {
    const { sessionId, event } = payload;
    this.reduceEvent(sessionId, event);
  }

  /**
   * 核心 reducer：镜像 message.svelte.ts 的流式约定，但以 sessionId 分键。
   */
  private reduceEvent(sessionId: string, event: AgentEvent): void {
    const state = this.ensureState(sessionId);

    switch (event.type) {
      case "agent_start":
        // 新 run 开始：标记运行中并清理上一轮的流式残留。
        state.isRunning = true;
        state.streamingText = "";
        state.thinkingText = "";
        state.error = null;
        break;

      case "message_start":
        // 一条消息开始（user / assistant / toolResult）：追加到已提交序列。
        this.appendMessage(sessionId, event.message);
        // 助手消息开始时清空流式累积，避免与上一条混叠。
        if (event.message.role === "assistant") {
          state.streamingText = "";
          state.thinkingText = "";
        }
        break;

      case "message_update":
        // 流式增量：仅更新文本/思考；工具事件留给 M2（此处不消费）。
        this.applyAssistantDelta(sessionId, event.assistantMessageEvent);
        break;

      case "message_end":
        // 一条消息结束：以终结 payload 覆盖已提交序列中的对应项，并清空流式累积。
        this.finalizeMessage(sessionId, event.message);
        state.streamingText = "";
        state.thinkingText = "";
        break;

      case "agent_end":
        // 整轮结束：**绝不**用 event.messages 覆盖已提交序列。
        // hand-agent 的 AgentEnd.messages 只含「本轮新增」消息（本回合的 user +
        // assistant），不含 seed 进来的历史 transcript。已提交序列已由
        // message_start/message_end 增量维护为 [history + 本轮]，若在此覆盖会把
        // 多轮历史抹成仅本轮两条，直到 reload 才恢复（违反 VAL-RUN-005）。
        // 故 agent_end 仅做流式残留清理，不触碰 state.messages。
        state.streamingText = "";
        state.thinkingText = "";
        break;

      case "tool_execution_start":
        // 工具开始执行：创建/追踪一条 tool-call 条目（args 已知、result 待定）。
        this.startToolCall(sessionId, event.toolCallId, event.toolName, event.args);
        break;

      case "tool_execution_update":
        // 流式部分结果：就地更新同一条目（不创建新卡）。
        this.updateToolCall(
          sessionId,
          event.toolCallId,
          event.toolName,
          event.args,
          event.partialResult.content,
        );
        break;

      case "tool_execution_end":
        // 工具执行结束：把同一条目翻转到终态（completed / error）并写入 result。
        this.endToolCall(
          sessionId,
          event.toolCallId,
          event.toolName,
          event.result.content,
          event.isError,
        );
        break;

      // turn_start / turn_end 在 M2 不消费（卡片由 message + tool_execution 事件驱动）。
      default:
        break;
    }
  }

  /**
   * `tool_execution_start`：按 `toolCallId` 建条目（已存在则保留终态/result，仅刷新
   * 已知字段——防御重复 start）。新条目进入 `executing`，result 待定。
   */
  private startToolCall(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    args: unknown,
  ): void {
    const state = this.ensureState(sessionId);
    const existing = state.toolCalls[toolCallId];
    state.toolCalls = {
      ...state.toolCalls,
      [toolCallId]: {
        toolCallId,
        toolName,
        args,
        status: existing?.status ?? "executing",
        result: existing?.result,
      },
    };
  }

  /**
   * `tool_execution_update`：就地更新同一 `toolCallId` 条目的部分结果，保持
   * `executing` 状态（同一张卡，不新建——VAL-TOOLS-004）。条目缺失（update 早于
   * start 的极端时序）则按 start 语义建条目。
   */
  private updateToolCall(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    args: unknown,
    partialResult: ToolResultContent[],
  ): void {
    const state = this.ensureState(sessionId);
    const existing = state.toolCalls[toolCallId];
    state.toolCalls = {
      ...state.toolCalls,
      [toolCallId]: {
        toolCallId,
        toolName,
        args: existing?.args ?? args,
        status: "executing",
        result: partialResult,
      },
    };
  }

  /**
   * `tool_execution_end`：把同一 `toolCallId` 条目翻转到终态（`isError` → `error`，
   * 否则 `completed`）并写入最终 result——卡片就地从 executing 转为终态（不新建卡）。
   */
  private endToolCall(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    result: ToolResultContent[],
    isError: boolean,
  ): void {
    const state = this.ensureState(sessionId);
    const existing = state.toolCalls[toolCallId];
    state.toolCalls = {
      ...state.toolCalls,
      [toolCallId]: {
        toolCallId,
        toolName,
        args: existing?.args,
        status: isError ? "error" : "completed",
        result,
      },
    };
  }

  /**
   * 应用助手流式增量。仅处理 text_delta / thinking_delta；其余增量（toolcall_* 等）
   * 在 M1 忽略，留作 M2 seam。
   */
  private applyAssistantDelta(
    sessionId: string,
    delta: AssistantMessageEvent,
  ): void {
    const state = this.ensureState(sessionId);
    switch (delta.type) {
      case "text_delta":
        state.streamingText += delta.delta;
        break;
      case "thinking_delta":
        state.thinkingText += delta.delta;
        break;
      default:
        break;
    }
  }

  /**
   * 追加一条消息到会话的已提交序列。
   */
  private appendMessage(sessionId: string, message: AgentMessage): void {
    const state = this.ensureState(sessionId);
    state.messages = [...state.messages, message];
  }

  /**
   * 以终结 payload 覆盖已提交序列中的「最后一条同角色消息」。
   *
   * 后端按 message_start -> message_update* -> message_end 顺序发送，故终结的
   * 消息应对应序列中最后一条同 role 的项；若找不到则追加（防御性）。
   */
  private finalizeMessage(sessionId: string, message: AgentMessage): void {
    const state = this.ensureState(sessionId);
    for (let i = state.messages.length - 1; i >= 0; i -= 1) {
      if (state.messages[i].role === message.role) {
        const next = [...state.messages];
        next[i] = message;
        state.messages = next;
        return;
      }
    }
    this.appendMessage(sessionId, message);
  }

  /**
   * 分发 `agent_stream_error`：为该会话设置错误 view-state（不清 isRunning，
   * closed 紧随其后才是回合终结信号）。
   */
  private handleStreamError(payload: AgentStreamErrorPayload): void {
    const state = this.ensureState(payload.sessionId);
    state.error = payload.error?.message ?? "Agent run error";
  }

  /**
   * 分发 `agent_stream_closed`：清 isRunning（每个 run 恰好一次），并通知
   * 已注册的 run-终结回调以刷新侧栏元数据（VAL-PERSIST-011）。回调抛错不影响
   * 本 store 的终结收尾。
   */
  private handleStreamClosed(payload: AgentStreamClosedPayload): void {
    const state = this.ensureState(payload.sessionId);
    state.isRunning = false;
    if (this.onRunClosed) {
      try {
        this.onRunClosed(payload.sessionId);
      } catch (error) {
        console.error("Agent run-closed callback failed:", error);
      }
    }
  }

  // ============================================
  // Public API
  // ============================================

  /**
   * 响应式 getter：返回某会话的运行 view-model（不存在则返回新建的空状态）。
   * timeline feature 直接消费此 getter；`toolCalls` 为 live tool-call view-model
   * （按 toolCallId 分键）。
   */
  runStateFor(sessionId: string): AgentRunState {
    return this.ensureState(sessionId);
  }

  /**
   * 把一个助手 `toolcall` 内容块归一化为卡片消费的 `ToolCallView`，调和 live 与
   * restored 两条来源：
   *  - run 期间：以 live `state.toolCalls[id]` 为准（携带 executing→终态的实时状态）。
   *  - reload 后：live 缺失，则用配对的已提交 `toolResult` 内容归一化（restored 路径）。
   *  - 都缺失：仅有 `toolcall` 块（结果尚未抵达 / 未持久化），呈现为 executing。
   *
   * 同一个 `toolCallId` 无论 live 还是 restored 都映射到同一张卡（VAL-TOOLS-004）。
   * `committedResult` 由 timeline 从已提交 transcript 里按 toolCallId 配对后传入。
   */
  toolCallViewFor(
    sessionId: string,
    toolCallId: string,
    toolName: string,
    args: unknown,
    committedResult?: { content: ToolResultContent[]; isError: boolean },
  ): ToolCallView {
    const live = this.states[sessionId]?.toolCalls[toolCallId];
    if (live) {
      // live 已有该调用：以其实时状态/结果为准，但回填 name/args（live end 事件
      // 可能未携带 args；toolcall 块始终有）。
      return {
        toolCallId,
        toolName: live.toolName || toolName,
        args: live.args ?? args,
        status: live.status,
        result: live.result,
      };
    }
    if (committedResult) {
      // restored：用配对的 toolResult 归一化为终态。
      return {
        toolCallId,
        toolName,
        args,
        status: committedResult.isError ? "error" : "completed",
        result: committedResult.content,
      };
    }
    // 仅有 toolcall 块、无结果：执行中（尚未结束或结果未持久化）。
    return {
      toolCallId,
      toolName,
      args,
      status: "executing",
      result: undefined,
    };
  }

  /**
   * 该会话是否存在活跃 run。
   */
  isRunning(sessionId: string): boolean {
    return this.states[sessionId]?.isRunning ?? false;
  }

  /**
   * 注册 run-终结回调（VAL-PERSIST-011）。每次 `agent_stream_closed` 抵达后以
   * 该会话 id 调用一次，供侧栏状态层刷新 messageCount / lastMessageAt / 排序。
   * 单例语义：仅保留最后一次注册（store 全生命周期只需一个 reactor）。
   */
  setOnRunClosed(callback: (sessionId: string) => void): void {
    this.onRunClosed = callback;
  }

  /**
   * 加载并 seed 某会话的已提交 transcript（打开会话时调用），按 seq 升序
   * （后端 `list_messages` 以 `ORDER BY seq ASC` 全量返回，无 LIMIT / 分页，故
   * 长 transcript 完整还原、不静默截断 —— VAL-PERSIST-004/005/007/008）。
   *
   * 不覆盖正在运行中的流式累积；仅写入已提交消息序列。
   *
   * 还原即「从存储重建」（VAL-PERSIST-006）：reopen 时除写入已提交序列外，还在
   * **无活跃 run** 时丢弃残留的 live `toolCalls`。store 单例 navigation-resilient、
   * 跨会话开合不卸载，上一轮 run 写入的 live tool-call 条目会存活；若不清，
   * `toolCallViewFor` 的 live 分支会优先于配对的已提交 `toolResult`，使卡片读取
   * 陈旧的 live 结果而非持久化结果（二者一致时侥幸正确，但违反「纯从存储重建」）。
   * 故无活跃 run 时清空 `toolCalls`，让 restored 路径据配对的 `toolResult` 重建终态卡。
   * run 进行中（reload 落在活跃会话上）则保留 live 条目，避免顶掉就地翻转的实时卡。
   *
   * 健壮性（VAL-PERSIST-012）：逐行隔离 payload 解析 —— 单条 payload 形状不可
   * 识别（非 user / assistant / toolResult，或缺失判别字段）时记录并跳过该行，
   * 绝不让一条坏行抛错而白屏整条 timeline；其余行照常渲染。
   */
  async loadTranscript(sessionId: UUID): Promise<void> {
    try {
      const rows: AgentSessionMessage[] =
        await getAgentSessionMessages(sessionId);
      const messages: AgentMessage[] = [];
      for (const row of rows) {
        const parsed = this.parseTranscriptRow(row);
        if (parsed) {
          messages.push(parsed);
        }
      }
      const state = this.ensureState(sessionId);
      state.messages = messages;
      // 无活跃 run 时丢弃残留 live tool-call，使卡片纯从配对的已提交 toolResult 重建。
      if (!state.isRunning) {
        state.toolCalls = {};
      }
    } catch (error) {
      console.error("Failed to load agent transcript:", error);
      const state = this.ensureState(sessionId);
      state.error = error instanceof Error ? error.message : "加载会话记录失败";
    }
  }

  /**
   * 校验并归一化单条 transcript 行的 payload。返回合法的 `AgentMessage`，
   * 或在 payload 缺失 / 形状不可识别 / 解析抛错时返回 `null`（调用方跳过该行）。
   */
  private parseTranscriptRow(row: AgentSessionMessage): AgentMessage | null {
    try {
      const payload = row?.payload as unknown;
      if (!payload || typeof payload !== "object") {
        console.warn(
          `Skipping corrupt agent transcript row (non-object payload, seq=${row?.seq}).`,
        );
        return null;
      }
      const role = (payload as { role?: unknown }).role;
      if (role !== "user" && role !== "assistant" && role !== "toolResult") {
        console.warn(
          `Skipping corrupt agent transcript row (unknown role=${String(role)}, seq=${row?.seq}).`,
        );
        return null;
      }
      return payload as AgentMessage;
    } catch (error) {
      console.warn(
        `Skipping corrupt agent transcript row (seq=${row?.seq}):`,
        error,
      );
      return null;
    }
  }

  /**
   * 中止某会话的活跃 run（透传到后端；对无活跃 run 为干净 no-op）。
   */
  async abort(sessionId: UUID): Promise<void> {
    try {
      await abortAgentRun(sessionId);
    } catch (error) {
      console.error("Failed to abort agent run:", error);
      throw error;
    }
  }

  /**
   * 清理某会话的运行状态（不影响其它会话）。
   */
  clear(sessionId: string): void {
    delete this.states[sessionId];
  }
}

// Export singleton instance（单例构造即建立 navigation-resilient 监听器）。
export const agentRunStore = new AgentRunStore();

// 一次性 wiring（与监听器同属 navigation-resilient 的 app 级接线）：每次 run 终结
// （`agent_stream_closed`）刷新侧栏该会话的元数据 / 排序（VAL-PERSIST-011）。
// 依赖方向单向（agentRun -> agentSession），agentSession 不反向 import 本模块。
agentRunStore.setOnRunClosed((sessionId) => {
  void agentSessionActions.refreshAfterRun(sessionId);
});
