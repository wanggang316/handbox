<script lang="ts">
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { Bot } from "@lucide/svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentProjectState } from "$lib/states/agentProject.svelte";
  import AgentSessionHeader from "$lib/components/agentsession/AgentSessionHeader.svelte";
  import AgentInput from "$lib/components/agentsession/AgentInput.svelte";
  import AgentTimeline from "$lib/components/agentsession/AgentTimeline.svelte";

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
    const matched = agentSessionActions.setCurrentById(sessionId);
    if (!matched && agentSessionState.sessions.length === 0) {
      agentSessionActions
        .loadSessions()
        .then(() => agentSessionActions.setCurrentById(sessionId))
        .catch((error) => {
          console.error("Failed to load agent sessions:", error);
        });
    }
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
    <div class="shrink-0 border-t border-base-300 px-4 py-3">
      {#if currentSession}
        <AgentInput session={currentSession} />
      {/if}
    </div>
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
