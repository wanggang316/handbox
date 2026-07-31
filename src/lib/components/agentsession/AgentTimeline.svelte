<script lang="ts">
  import { ChartNoAxesColumn, Check, Copy } from "@lucide/svelte";
  import {
    renderMarkdown,
    markdownInteractions,
    copyToClipboard,
  } from "$lib/utils";
  import { t } from "$lib/i18n";
  import Tooltip from "$lib/components/ui/Tooltip.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import type {
    AgentMessage,
    ToolResultContent,
  } from "$lib/types/agentSession";
  import AgentThinkingBlock from "./AgentThinkingBlock.svelte";
  import AgentToolCallCard from "./AgentToolCallCard.svelte";
  import HtmlCard from "./HtmlCard.svelte";
  import { RENDER_CARD_TOOL_NAME } from "./renderCard";
  import AppPill from "./AppPill.svelte";
  import { RENDER_APP_TOOL_NAME } from "./renderApp";
  import {
    resolveSpec,
    looksLikeStreamingSpec,
  } from "$lib/components/genui/jsonui/resolveSpec";
  import { uiRegistry } from "$lib/components/genui/jsonui/registry";
  import { Renderer, JsonUIProvider } from "@json-render/svelte";

  interface Props {
    sessionId: string;
    /** Folded render_app artifact title (from +page); AppPill fallback. */
    appTitle?: string;
  }

  let { sessionId, appTitle }: Props = $props();

  const runState = $derived(agentRunStore.runStateFor(sessionId));

  // Extract plain text from a user message (content is a string or block array).
  function userText(message: Extract<AgentMessage, { role: "user" }>): string {
    if (typeof message.content === "string") {
      return message.content;
    }
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }

  // Join all text blocks (thinking / toolcall blocks render separately).
  function assistantText(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ): string {
    return message.content
      .map((block) => (block.type === "text" ? block.text : ""))
      .join("");
  }

  // Join all thinking block text (empty when absent).
  function assistantThinking(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ): string {
    return message.content
      .map((block) => (block.type === "thinking" ? block.thinking : ""))
      .join("");
  }

  // Whether the turn shows token usage. Aborted/errored turns get a synthetic
  // all-zero usage from hand-agent (never the previous turn's numbers); this
  // view-layer gate also hides usage for turns that produced no tokens, so a
  // misleading "input 0 · output 0" never sits next to an error message.
  function hasUsage(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ): boolean {
    if (message.stopReason === "aborted" || message.stopReason === "error") {
      return false;
    }
    const u = message.usage;
    return !!u && (u.input > 0 || u.output > 0 || u.totalTokens > 0);
  }

  // Input/output tokens, spelled out for the usage tooltip and its aria-label.
  function usageLabel(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ): string {
    const input = t("agent.timeline.usageInput", { count: message.usage.input });
    const output = t("agent.timeline.usageOutput", {
      count: message.usage.output,
    });
    return `${input} · ${output}`;
  }

  // Index of the message whose copy button is showing its confirmation tick;
  // one at a time, so a second copy moves the tick rather than stacking ticks.
  let copiedIndex = $state<number | null>(null);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function copyMessage(index: number, text: string) {
    await copyToClipboard(text);
    copiedIndex = index;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copiedIndex = null), 1500);
  }

  $effect(() => () => clearTimeout(copiedTimer));

  // Tool-call blocks keep the assistant content's source order, so parallel
  // calls render as cards in issue order, not completion order.
  function assistantToolCalls(
    message: Extract<AgentMessage, { role: "assistant" }>,
  ) {
    return message.content.filter((block) => block.type === "toolcall");
  }

  // Committed toolResult messages indexed by toolCallId, so the restored path
  // (no live state after reload) can reconcile toolcall blocks into final-state
  // cards. Live state wins during a run; this index is the fallback.
  const committedToolResults = $derived.by(() => {
    const map = new Map<
      string,
      { content: ToolResultContent[]; isError: boolean }
    >();
    for (const message of runState.messages) {
      if (message.role === "toolResult") {
        map.set(message.toolCallId, {
          content: message.content,
          isError: message.isError,
        });
      }
    }
    return map;
  });

  // Normalize a toolcall block into the card view-model: live first, restored fallback.
  function toolCallView(block: Extract<AgentMessage, { role: "assistant" }>["content"][number]) {
    if (block.type !== "toolcall") {
      throw new Error("toolCallView expects a toolcall block");
    }
    return agentRunStore.toolCallViewFor(
      sessionId,
      block.id,
      block.name,
      block.arguments,
      committedToolResults.get(block.id),
    );
  }

  // Index of the in-progress assistant skeleton: the reducer appends the
  // assistant message at message_start (empty content, zero usage) while
  // deltas flow through streamingText/thinkingText, so the committed loop must
  // suppress the skeleton and leave it to the LIVE view.
  // Suppress only while the last assistant message has no committed content.
  // Once message_end writes real content, return -1 so the finished message
  // renders from the committed sequence at once — otherwise the gap between
  // message_end and agent_stream_closed (isRunning still true, streamingText
  // cleared) flashes answer → pulse-dot → answer.
  const liveAssistantIndex = $derived.by(() => {
    if (!runState.isRunning) {
      return -1;
    }
    const last = runState.messages.length - 1;
    if (last < 0 || runState.messages[last].role !== "assistant") {
      return -1;
    }
    const lastMsg = runState.messages[last] as Extract<
      AgentMessage,
      { role: "assistant" }
    >;
    // A message with toolcall blocks counts as "has content": its cards render
    // in the committed branch (reconciling live state by toolCallId in place);
    // treating it as an empty skeleton would let the pulse-dot displace the
    // cards while tools execute.
    const hasContent =
      assistantText(lastMsg).length > 0 ||
      assistantThinking(lastMsg).length > 0 ||
      assistantToolCalls(lastMsg).length > 0;
    return hasContent ? -1 : last;
  });

  // The LIVE view shows only while running with no displayable finished
  // content: streaming text/thinking exists, or the last skeleton is still
  // empty. Once the message is finished and streaming has cleared, hiding it
  // removes the pulse-dot flash between message_end and agent_stream_closed.
  const showLiveView = $derived(
    runState.isRunning &&
      (!!runState.streamingText ||
        !!runState.thinkingText ||
        liveAssistantIndex >= 0),
  );

  let messagesContainer: HTMLDivElement;

  function scrollToBottom() {
    if (messagesContainer) {
      messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }
  }

  // Session switch / transcript arrival: pin to bottom synchronously after DOM
  // update. The component instance is reused across sessions and keeps the old
  // scroll position; relying on the delayed scroll alone would show one frame
  // at the old position and jump ~100ms later.
  $effect(() => {
    void sessionId;
    void runState.messages.length;
    scrollToBottom();
  });

  // Scroll on message-count change; the delay re-pins after late layout
  // (markdown/images growing the content).
  $effect(() => {
    if (runState.messages.length > 0) {
      setTimeout(scrollToBottom, 100);
    }
  });

  // Scroll as streaming text/thinking grows.
  $effect(() => {
    if (runState.streamingText || runState.thinkingText) {
      setTimeout(scrollToBottom, 50);
    }
  });
