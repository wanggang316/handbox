<script lang="ts">
  import { ChevronDown, ChevronRight } from "@lucide/svelte";
  import { renderMarkdown, markdownInteractions } from "$lib/utils";
  import { t } from "$lib/i18n";

  interface Props {
    // 思考内容：流式累积文本，或已提交助手消息的 thinking 块内容。
    thinking: string;
    // 是否处于活跃流式状态（仅影响标题文案）。
    isStreaming?: boolean;
  }

  let { thinking, isStreaming = false }: Props = $props();

  // 流式期间默认收起，完成的消息默认展开（镜像 chat reasoning 块行为）。
  let expanded = $state(!isStreaming);

  function toggle() {
    expanded = !expanded;
  }
</script>

<div class="mb-4">
  <!-- 思考标题，可点击折叠 -->
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

  <!-- 思考内容，根据展开状态显示 -->
  {#if expanded}
    <div
      class="mt-2 mb-6 px-4 text-sm border-l border-[var(--hairline)] text-base-content/80 break-words leading-relaxed markdown-content"
      use:markdownInteractions
    >
      {@html renderMarkdown(thinking)}
    </div>
  {/if}
</div>
