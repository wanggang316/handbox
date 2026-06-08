<script lang="ts">
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { Bot } from "@lucide/svelte";
  import { uiState } from "$lib/states/ui.svelte";

  // 当前选中的 Agent 会话 ID（来自 ?id= 查询参数）
  let sessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : "",
  );

  // 记录最近打开的会话，供切换回 Agent 模式时恢复（VAL-MODE-005）
  $effect(() => {
    if (sessionId) {
      uiState.setLastAgentSessionId(sessionId);
    }
  });
</script>

<!-- Agent 模式落地页（将被 (app) 分组布局包裹） -->
<div class="flex-1 flex flex-col h-full">
  <div
    class="flex-1 flex flex-col items-center justify-center text-base-content/50"
  >
    <Bot size={48} class="mb-4 opacity-20" />
    {#if sessionId}
      <p class="text-sm">Agent 会话 {sessionId}</p>
    {:else}
      <p class="text-sm">选择或新建一个 Agent 会话</p>
    {/if}
  </div>
</div>
