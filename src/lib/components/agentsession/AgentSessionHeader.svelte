<script lang="ts">
  /**
   * Agent 会话头部：显示当前会话的名称 + 模型 + (可选) 工作目录 / 思考强度。
   * 由 `agentSessionState.currentSession` 驱动，故重新打开会话时配置即可见。
   */
  import { Bot, FolderOpen, Brain } from "@lucide/svelte";
  import { agentSessionState } from "$lib/states/agentSession.svelte";

  const session = $derived(agentSessionState.currentSession);
</script>

{#if session}
  <header
    class="flex items-center gap-3 px-4 py-2.5 border-b border-base-300 shrink-0"
  >
    <Bot size={18} class="opacity-60 shrink-0" />
    <div class="flex flex-col min-w-0">
      <span class="text-sm font-medium text-base-content truncate">
        {session.name}
      </span>
      <div
        class="flex items-center gap-3 text-xs text-base-content/50 mt-0.5"
      >
        {#if session.modelId}
          <span class="truncate">{session.modelId}</span>
        {/if}
        {#if session.thinkingLevel && session.thinkingLevel !== "off"}
          <span class="flex items-center gap-1 shrink-0">
            <Brain size={12} />
            {session.thinkingLevel}
          </span>
        {/if}
        {#if session.workingDir}
          <span class="flex items-center gap-1 min-w-0">
            <FolderOpen size={12} class="shrink-0" />
            <span class="truncate">{session.workingDir}</span>
          </span>
        {/if}
      </div>
    </div>
  </header>
{/if}
