<script lang="ts">
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { Bot } from "@lucide/svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import AgentSessionCreateModal from "$lib/components/agentsession/AgentSessionCreateModal.svelte";
  import { goto } from "$app/navigation";
  import type { AgentSession } from "$lib/types";

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

  function handleCreated(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }
</script>

<!-- Agent 模式落地页（将被 (app) 分组布局包裹） -->
<div class="flex-1 flex flex-col h-full">
  {#if sessionId}
    <!-- 已选中会话：渲染会话占位（header/timeline 由后续 feature 接管）。 -->
    <div class="flex-1 flex flex-col items-center justify-center text-base-content/50">
      <Bot size={48} class="mb-4 opacity-20" />
      <p class="text-sm">{currentSession?.name ?? `Agent 会话 ${sessionId}`}</p>
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
