<script lang="ts">
  import { Loader2, XCircle, ChevronDown } from "@lucide/svelte";
  import { renderCodeBlock } from "$lib/utils/code";
  import { parseMcpToolName, toolArgSummary } from "$lib/utils/toolCall";
  import { resolveToolIcon } from "$lib/constants/agentTools";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import { mcpState } from "$lib/states/mcp.svelte";
  import { findToolByName } from "$lib/states/toolDefinition.svelte";
  import { t } from "$lib/i18n";
  import type { ToolCallView } from "$lib/states/agentRun.svelte";
  import type { ToolResultContent } from "$lib/types/agentSession";

  interface Props {
    // Normalized tool-call view-model: live (tool_execution events) and
    // restored (committed toolcall + toolResult) sources share this shape, so
    // the same call renders as the same row either way.
    toolCall: ToolCallView;
  }

  let { toolCall }: Props = $props();

  // A user-defined tool carries its own icon and label; falling back to the
  // built-in registry covers every other name. Undefined once the definition is
  // deleted, so an old transcript keeps rendering under the generic glyph.
  const custom = $derived(findToolByName(toolCall.toolName));
  const ToolIcon = $derived(
    custom ? resolveAgentIcon(custom.icon) : resolveToolIcon(toolCall.toolName),
  );

  // An MCP tool registers as `mcp__<serverId>__<tool>`: the row shows the tool
  // under its own name, prefixed by the server it came from (two servers can
  // both expose a `search`). The id is a uuid, so the label comes from the
  // server list — absent until MCP settings have been visited this session,
  // which drops the prefix rather than showing a uuid.
  const mcp = $derived(parseMcpToolName(toolCall.toolName));
  const serverLabel = $derived.by(() => {
    if (!mcp) return "";
    const server = mcpState.servers.find((s) => s.id === mcp.serverId);
    return server?.displayName || server?.name || "";
  });
  // The row shows what the tool is called in the UI; the registration name the
  // model actually used stays on the row's title attribute.
  const displayName = $derived(
    custom?.displayName ||
      mcp?.tool ||
      toolCall.toolName ||
      t("agent.toolCall.fallbackName"),
  );

  // What the call operates on, on the collapsed row: the path it reads, the
  // command it runs. The expanded body carries the arguments in full.
  const summary = $derived(toolArgSummary(toolCall.args));

  const isError = $derived(toolCall.status === "error");

  // Running and failed are the states worth a word; a completed call says so by
  // having a result to open, and stays a quiet line.
  const statusDisplay = $derived.by(() => {
    switch (toolCall.status) {
      case "executing":
        return {
          text: t("agent.toolCall.executing"),
          icon: Loader2,
          animate: true,
        };
      case "error":
        return {
          text: t("agent.toolCall.error"),
          icon: XCircle,
          animate: false,
        };
      default:
        return null;
    }
  });

  // Render args (any shape) as a formatted JSON code block.
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

  // Joined text blocks; image blocks render separately.
  const textResult = $derived.by(() =>
    (toolCall.result ?? [])
      .filter((block): block is Extract<ToolResultContent, { type: "text" }> =>
        block.type === "text",
      )
      .map((block) => block.text)
      .join("\n"),
  );

  // Image result blocks render as <img>, not raw base64 text.
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

  const hasArgs = $derived(
    toolCall.args !== undefined && toolCall.args !== null,
  );
  const hasResult = $derived(textResult.length > 0 || imageResults.length > 0);
  // Nothing behind the disclosure → a plain line, same as a hook notice with no
  // execution capture.
  const expandable = $derived(hasArgs || hasResult);
</script>

<!-- A tool call reads as one line in the transcript — icon, name, subject,
     state — matching the hook-notice rows it sits between. The arguments and
     the result stay behind the native disclosure. -->
{#snippet identity()}
  <ToolIcon size={12} class="shrink-0" />
  {#if serverLabel}
    <span class="shrink-0 text-base-content/50">{serverLabel}</span>
  {/if}
  <span class="shrink-0 font-medium">{displayName}</span>
  {#if summary}
    <span class="truncate font-mono text-base-content/50">{summary}</span>
  {/if}
  {#if statusDisplay}
    {@const StatusIcon = statusDisplay.icon}
    <span
      class="flex shrink-0 items-center gap-1 {isError
        ? 'text-error'
        : 'text-info'}"
    >
      <StatusIcon size={12} class={statusDisplay.animate ? "animate-spin" : ""} />
      <span>{statusDisplay.text}</span>
    </span>
  {/if}
{/snippet}

{#if expandable}
  <details class="tool-call group px-3 py-1.5">
    <summary
      class="flex cursor-pointer list-none items-center gap-2 text-xs transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)] {isError
        ? 'text-error'
        : 'text-base-content/70 hover:text-base-content'}"
      title={toolCall.toolName}
    >
      {@render identity()}
      <ChevronDown
        size={12}
        class="shrink-0 opacity-60 transition-transform group-open:rotate-180"
      />
    </summary>

    <div
      class="mt-1.5 ml-5 max-h-80 space-y-2 overflow-auto text-[11px] leading-relaxed"
    >
      {#if hasArgs}
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
              class="mb-2 h-auto max-w-full rounded-md"
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
  </details>
{:else}
  <div
    class="flex items-center gap-2 px-3 py-1.5 text-xs {isError
      ? 'text-error'
      : 'text-base-content/70'}"
    title={toolCall.toolName}
  >
    {@render identity()}
  </div>
{/if}

<style>
  /* WebKit draws its own disclosure marker on <summary>; the row supplies its
     own chevron instead. */
  .tool-call summary::-webkit-details-marker {
    display: none;
  }
</style>
