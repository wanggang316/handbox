<script lang="ts">
  import { untrack } from "svelte";
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { Bot } from "@lucide/svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentApprovalStore } from "$lib/states/agentApproval.svelte";
  import { agentProjectState } from "$lib/states/agentProject.svelte";
  import AgentSessionHeader from "$lib/components/agentsession/AgentSessionHeader.svelte";
  import AgentInput from "$lib/components/agentsession/AgentInput.svelte";
  import AgentTimeline from "$lib/components/agentsession/AgentTimeline.svelte";
  import AgentApprovalModal from "$lib/components/agentsession/AgentApprovalModal.svelte";

  // 当前选中的 Agent 会话 ID（来自 ?id= 查询参数）
  let sessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : "",
  );

  // 侧栏 AgentProjectList 挂载时已拉取项目列表，这里只读以区分引导文案分支。
  const hasProjects = $derived(agentProjectState.projects.length > 0);

  // 记录最近打开的会话，供切换回 Agent 模式时恢复（VAL-MODE-005）
  $effect(() => {
    if (sessionId) {
      uiState.setLastAgentSessionId(sessionId);
    }
  });

  // 列表拉取的在途去重标记：列表确为空时 `loadSessions` 对 `sessions` 的
  // 赋值（新数组引用）会重触发下方 effect，不去重会无限重拉。完成（.then/
  // .catch）即重置，去重只约束在途窗口，同一 id 之后仍可再次发起拉取。
  let probedSessionId = "";

  // 恢复指针失效（id 指向已删 session 等）：清当前会话、清掉失效的
  // lastAgentSessionId，并回 Agent 落地页（VAL-CROSS-013 / 009）。
  // replaceState 避免返回键回到死 id 再次触发重定向。
  function handleMissingSession(id: string) {
    agentSessionState.currentSession = null;
    // untrack：本函数会在下方 effect 内同步调用，指针的读-写不进依赖
    // （对齐 AgentProjectList 折叠展开的 untrack 惯例），避免冗余重跑。
    untrack(() => {
      if (uiState.lastAgentSessionId === id) {
        uiState.setLastAgentSessionId(null);
      }
    });
    goto("/agent", { replaceState: true });
  }

  // 将 store 当前会话与 ?id= 同步。
  // 列表可能尚未加载（直接打开 /agent?id= 时），此时先拉取列表再定位。
  // 工作目录是否仍存在于磁盘只在 M2 工具调用阶段才有意义，此处仅渲染、绝不崩溃。
  $effect(() => {
    if (!browser) {
      return;
    }
    if (!sessionId) {
      // 回到落地页：清空当前会话。
      agentSessionState.currentSession = null;
      return;
    }
    const id = sessionId;
    if (agentSessionActions.setCurrentById(id)) {
      return;
    }
    if (agentSessionState.sessions.length === 0) {
      if (probedSessionId === id) {
        // 本 id 拉取在途：失效判定交由下方 .then，避免重拉循环。
        return;
      }
      probedSessionId = id;
      agentSessionActions
        .loadSessions()
        .then(() => {
          probedSessionId = "";
          // 用户可能已离开该 id（guard 后仅在仍停留时处理失效指针）；
          // 拉取失败走 catch 只记录，不误判为指针失效。
          if (sessionId === id && !agentSessionActions.setCurrentById(id)) {
            handleMissingSession(id);
          }
        })
        .catch((error) => {
          probedSessionId = "";
          console.error("Failed to load agent sessions:", error);
        });
      return;
    }
    // 列表已就绪但查不到：指针指向不存在的会话，优雅回落地态。
    handleMissingSession(id);
  });

  const currentSession = $derived(agentSessionState.currentSession);

  // 打开会话时 seed 已提交 transcript（按 sessionId 分键，互不干扰）。
  $effect(() => {
    if (!browser || !sessionId) {
      return;
    }
    agentRunStore.loadTranscript(sessionId).catch((error) => {
      console.error("Failed to load agent transcript:", error);
    });
  });

  // 当前会话的运行 view-model（响应式 getter；用于空态判定，渲染由 AgentTimeline 消费）。
  const runState = $derived(
    sessionId ? agentRunStore.runStateFor(sessionId) : null,
  );

  // 是否处于空态（无任何已提交消息、无流式内容、无错误、未运行）。
  const isEmpty = $derived(
    !runState ||
      (runState.messages.length === 0 &&
        !runState.streamingText &&
        !runState.thinkingText &&
        !runState.error &&
        !runState.isRunning),
  );

  // 当前会话的待审批请求（危险工具调用 → 弹审批弹窗、对话暂停，VAL-CAPERM-001）。
  const pendingApproval = $derived(
    sessionId ? agentApprovalStore.pendingFor(sessionId) : null,
  );

  // 用户决策：allow → 工具执行、对话继续（VAL-CAPERM-003）；deny → 工具被 Cancel、
  // 模型收被拒结果、对话继续不中断（VAL-CAPERM-005）。store 先清键关弹窗再回灌。
  function handleApprovalRespond(allow: boolean) {
    if (!sessionId) return;
    void agentApprovalStore.respond(sessionId, allow);
  }
