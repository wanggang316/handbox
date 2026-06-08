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
 * 工具调用（toolcall）的渲染留给 M2 —— 此处仅在 reduce 时留出 seam，不消费工具事件。
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
} from "$lib/types/agentSession";
import {
  listenToAgentStreamEvents,
  getAgentSessionMessages,
  abortAgentRun,
} from "$lib/api/agentSession";
import { agentSessionActions } from "$lib/states/agentSession.svelte";

/**
 * 单个会话的运行 view-model。
 *
 * `messages` 为已提交（finalized）消息序列；`streamingText` / `thinkingText` 为
 * 当前正在流式累积的助手文本/思考文本；`isRunning` 表示该会话存在活跃 run；
 * `error` 为该会话最近一次 run-level 错误（在 `agent_stream_closed` 之前抵达）。
 */
export interface AgentRunState {
  messages: AgentMessage[];
  streamingText: string;
  thinkingText: string;
  isRunning: boolean;
  error: string | null;
}

function createEmptyRunState(): AgentRunState {
  return {
    messages: [],
    streamingText: "",
    thinkingText: "",
    isRunning: false,
    error: null,
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
        // 整轮结束：以权威 messages 列表替换已提交序列并清理流式累积。
        state.messages = event.messages;
        state.streamingText = "";
        state.thinkingText = "";
        break;

      // turn_start / turn_end / tool_execution_* 在 M1 不消费（M2 timeline/tool seam）。
      default:
        break;
    }
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
   * timeline feature 直接消费此 getter；保持稳定的形状以便其无需重构即可接入。
   */
  runStateFor(sessionId: string): AgentRunState {
    return this.ensureState(sessionId);
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
