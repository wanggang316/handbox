/**
 * Agent 工具审批状态管理 - Svelte 5 runes
 *
 * 危险工具（write/edit/bash）调用时后端 `PermissionExtension` emit
 * `agent_approval_request` 并 await 用户决策；本 store 持有「待审批请求」并**按
 * sessionId 分键**，使审批弹窗弹出、对应会话的对话暂停（VAL-CAPERM-001），决策
 * 经 `respondAgentApproval(requestId, allow)` 回灌：allow → 工具执行、对话继续
 * （VAL-CAPERM-003）；deny → 工具被 Cancel、模型收被拒结果、对话继续不中断
 * （VAL-CAPERM-005）。
 *
 * 设计对齐 `agentRun.svelte.ts`：
 *  - 单例构造即建立**一次性** navigation-resilient 监听器（跨路由 / 模式切换不卸载）。
 *  - 监听器只订阅 `agent_approval_request` 通道，与 run 三通道及 chat 流互不相干。
 *  - 写路径（事件抵达 / 决策回灌）经响应式 map 更新，读路径用引用稳定的 getter。
 *
 * 与 run 状态独立：审批不进 run reducer、不影响 closed-once；run state 反映工具
 * 执行进度，审批 state 只反映「是否有待决策的危险调用」。AgentInput / 页面据
 * `pendingFor(sessionId)` 暂停输入并挂载弹窗。
 *
 * 作用域（本次允许 / 始终允许本会话）经 `decision` 三态回灌（m2-approval-scope）：
 * `allow_once` 一次性、`allow_always` 本会话记住该工具（后端进程内存集、不跨会话/
 * 重启）、`deny` 拒绝。边界 / 可达性 / 隔离留给后续 feature（m2-approval-edge-cases）。
 */

import type {
  AgentApprovalRequest,
  ApprovalDecision,
} from "$lib/types/agentSession";
import {
  listenToAgentStreamEvents,
  respondAgentApproval,
} from "$lib/api/agentSession";

class AgentApprovalStore {
  // 按 sessionId 分键的待审批请求。每个会话最多一个在途请求：后端在 await 决策
  // 期间不会为同一会话的同一 run 并发发起第二个危险调用（钩子链串行 await），故
  // 单值即可表达「该会话对话暂停于此请求」。新请求覆盖旧键是防御性兜底。
  private pending = $state<Record<string, AgentApprovalRequest>>({});

  // 一次性流式监听器的清理函数（store 生命周期内通常不调用）。
  private unlisten: (() => void) | null = null;

  constructor() {
    // 单例构造即建立监听器：navigation-resilient，跨路由与模式切换持续接收。
    void this.initListener();
  }

  /**
   * 建立全局 Agent 审批监听器（仅一次）。请求按 payload 的 sessionId 分键存入。
   */
  private async initListener(): Promise<void> {
    if (this.unlisten) {
      return;
    }
    try {
      this.unlisten = await listenToAgentStreamEvents({
        onApprovalRequest: (payload) => this.handleApprovalRequest(payload),
      });
    } catch (error) {
      console.error("Failed to init agent approval listener:", error);
    }
  }

  /**
   * 分发 `agent_approval_request`：把请求按 sessionId 记入待审批 map，使弹窗弹出
   * 且该会话对话暂停。展示的 `args` 即将执行的参数（VAL-CAPERM-002）。
   */
  private handleApprovalRequest(payload: AgentApprovalRequest): void {
    this.pending = { ...this.pending, [payload.sessionId]: payload };
  }

  // ============================================
  // Public API
  // ============================================

  /**
   * 响应式 getter：返回某会话当前待审批的请求（无则 `null`）。AgentInput 据此非空
   * 即暂停输入；页面据此挂载审批弹窗。**只读**：不在此写 `$state`（getter 被
   * `$derived` / 模板消费，写入会触发 Svelte 的 state_unsafe_mutation）。
   */
  pendingFor(sessionId: string): AgentApprovalRequest | null {
    return this.pending[sessionId] ?? null;
  }

  /**
   * 该会话是否有待审批请求（对话是否因此暂停）。
   */
  hasPending(sessionId: string): boolean {
    return !!this.pending[sessionId];
  }

  /**
   * 回应一次待审批请求（含作用域）：先本地清键（弹窗即时关闭、对话立刻不再显示
   * 暂停态），再把 `decision` 回灌后端。`requestId` 取自当前 pending 条目，故只回应
   * 当前展示的请求。
   *
   * `decision` 三态：`allow_once` 本次允许（不记忆）、`allow_always` 本会话始终允许
   * 该工具（后端按 sessionId 键控的进程内存集，同会话同工具后续不再弹窗）、`deny`
   * 拒绝。`allow_always` 的作用域记忆在后端，前端只透传 decision。
   *
   * 先清键再回灌：UI 反馈即时；回灌失败仅记录，不回滚清键——后端对未知 / 重复
   * `requestId` 幂等 no-op，重新弹出反而会让用户对着一个后端可能已放弃的请求二次
   * 决策。无待审批请求时为干净 no-op。
   */
  async respond(sessionId: string, decision: ApprovalDecision): Promise<void> {
    const request = this.pending[sessionId];
    if (!request) {
      return;
    }
    this.clear(sessionId);
    try {
      await respondAgentApproval(request.requestId, decision);
    } catch (error) {
      console.error("Failed to respond to agent approval:", error);
    }
  }

  /**
   * 清除某会话的待审批请求（关闭弹窗 / 解除暂停）。非响应式安全：仅在写路径调用。
   */
  private clear(sessionId: string): void {
    if (!this.pending[sessionId]) {
      return;
    }
    const next = { ...this.pending };
    delete next[sessionId];
    this.pending = next;
  }
}

// Export singleton instance（单例构造即建立 navigation-resilient 监听器）。
export const agentApprovalStore = new AgentApprovalStore();