</script>

<!-- Agent 模式落地页（将被 (app) 分组布局包裹） -->
<div class="flex-1 flex flex-col h-full">
  {#if sessionId}
    <!-- 已选中会话：Header（顶部）+ 内容区（完整 timeline 渲染）+ input 槽。 -->
    <AgentSessionHeader />

    <!--
      内容区：完整 timeline 渲染 —— 用户气泡 / 助手 markdown / 思考块 / 用量 / 多轮 / 错误态。
      AgentTimeline 消费 `agentRunStore.runStateFor(sessionId)` 这一稳定 getter；
      工具卡片留给 M2（toolResult / toolcall 块当前仅占位）。
    -->
    {#if isEmpty}
      <div
        class="flex-1 flex flex-col items-center justify-center text-base-content/40"
      >
        <Bot size={40} class="mb-3 opacity-20" />
        <p class="text-sm">开始与 {currentSession?.name ?? "Agent"} 对话</p>
      </div>
    {:else}
      <AgentTimeline {sessionId} />
    {/if}

    <!-- Input 槽：纯文本 composer（textarea + 模型/思考选择 + 发送/停止）。 -->
    <!--
      `{#key currentSession.id}` 强制 per-session 重新挂载 AgentInput：切换会话时
      销毁旧实例、重建新实例，使所有瞬时 composer 态（input / attachments /
      forced chip / slash 浮层）全部回到初值，绝不在会话 A 与 B 间串台
      （VAL-SLASH-023）。组件实例被复用是底层 bug；重新挂载即正确语义。
    -->
    <div class="shrink-0 border-t border-base-300 px-4 py-3">
      {#if currentSession}
        {#key currentSession.id}
          <AgentInput session={currentSession} />
        {/key}
      {/if}
    </div>

    <!-- 工具审批弹窗：危险工具调用待决期间弹出、对话暂停；允许 / 拒绝后关闭、
         决策经 store 回灌后端（VAL-CAPERM-001/003/005）。按当前会话分键。 -->
    {#if pendingApproval}
      <AgentApprovalModal
        request={pendingApproval}
        onRespond={handleApprovalRespond}
      />
    {/if}
  {:else}
    <!-- 空落地页：无任何新建动作，引导走侧栏常驻入口（VAL-CREATE-005） -->
    <div class="flex-1 flex flex-col items-center justify-center text-base-content/50">
      <Bot size={48} class="mb-4 opacity-20" />
      {#if hasProjects}
        <p class="text-sm">在左侧选择一个会话，或在项目上点 + 新建</p>
      {:else}
        <p class="text-sm">先在左侧点 + 选择项目目录</p>
      {/if}
    </div>
  {/if}
</div>
