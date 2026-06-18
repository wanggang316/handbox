<script lang="ts">
  import {
    Pause,
    Loader2,
    CheckCircle2,
    XCircle,
    ChevronRight,
    ChevronDown,
  } from "lucide-svelte";
  import { messageStore } from "$lib/states";
  import type { ToolCall } from "$lib/types";
  import { renderCodeBlock } from "$lib/utils/code";
  import { t } from "$lib/i18n";

  interface Props {
    toolCalls?: ToolCall[];
    messageId?: string;
    isStreaming?: boolean;
  }

  let {
    toolCalls = [],
    messageId,
    isStreaming = false,
  }: Props = $props();

  const calls = $derived(() => toolCalls ?? []);

  const expandedTools = $state<Record<string, boolean>>({});

  function getToolKey(tool: ToolCall): string {
    return tool.id ?? `index-${tool.index}`;
  }

  function isExpanded(tool: ToolCall): boolean {
    return expandedTools[getToolKey(tool)] ?? false;
  }

  function toggleTool(tool: ToolCall) {
    const key = getToolKey(tool);
    expandedTools[key] = !isExpanded(tool);
  }

  function renderJsonBlock(content?: string): string {
    if (!content) return "";

    let formatted = content;

    try {
      const parsed = JSON.parse(content);
      formatted = JSON.stringify(parsed, null, 2);
    } catch {}

    return renderCodeBlock(formatted, { language: "json", variant: "compact" });
  }

  const isExecuting = $derived(() => {
    const list = calls();
    return list.some(call => call.executionStatus === "executing");
  });

  const pendingManualTools = $derived(() => {
    const list = calls();
    return list.filter(
      call => call.executionMode === "manual" && call.executionStatus === "pending"
    );
  });

  const showExecuteAllButton = $derived(() => pendingManualTools().length > 1);

  function getToolExecutionStatusDisplay(status?: string) {
    switch (status) {
      case "pending":
        return { text: t("chat.toolPending"), icon: Pause, color: "text-base-content/60" };
      case "executing":
        return { text: t("chat.toolExecuting"), icon: Loader2, color: "text-info", animate: true };
      case "completed":
        return { text: t("chat.toolCompleted"), icon: CheckCircle2, color: "text-success" };
      case "failed":
        return { text: t("chat.toolFailed"), icon: XCircle, color: "text-error" };
      default:
        return { text: t("chat.toolUnknown"), icon: Pause, color: "text-base-content/40" };
    }
  }

  async function handleExecuteAllToolCalls() {
    const list = calls();

    if (list.length === 0) {
      console.warn("没有找到工具调用");
      return;
    }

    if (!messageId) {
      console.error("消息 ID 不存在");
      return;
    }

    try {
      console.log("执行所有工具调用:", list);
      await messageStore.executeAllToolCalls(messageId, list);
    } catch (error) {
      console.error("执行工具调用失败:", error);
    }
  }

  async function handleExecuteSingleTool(toolCallId: string) {
    if (!messageId) {
      console.error("消息 ID 不存在");
      return;
    }

    if (!toolCallId) {
      console.error("工具调用 ID 不存在");
      return;
    }

    try {
      console.log("执行单个工具调用:", toolCallId);
      await messageStore.executeToolCall(messageId, toolCallId);
    } catch (error) {
      console.error("执行单个工具调用失败:", error);
    }
  }

  $effect(() => {
    const list = calls();
    if (!messageId || list.length === 0 || isStreaming) return;

    const autoExecuteCalls = list.filter(
      call => call.executionMode === "auto" && call.executionStatus === "pending"
    );

    if (autoExecuteCalls.length > 0) {
      const timeoutId = setTimeout(() => {
        handleExecuteAllToolCalls();
      }, 100);

      return () => clearTimeout(timeoutId);
    }
  });
</script>

{#if calls().length > 0}
  <div class="mb-4 p-0 text-sm text-base-content">
    <div class="space-y-2">
      {#each calls() as tool (tool.id || tool.index)}
        {@const statusDisplay = getToolExecutionStatusDisplay(tool.executionStatus)}
        <div class="rounded-lg border border-[var(--hairline)] bg-base-300 text-xs hover:bg-base-300/80 transition-colors">
          <!-- header -->
          <div class="flex items-center justify-between gap-2">
            <button
              type="button"
              class="flex flex-1 items-center gap-2 text-left p-2"
              onclick={() => toggleTool(tool)}
            >
              {#if isExpanded(tool)}
                <ChevronDown size={14} class="shrink-0 text-base-content" />
              {:else}
                <ChevronRight size={14} class="shrink-0 text-base-content" />
              {/if}

              <div class="flex flex-col gap-1">
                <div class="text-sm text-base-content">
                  {tool.function?.name || t("chat.toolFallbackName", { index: tool.index })}
                </div>
              </div>
            </button>

            <div class="flex items-center justify-end gap-2 px-2 py-1">
              <span class={`text-[10px] ${statusDisplay.color} flex items-center gap-1`}>
                {#if statusDisplay.icon}
                  <statusDisplay.icon size={12} class={statusDisplay.animate ? "animate-spin" : ""} />
                {/if}
                <span>{statusDisplay.text}</span>
              </span>

              {#if tool.executionMode === "manual"}
                {#if tool.executionStatus === "pending"}
                  <button
                    class="px-2 py-0.5 text-[10px] text-base-content/80"
                    onclick={() => handleExecuteSingleTool(tool.id || "")}
                    disabled={isExecuting()}
                  >
                    {t("chat.execute")}
                  </button>
                {:else if tool.executionStatus === "failed" || tool.executionStatus === "completed"}
                  <button
                    class="px-2 py-0.5 text-[10px] text-base-content/80"
                    onclick={() => handleExecuteSingleTool(tool.id || "")}
                    disabled={isExecuting()}
                  >
                    {t("chat.reExecute")}
                  </button>
                {/if}
              {/if}
            </div>
          </div>

          {#if isExpanded(tool)}
            <div class="p-3 space-y-2 rounded-b-lg text-[11px] leading-relaxed max-h-80 overflow-auto border-t border-[var(--hairline)]">
              {#if tool.function?.arguments}
                <div class="">
                  <div class="mb-1 text-[10px] text-base-content/70">Request</div>
                  <div class="flex-1 break-words">
                    {@html renderJsonBlock(tool.function.arguments)}
                  </div>
                </div>
              {/if}

              {#if tool.result}
                <div class="">
                  <div class="mb-1 text-[10px] text-base-content/70">Response</div>
                  <div class="flex-1 break-words">
                    {@html renderJsonBlock(tool.result)}
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}
