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
  import AgentSessionCreateModal from "$lib/components/agentsession/AgentSessionCreateModal.svelte";
  import AgentSessionHeader from "$lib/components/agentsession/AgentSessionHeader.svelte";
  import { goto } from "$app/navigation";
  import type { AgentSession } from "$lib/types";
  import type { AgentMessage } from "$lib/types/agentSession";

  // 当前选中的 Agent 会话 ID（来自 ?id= 查询参数）
  let sessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : "",
  );

  let showCreateModal = $state(false);

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

  // 当前会话的运行 view-model（响应式 getter；timeline feature 将复用此入口）。
  const runState = $derived(
    sessionId ? agentRunStore.runStateFor(sessionId) : null,
  );

  function handleCreated(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }

  // 从 AgentMessage 提取可显示文本（M1 最小渲染；富 timeline 为下一个 feature）。
  function messageText(message: AgentMessage): string {
    if (message.role === "user") {
      if (typeof message.content === "string") {
        return message.content;
      }
      return message.content
        .map((block) => (block.type === "text" ? block.text : ""))
        .join("");
    }
    if (message.role === "assistant") {
      return message.content
        .map((block) => (block.type === "text" ? block.text : ""))
        .join("");
    }
    // toolResult：M1 仅提取文本块（工具卡片为 M2）。
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }

  function messageRoleLabel(message: AgentMessage): string {
    switch (message.role) {
      case "user":
        return "你";
      case "assistant":
        return "助手";
      default:
        return "工具";
    }
  }
</script>

<!-- Agent 模式落地页（将被 (app) 分组布局包裹） -->
<div class="flex-1 flex flex-col h-full">
  {#if sessionId}
    <!-- 已选中会话：Header（顶部）+ 内容区（最小 timeline view-model 渲染）+ input 槽。 -->
    <AgentSessionHeader />

    <!--
      内容区：M1 最小渲染 —— 列出已提交消息文本 + 当前流式文本行 + 错误行。
      富 timeline（思考块 / 工具卡片）为下一个 feature（m1-agent-timeline-render），
      它将消费 `agentRunStore.runStateFor(sessionId)` 这一稳定 getter。
    -->
    <div class="flex-1 overflow-y-auto px-4 py-4 flex flex-col gap-3">
      {#if runState}
        {#each runState.messages as message, i (i)}
          <div class="flex flex-col gap-1">
            <span class="text-xs text-base-content/40">
              {messageRoleLabel(message)}
            </span>
            <p class="text-sm text-base-content whitespace-pre-wrap break-words">
              {messageText(message)}
            </p>
          </div>
        {/each}

        {#if runState.thinkingText}
          <div class="flex flex-col gap-1">
            <span class="text-xs text-base-content/40">思考中</span>
            <p
              class="text-sm text-base-content/60 italic whitespace-pre-wrap break-words"
            >
              {runState.thinkingText}
            </p>
          </div>
        {/if}

        {#if runState.streamingText}
          <div class="flex flex-col gap-1">
            <span class="text-xs text-base-content/40">助手</span>
            <p class="text-sm text-base-content whitespace-pre-wrap break-words">
              {runState.streamingText}
            </p>
          </div>
        {/if}

        {#if runState.error}
          <div
            class="px-3 py-2 rounded-md bg-error/10 text-error text-sm whitespace-pre-wrap break-words"
          >
            {runState.error}
          </div>
        {/if}

        {#if runState.messages.length === 0 && !runState.streamingText && !runState.error}
          <div
            class="flex-1 flex flex-col items-center justify-center text-base-content/40"
          >
            <Bot size={40} class="mb-3 opacity-20" />
            <p class="text-sm">开始与 {currentSession?.name ?? "Agent"} 对话</p>
          </div>
        {/if}
      {/if}
    </div>

    <!-- Input 槽占位：发送 / 停止由下一个 feature（m1-agent-input）接管。 -->
    <div class="shrink-0 border-t border-base-300 px-4 py-3">
      <div
        class="text-xs text-base-content/30 text-center select-none"
        aria-hidden="true"
      >
        输入框（下一步）
      </div>
    </div>
  {:else}
    <!-- 空落地页 -->
    <div class="flex-1 flex flex-col items-center justify-center text-base-content/50">
      <Bot size={48} class="mb-4 opacity-20" />
      <p class="text-sm">选择或新建一个 Agent 会话</p>
      <button
        class="mt-4 px-3 py-1.5 text-[13px] rounded-md bg-primary text-primary-content hover:opacity-90"
        onclick={() => (showCreateModal = true)}
      >
        新建 Agent 会话
      </button>
    </div>
  {/if}
</div>

<AgentSessionCreateModal bind:open={showCreateModal} onCreated={handleCreated} />
