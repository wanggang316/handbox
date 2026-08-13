<script module lang="ts">
  /**
   * Where each session was left, keyed by session id. Module-level so it
   * outlives the component: entering settings tears the timeline down, and the
   * reader expects to come back to the spot they were reading.
   *
   * A reader parked at the end is remembered as "bottom" rather than as pixels,
   * so a session that grew while they were away still opens at its newest turn.
   */
  type ScrollAnchor = "bottom" | { index: number; offset: number };
  const scrollMemory = new Map<string, ScrollAnchor>();
</script>

<script lang="ts">
  import { tick, untrack } from "svelte";
  import { ChartNoAxesColumn, Check, Copy } from "@lucide/svelte";
  import {
    renderMarkdown,
    markdownInteractions,
    copyToClipboard,
  } from "$lib/utils";
  import { t } from "$lib/i18n";
  import Button from "$lib/components/ui/Button.svelte";
  import Tooltip from "$lib/components/ui/Tooltip.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import type { HookRuleNotification } from "$lib/types";
  import type {
    AgentMessage,
    ToolResultContent,
  } from "$lib/types/agentSession";
  import { Anchor, ChevronDown } from "@lucide/svelte";
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

  // Hook-rule firings render inline where they happened. Firings that carry a
  // callId attach to that tool card (before/approval-hooks above it,
  // after-hooks below); the rest (prompt and turn-end rules) anchor by message
  // index: entries anchored to index i appear right after that message
  // (-1 = before the first).
  function hookNoticesAfter(anchor: number) {
    return runState.hookNotices.filter(
      (entry) => entry.notice.callId === null && entry.anchor === anchor,
    );
  }

  function hookNoticesForCall(
    callId: string,
    events: HookRuleNotification["event"][],
  ) {
    return runState.hookNotices.filter(
      (entry) =>
        entry.notice.callId === callId &&
        events.includes(entry.notice.event),
    );
  }

  // Labels mirror the settings page so a hook reads the same in both places.
  function hookEventLabel(event: HookRuleNotification["event"]): string {
    switch (event) {
      case "before_tool_call":
        return t("settings.hooks.event.before");
      case "after_tool_call":
        return t("settings.hooks.event.after");
      case "user_prompt_submit":
        return t("settings.hooks.event.prompt");
      case "turn_end":
        return t("settings.hooks.event.turnEnd");
      case "approval_requested":
        return t("settings.hooks.event.approval");
      default:
        return event;
    }
  }

  function hookActionLabel(action: HookRuleNotification["action"]): string {
    switch (action) {
      case "notify":
        return t("settings.hooks.action.notify");
      case "run_command":
        return t("settings.hooks.action.runCommand");
      default:
        return action;
    }
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

  // Progressive mount. Entering the session view (cold start, session switch,
  // returning from settings) rebuilds the whole transcript at once — hundreds
  // of rows, tool cards and thinking blocks in a single task, which locks the
  // window for as long as it takes. Only the tail is mounted up front; the rest
  // fills in one idle chunk at a time, so the first paint costs a viewport.
  const INITIAL_WINDOW = 20;
  const FILL_CHUNK = 40;
  // Slack for "parked at the end": sub-pixel scroll offsets and a growing last
  // turn should still read as being at the bottom.
  const BOTTOM_EPSILON = 24;

  // Where the first render should start: the tail, or the remembered row when
  // the reader left mid-transcript (that row and everything under it must exist
  // before the position means anything). Pure — no rune reads — so the fill loop
  // can call it without making its caller's effect depend on `windowStart`.
  function windowStartFor(id: string, total: number): number {
    const tailStart = Math.max(0, total - INITIAL_WINDOW);
    const anchor = scrollMemory.get(id);
    if (!anchor || anchor === "bottom") return tailStart;
    return Math.min(anchor.index, tailStart);
  }

  // First rendered message index; 0 once the whole transcript is mounted.
  // Seeded at init, not from an effect: effects run after the first render, so
  // starting at 0 would build the entire transcript and immediately throw it
  // away — exactly the cost this avoids.
  let windowStart = $state(
    untrack(() => windowStartFor(sessionId, runState.messages.length)),
  );

  // Whether the reader is parked at the end. Plain `let`, not `$state`: it
  // gates imperative scrolling and must not invalidate anything when it flips.
  let stickToBottom = true;

  // True while openWindow is positioning the view. Programmatic scrolling fires
  // the same scroll events the reader's own does, and treating those as intent
  // would overwrite the very position being restored.
  let opening = false;

  // Real input during the open sequence: the reader has taken over, so stop
  // re-asserting the anchor under them.
  let readerMoved = false;

  function noteReaderInput() {
    readerMoved = true;
  }

  // Auto-scroll (new turn, streaming text, hook notice) applies only to a
  // reader who is already at the end — otherwise reading history mid-run, or a
  // just-restored position, gets yanked to the bottom.
  function pinToBottom() {
    if (stickToBottom) {
      scrollToBottom();
    }
  }

  // The reader's position as a message row plus its offset from the container's
  // top edge, which survives the older messages mounting in later — a pixel
  // scrollTop would not.
  function readAnchor(): ScrollAnchor {
    const el = messagesContainer;
    if (!el) return "bottom";
    if (el.scrollHeight - el.scrollTop - el.clientHeight <= BOTTOM_EPSILON) {
      return "bottom";
    }
    const top = el.getBoundingClientRect().top;
    for (const row of el.querySelectorAll<HTMLElement>("[data-message-index]")) {
      const box = row.getBoundingClientRect();
      // First row still visible: the one straddling the top edge, so its offset
      // is negative and restoring reproduces the same partial row.
      if (box.bottom > top) {
        return {
          index: Number(row.dataset.messageIndex),
          offset: box.top - top,
        };
      }
    }
    return "bottom";
  }

  function restoreAnchor(anchor: ScrollAnchor) {
    const el = messagesContainer;
    if (!el) return;
    if (anchor === "bottom") {
      scrollToBottom();
      return;
    }
    const row = el.querySelector<HTMLElement>(
      `[data-message-index="${anchor.index}"]`,
    );
    if (!row) {
      // The remembered row is gone (transcript compacted, session cleared).
      scrollToBottom();
      return;
    }
    el.scrollTop +=
      row.getBoundingClientRect().top -
      el.getBoundingClientRect().top -
      anchor.offset;
  }

  let scrollFrame = 0;

  // rAF-throttled: scroll fires far more often than the memory needs updating,
  // and reading layout on every event would trade one jank for another.
  function handleScroll() {
    if (opening || scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      const anchor = readAnchor();
      stickToBottom = anchor === "bottom";
      scrollMemory.set(sessionId, anchor);
    });
  }

  $effect(() => () => cancelAnimationFrame(scrollFrame));

  const visibleMessages = $derived(
    windowStart > 0 ? runState.messages.slice(windowStart) : runState.messages,
  );

  function idle(): Promise<void> {
    return new Promise((resolve) => {
      if (typeof window !== "undefined" && window.requestIdleCallback) {
        // timeout: never let a busy main thread stall the fill indefinitely.
        window.requestIdleCallback(() => resolve(), { timeout: 200 });
      } else {
        setTimeout(resolve, 16);
      }
    });
  }

  function raf(): Promise<void> {
    return new Promise((resolve) => requestAnimationFrame(() => resolve()));
  }

  /**
   * Open the session at `anchor`, then widen the window upwards a chunk at a
   * time until the whole transcript is mounted.
   *
   * `from` is passed in rather than read off `windowStart`: this function's
   * first statements run synchronously inside the caller's effect, and reading
   * the rune there would make that effect depend on the state this loop writes
   * — a reset/refill cycle that never lands on 0.
   *
   * The anchor is re-asserted after every mutation instead of preserving a
   * pixel offset. Each chunk prepends content above the reader, and a one-shot
   * placement cannot survive that, nor the late layout (images, KaTeX, code
   * blocks) that keeps changing heights for a few frames afterwards.
   */
  async function openWindow(
    cancelled: () => boolean,
    from: number,
    anchor: ScrollAnchor,
  ) {
    const stop = () => cancelled() || readerMoved || !messagesContainer;

    opening = true;
    readerMoved = false;
    try {
      let start = from;
      await tick();
      if (stop()) return;
      restoreAnchor(anchor);

      while (!stop() && start > 0) {
        await idle();
        if (stop()) return;
        start = Math.max(0, start - FILL_CHUNK);
        windowStart = start;
        await tick();
        if (stop()) return;
        restoreAnchor(anchor);
      }

      for (let frame = 0; frame < 3; frame += 1) {
        await raf();
        if (stop()) return;
        restoreAnchor(anchor);
      }
    } finally {
      opening = false;
    }
  }

  // Re-window on session switch and when a restore lands the transcript in one
  // shot (`hydrated`). Message count is read untracked: streaming appends must
  // not reset the window mid-run.
  $effect(() => {
    const id = sessionId;
    void runState.hydrated;

    const total = untrack(() => runState.messages.length);
    const anchor = scrollMemory.get(id) ?? "bottom";
    stickToBottom = anchor === "bottom";

    const start = windowStartFor(id, total);
    windowStart = start;

    let stopped = false;
    void openWindow(() => stopped, start, anchor);
    return () => {
      stopped = true;
    };
  });

  // A committed turn keeps the newest content in view; the delay re-pins after
  // late layout (markdown / images growing the content). Session switches and
  // restores are positioned by openWindow instead.
  $effect(() => {
    if (runState.messages.length > 0) {
      pinToBottom();
      setTimeout(pinToBottom, 100);
    }
  });

  // Scroll as streaming text/thinking grows.
  $effect(() => {
    if (runState.streamingText || runState.thinkingText) {
      setTimeout(pinToBottom, 50);
    }
  });

  // A hook notice appended at the bottom must not land below the fold.
  $effect(() => {
    if (runState.hookNotices.length > 0) {
      setTimeout(pinToBottom, 50);
    }
  });