</script>

<!-- The message stream is content: bubbles, markdown replies, tool-card bodies
     and error text must all be selectable (buttons stay unselectable globally). -->
<div
  bind:this={messagesContainer}
  class="flex-1 overflow-y-auto select-text scroll-column"
>
  <div class="chat-column py-4 space-y-6">
    <!-- Committed messages. messages is append-only (the reducer only appends
         or finalizes in place, never reorders), so index keys reuse DOM safely;
         cards key by toolCallId so their state never shifts with the index. -->
    {#each runState.messages as message, i (i)}
      {#if message.role === "user"}
        <div class="flex justify-end">
          <div class="flex flex-col items-end">
            <div
              class="inline-block max-w-full px-3.5 py-2 rounded-lg bg-base-200 text-base-content border border-[var(--hairline)]"
            >
              <div
                class="whitespace-pre-wrap break-words text-[15px] leading-[1.6] text-left"
              >
                {userText(message)}
              </div>
            </div>
          </div>
        </div>
      {:else if message.role === "assistant" && i !== liveAssistantIndex}
        <!-- Finished assistant message; the in-progress skeleton renders in the
             LIVE view below and is skipped here. -->
        <div class="flex flex-col gap-2">
          <div class="flex-1 min-w-0">
            {#if assistantThinking(message)}
              <AgentThinkingBlock thinking={assistantThinking(message)} />
            {/if}

            {#if assistantText(message)}
              {@const genuiSpec = resolveSpec(assistantText(message))}
              {#if genuiSpec}
                <!-- GenUI card: the whole reply is a valid JSON-Render spec →
                     rendered as an interactive card; non-spec replies fall
                     through to markdown. -->
                <JsonUIProvider initialState={{}}>
                  <Renderer spec={genuiSpec} registry={uiRegistry} />
                </JsonUIProvider>
              {:else}
                <div
                  class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
                  use:markdownInteractions
                >
                  {@html renderMarkdown(assistantText(message))}
                </div>
              {/if}
            {/if}

            <!-- Tool-call cards in source order; one card per toolCallId flips
                 from executing to final in place (live), reconciled from the
                 committed toolResult after reload (restored). -->
            {#if assistantToolCalls(message).length}
              <div class="mt-2 space-y-2">
                {#each assistantToolCalls(message) as block (block.id)}
                  {#if block.name === RENDER_CARD_TOOL_NAME}
                    <!-- render_card is purely presentational: its arguments are
                         the card, so it renders as an inline sandbox card
                         rather than a generic tool card. -->
                    <HtmlCard toolCall={toolCallView(block)} />
                  {:else if block.name === RENDER_APP_TOOL_NAME}
                    <!-- render_app: the timeline shows only a clickable pill;
                         the app itself lives in the side AppPanel. -->
                    <AppPill
                      toolCall={toolCallView(block)}
                      {sessionId}
                      fallbackTitle={appTitle}
                    />
                  {:else}
                    <AgentToolCallCard toolCall={toolCallView(block)} />
                  {/if}
                {/each}
              </div>
            {/if}

            {#if message.stopReason === "error" && message.errorMessage}
              <div
                class="mt-2 px-3 py-2 rounded-md bg-error/10 text-error text-sm whitespace-pre-wrap break-words"
              >
                {message.errorMessage}
              </div>
            {/if}

            <!-- Message actions. Usage is an icon rather than a running total:
                 the numbers matter when asked for, not on every turn. -->
            {#if assistantText(message) || hasUsage(message)}
              <div class="mt-2 flex items-center gap-0.5 text-base-content/40">
                {#if assistantText(message)}
                  <Tooltip
                    content={copiedIndex === i
                      ? t("agent.timeline.copied")
                      : t("agent.timeline.copy")}
                  >
                    <button
                      class="p-1 rounded-md transition-colors hover:bg-base-200 hover:text-base-content/70"
                      aria-label={t("agent.timeline.copy")}
                      onclick={() => copyMessage(i, assistantText(message))}
                    >
                      {#if copiedIndex === i}
                        <Check size={14} />
                      {:else}
                        <Copy size={14} />
                      {/if}
                    </button>
                  </Tooltip>
                {/if}

                {#if hasUsage(message)}
                  <Tooltip content={usageLabel(message)}>
                    <!-- Not focusable: one tab stop per message would bury the
                         real controls; the label carries the numbers for AT. -->
                    <span
                      class="flex p-1 transition-colors hover:text-base-content/70"
                      role="img"
                      aria-label={usageLabel(message)}
                    >
                      <ChartNoAxesColumn size={14} />
                    </span>
                  </Tooltip>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}
      <!-- toolResult messages are not rendered standalone: the paired toolcall
           card presents them inside the assistant turn, avoiding a detached
           tool-result block. -->
    {/each}

    <!-- LIVE streaming view: growing thinking block + streaming text. -->
    {#if showLiveView}
      <div class="flex flex-col gap-2">
        <div class="flex-1 min-w-0">
          {#if runState.thinkingText}
            <AgentThinkingBlock thinking={runState.thinkingText} isStreaming />
          {/if}

          {#if runState.streamingText}
            {#if looksLikeStreamingSpec(runState.streamingText)}
              <!-- Spec-shaped stream: unclosed JSON is not rendered char-by-char
                   (it would flash raw JSON); show a placeholder until the
                   committed branch renders the GenUI card at message_end. -->
              <div
                class="py-2 flex items-center gap-2 text-sm text-base-content/50"
              >
                <div
                  class="h-3 w-3 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
                ></div>
                <span>{t("agent.timeline.genuiStreaming")}</span>
              </div>
            {:else}
              <div
                class="flex-1 break-words text-[15px] leading-[1.6] markdown-content"
                use:markdownInteractions
              >
                {@html renderMarkdown(runState.streamingText)}
              </div>
            {/if}
          {:else if !runState.thinkingText}
            <!-- Stream started but no content yet: progress indicator. -->
            <div class="py-2 text-base-content flex items-center">
              <div
                class="h-4 w-4 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
              ></div>
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Compaction indicator. Compaction happens within a turn without a
         terminal signal, so it coexists with the streaming view; hidden when
         compaction_end arrives. The summary is intentionally not rendered. -->
    {#if runState.isCompacting}
      <div
        class="flex items-center gap-2 px-3 py-2 text-xs text-base-content/60"
      >
        <div
          class="h-3 w-3 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
        ></div>
        <span>{t("agent.timeline.compacting")}</span>
      </div>
    {/if}

    <!-- Run-level errors are visible, never a silent stop. -->
    {#if runState.error}
      <div
        class="px-3 py-2 rounded-md bg-error/10 text-error text-sm whitespace-pre-wrap break-words"
      >
        {runState.error}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Classic (space-taking) scrollbars would otherwise eat width on the right
     only, shifting the centred column off the composer's axis; a stable gutter
     on both edges keeps the two aligned. No-op under overlay scrollbars. */
  .scroll-column {
    scrollbar-gutter: stable both-edges;
  }

  .overflow-y-auto::-webkit-scrollbar {
    width: 6px;
  }

  .overflow-y-auto::-webkit-scrollbar-track {
    background: transparent;
  }

  .overflow-y-auto::-webkit-scrollbar-thumb {
    background: color-mix(in oklch, var(--base-content) 15%, transparent);
    border-radius: 3px;
  }

  .overflow-y-auto::-webkit-scrollbar-thumb:hover {
    background: color-mix(in oklch, var(--base-content) 25%, transparent);
  }
</style>
