<script lang="ts">
  import {
    Loader2,
    CheckCircle2,
    XCircle,
    ChevronRight,
    ChevronDown,
  } from "@lucide/svelte";
  import { renderCodeBlock } from "$lib/utils/code";
  import { t } from "$lib/i18n";
  import type { ToolCallView } from "$lib/states/agentRun.svelte";
  import type { ToolResultContent } from "$lib/types/agentSession";

  interface Props {
    // 归一化的工具调用 view-model：live（tool_execution 事件）与 restored
    // （committed toolcall + toolResult）两条来源统一到此形状，故同一调用
    // 无论实时还是还原都由本组件渲染同一张卡（VAL-TOOLS-004）。
    toolCall: ToolCallView;
  }

  let { toolCall }: Props = $props();

  let expanded = $state(false);

  function toggle() {
    expanded = !expanded;
  }

  // 状态指示（执行中 spinner / 完成 check / 失败 mark），中文标签。
  const statusDisplay = $derived.by(() => {
    switch (toolCall.status) {
      case "executing":
        return {
          text: t("agent.toolCall.executing"),
          icon: Loader2,
          color: "text-info",
          animate: true,
        };
      case "completed":
        return {
          text: t("agent.toolCall.completed"),
          icon: CheckCircle2,
          color: "text-success",
          animate: false,
        };
      case "error":
        return {
          text: t("agent.toolCall.error"),
          icon: XCircle,
          color: "text-error",
          animate: false,
        };
    }
  });

  const StatusIcon = $derived(statusDisplay.icon);
  const isError = $derived(toolCall.status === "error");

  // 把 args（任意结构）渲染为格式化 JSON 代码块。
  function renderArgs(args: unknown): string {
    if (args === undefined || args === null) return "";
    let formatted: string;
    if (typeof args === "string") {
      try {
        formatted = JSON.stringify(JSON.parse(args), null, 2);
      } catch {
        formatted = args;
      }
    } else {
      formatted = JSON.stringify(args, null, 2);
    }
    return renderCodeBlock(formatted, { language: "json", variant: "compact" });
  }

  // 结果文本块拼接（image 块单独按图片渲染）。
  const textResult = $derived.by(() =>
    (toolCall.result ?? [])
      .filter((block): block is Extract<ToolResultContent, { type: "text" }> =>
        block.type === "text",
      )
      .map((block) => block.text)
      .join("\n"),
  );

  // 结果中的 image 块：read_file 等返回的图片按 <img> 渲染，而非原始 base64 文本。
  const imageResults = $derived.by(() =>
    (toolCall.result ?? []).filter(
      (block): block is Extract<ToolResultContent, { type: "image" }> =>
        block.type === "image",
    ),
  );

  function renderResultText(text: string): string {
    return renderCodeBlock(text, { variant: "compact" });
  }

  function imageSrc(
    block: Extract<ToolResultContent, { type: "image" }>,
  ): string {
    return `data:${block.mimeType};base64,${block.data}`;
  }

  const hasResult = $derived(textResult.length > 0 || imageResults.length > 0);
</script>

<div
  class={`rounded-lg border bg-base-300 text-xs transition-colors ${
    isError
      ? "border-error/40 hover:bg-error/10"
      : "border-[var(--hairline)] hover:bg-base-300/80"
  }`}
>
  <!-- header：展开/收起 + 工具名 + 状态。 -->
  <div class="flex items-center justify-between gap-2">
    <button
      type="button"
      class="flex flex-1 items-center gap-2 text-left p-2"
      onclick={toggle}
    >
      {#if expanded}
        <ChevronDown size={14} class="shrink-0 text-base-content" />
      {:else}
        <ChevronRight size={14} class="shrink-0 text-base-content" />
      {/if}

      <div class="flex flex-col gap-1">
        <div class="text-sm text-base-content">
          {toolCall.toolName || t("agent.toolCall.fallbackName")}
        </div>
      </div>
    </button>

    <div class="flex items-center justify-end gap-2 px-2 py-1">
      <span
        class={`text-[10px] ${statusDisplay.color} flex items-center gap-1`}
      >
        <StatusIcon
          size={12}
          class={statusDisplay.animate ? "animate-spin" : ""}
        />
        <span>{statusDisplay.text}</span>
      </span>
    </div>
  </div>

  {#if expanded}
    <div
      class="p-3 space-y-2 rounded-b-lg text-[11px] leading-relaxed max-h-80 overflow-auto border-t border-[var(--hairline)]"
    >
      {#if toolCall.args !== undefined && toolCall.args !== null}
        <div>
          <div class="mb-1 text-[10px] text-base-content/70">Request</div>
          <div class="flex-1 break-words">
            {@html renderArgs(toolCall.args)}
          </div>
        </div>
      {/if}

      {#if hasResult}
        <div>
          <div class="mb-1 text-[10px] text-base-content/70">Response</div>

          {#each imageResults as image, idx (idx)}
            <img
              src={imageSrc(image)}
              alt={t("agent.toolCall.resultImageAlt")}
              class="max-w-full h-auto rounded-md mb-2"
            />
          {/each}

          {#if textResult}
            <div class="flex-1 break-words">
              {@html renderResultText(textResult)}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