</script>

{#snippet hookIdentity(notice: HookRuleNotification)}
  <Anchor size={12} class="shrink-0" />
  <span class="shrink-0 text-base-content/50">Hooks</span>
  <span class="shrink-0 font-medium">{notice.ruleName}</span>
  <span
    class="shrink-0 rounded px-1.5 py-0.5 text-[10px] bg-base-content/10 text-base-content/70"
  >
    {hookEventLabel(notice.event)} · {hookActionLabel(notice.action)}
  </span>
  {#if notice.message}
    <span class="truncate text-base-content/50">{notice.message}</span>
  {/if}
{/snippet}

{#snippet hookNoticeRow(notice: HookRuleNotification)}
  {@const tone =
    notice.outcome === "denied" || notice.outcome === "failed"
      ? "text-warning"
      : "text-base-content/70"}
  <!-- The row carries the hook's identity — name, kind, message. A command
       firing keeps its execution capture behind the native disclosure; a row
       with nothing more to show stays a plain line. -->
  {#if notice.detail}
    <details class="hook-notice group px-3 py-1.5">
      <summary
        class="flex cursor-pointer list-none items-center gap-2 text-xs {tone}"
      >
        {@render hookIdentity(notice)}
        <ChevronDown
          size={12}
          class="shrink-0 opacity-60 transition-transform group-open:rotate-180"
        />
      </summary>
      <pre
        class="mt-1.5 ml-5 max-h-64 overflow-y-auto rounded-md bg-base-200 px-3 py-2 text-xs leading-relaxed whitespace-pre-wrap break-words text-base-content/70">{notice.detail}</pre>
    </details>
  {:else}
    <div class="flex items-center gap-2 px-3 py-1.5 text-xs {tone}">
      {@render hookIdentity(notice)}
    </div>
  {/if}
{/snippet}

<!-- The message stream is content: bubbles, markdown replies, tool-card bodies
     and error text must all be selectable (buttons stay unselectable globally). -->
<div
  bind:this={messagesContainer}
  class="flex-1 overflow-y-auto select-text scroll-column"
  onscroll={handleScroll}
  onwheel={noteReaderInput}
  ontouchmove={noteReaderInput}
>
  <div class="chat-column py-4 space-y-6">
    <!-- Hook firings that preceded every message (a prompt rule on the first turn). -->
    {#each hookNoticesAfter(-1) as entry}
      {@render hookNoticeRow(entry.notice)}
    {/each}

    <!-- Committed messages, from `windowStart` to the end (see the progressive
         mount above). messages is append-only (the reducer only appends or
         finalizes in place, never reorders), so absolute index keys reuse DOM
         safely — including when the window widens and prepends older rows;
         cards key by toolCallId so their state never shifts with the index. -->
    {#each visibleMessages as message, offset (windowStart + offset)}
      {@const i = windowStart + offset}
      {#if message.role === "user"}
        <!-- data-message-index anchors the reader's position across a teardown
             (see scrollMemory): the row, not a pixel offset, is what survives
             older messages mounting in later. -->
        <div class="flex justify-end" data-message-index={i}>
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
        <div class="flex flex-col gap-2" data-message-index={i}>
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
                  <!-- Before- and approval-hooks fire before their call runs,
                       so they read above the card; an after-hook reads below. -->
                  {#each hookNoticesForCall(block.id, [
                    "before_tool_call",
                    "approval_requested",
                  ]) as entry}
                    {@render hookNoticeRow(entry.notice)}
                  {/each}
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
                  {#each hookNoticesForCall(block.id, ["after_tool_call"]) as entry}
                    {@render hookNoticeRow(entry.notice)}
                  {/each}
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
                    <Button
                      variant="clear"
                      size="icon-sm"
                      class="text-base-content/40 enabled:hover:text-base-content"
                      ariaLabel={t("agent.timeline.copy")}
                      onclick={() => copyMessage(i, assistantText(message))}
                    >
                      {#if copiedIndex === i}
                        <Check size={14} />
                      {:else}
                        <Copy size={14} />
                      {/if}
                    </Button>
                  </Tooltip>
                {/if}

                {#if hasUsage(message)}
                  <Tooltip content={usageLabel(message)}>
                    <!-- Mirrors the icon-sm clear button: it is a hover target
                         like its neighbour, so it answers hover the same way.
                         It stays out of the tab order though — one stop per
                         message would bury the row's real control, and the
                         label already carries the numbers for AT. -->
                    <span
                      class="flex size-7 items-center justify-center rounded-md transition-[color,background-color] duration-[var(--dur-fast)] ease-[var(--ease-out)] hover:bg-base-300 hover:text-base-content"
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

      <!-- Hook firings anchored right after this message, in arrival order. -->
      {#each hookNoticesAfter(i) as entry}
        {@render hookNoticeRow(entry.notice)}
      {/each}
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
  /* WebKit draws its own disclosure marker on <summary>; the row supplies its
     own chevron instead. */
  .hook-notice summary::-webkit-details-marker {
    display: none;
  }

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
