<script lang="ts">
  import { ChevronDown, ChevronRight } from "@lucide/svelte";
  import { renderMarkdown, markdownInteractions } from "$lib/utils";
  import { t } from "$lib/i18n";

  interface Props {
    // Streaming accumulated text, or the thinking block of a committed message.
    thinking: string;
    // Only affects the title text.
    isStreaming?: boolean;
  }

  let { thinking, isStreaming = false }: Props = $props();

  // Collapsed by default while streaming; completed messages start expanded.
  let expanded = $state(!isStreaming);

  function toggle() {
    expanded = !expanded;
  }
</script>

<div class="mb-4">
  <button
    class="flex items-center gap-1 my-2 text-left hover:bg-base-300 rounded-full py-1 px-2"
    onclick={toggle}
  >
    {#if expanded}
      <ChevronDown size={16} class="text-base-content" />
    {:else}
      <ChevronRight size={16} class="text-base-content" />
    {/if}
    <span class="text-sm font-medium text-base-content/80">
      {isStreaming
        ? t("agent.thinkingBlock.streaming")
        : t("agent.thinkingBlock.title")}
    </span>
  </button>

  {#if expanded}
    <div
      class="mt-2 mb-6 px-4 text-sm border-l border-[var(--hairline)] text-base-content/80 break-words leading-relaxed markdown-content"
      use:markdownInteractions
    >
      {@html renderMarkdown(thinking)}
    </div>
  {/if}
</div>
