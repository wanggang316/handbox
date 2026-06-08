<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { getAgentSessions } from "$lib/api/agentSession";
  import type { AgentSession } from "$lib/types";

  interface Props {
    activeId?: string;
  }

  let { activeId = "" }: Props = $props();

  let sessions = $state<AgentSession[]>([]);
  let isLoading = $state(true);

  onMount(async () => {
    try {
      sessions = await getAgentSessions();
    } catch (error) {
      console.error("Failed to load agent sessions:", error);
    } finally {
      isLoading = false;
    }
  });

  function handleSessionClick(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }
</script>

<div class="flex flex-col h-full">
  <!-- 标题 -->
  <div class="text-sm text-base-content/70 pb-2 pl-4 flex-shrink-0">Agent 会话</div>

  <!-- 会话列表 -->
  <div class="flex-1 overflow-y-auto space-y-0.5 px-2">
    {#if isLoading}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        加载中…
      </div>
    {:else if sessions.length === 0}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        还没有 Agent 会话
      </div>
    {:else}
      {#each sessions as session (session.id)}
        <button
          class="w-full py-0.5 px-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 {session.id ===
          activeId
            ? 'bg-base-300 text-base-content'
            : ''}"
          onclick={() => handleSessionClick(session)}
        >
          <span class="truncate block">{session.name}</span>
        </button>
      {/each}
    {/if}
  </div>
</div>
