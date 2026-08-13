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
  import MessageNavRail, { type MessageNavItem } from "./MessageNavRail.svelte";
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

  // ── Scroll orchestration ─────────────────────────────────────────────────
  //
  // Two modes. `stick` follows the bottom so a running turn stays in view —
  // the default, and what restoring a session lands in. Pinning puts one
  // question at the top of the viewport and leaves it there: sending enters it
  // (the question you just asked heads the screen while the answer grows under
  // it) and so does clicking the nav rail.
  //
  // Pinning the *last* question needs slack below it — otherwise the scroll
  // range ends before the question reaches the top. `spacer` supplies exactly
  // that much, and shrinks as the answer fills it, so the pinned question does
  // not drift while the reply streams.

  /** Gap left above a pinned question. Matches the column's top padding. */
  const TOP_PAD = 16;
  /** Distance from the bottom that still counts as "at the bottom". */
  const BOTTOM_EPS = 24;
  /** Free space beside the chat column needed before the rail is shown. */
  const RAIL_MIN_WIDTH = 960;

  // Reactive refs: the ResizeObserver is wired in an effect that must re-run
  // once the bindings land, never silently no-op on an unbound element.
  let scrollEl = $state<HTMLDivElement>();
  let contentEl = $state<HTMLDivElement>();

  let spacer = $state(0);
  let scrollWidth = $state(0);
  let activeNavIndex = $state(-1);

  // Scroll policy is imperative bookkeeping, deliberately outside $state: it is
  // read by handlers, never by the template, and reactivity here would only
  // feed effects back into themselves.
  let stick = true;
  let pinnedIndex: number | null = null;
  // Set by real input events only, so a programmatic scroll can never be
  // mistaken for the user asking to follow the bottom again.
  let userIntent = false;

  // Question anchors by message index, registered by the rendered bubbles.
  const anchorEls = new Map<number, HTMLElement>();

  function anchor(node: HTMLElement, index: number) {
    anchorEls.set(index, node);
    return {
      update(next: number) {
        anchorEls.delete(index);
        index = next;
        anchorEls.set(index, node);
      },
      destroy() {
        anchorEls.delete(index);
      },
    };
  }

  /** Offset of a question from the top of the scrollable content. */
  function anchorOffset(index: number): number | null {
    const el = anchorEls.get(index);
    if (!el || !contentEl) {
      return null;
    }
    return el.getBoundingClientRect().top - contentEl.getBoundingClientRect().top;
  }

  const lastUserIndex = $derived.by(() => {
    for (let i = runState.messages.length - 1; i >= 0; i -= 1) {
      if (runState.messages[i].role === "user") {
        return i;
      }
    }
    return -1;
  });

  function scrollToBottom() {
    if (scrollEl) {
      scrollEl.scrollTop = scrollEl.scrollHeight;
    }
  }

  // Slack below the last question, needed only while it is the pinned one.
  // `spacer` lives outside the measured column, so the natural content height
  // is read directly and the two never chase each other.
  function recomputeSpacer() {
    if (!scrollEl || !contentEl) {
      return;
    }
    let next = 0;
    if (pinnedIndex !== null && pinnedIndex === lastUserIndex) {
      const top = anchorOffset(pinnedIndex);
      if (top !== null) {
        const natural = contentEl.getBoundingClientRect().height;
        next = Math.max(
          0,
          Math.round(top - TOP_PAD + scrollEl.clientHeight - natural),
        );
      }
      // The slack is spent: the running turn has filled the screen below its
      // question. Holding the pin from here on would freeze the view while
      // tool cards keep landing off-screen, so hand back to bottom-following.
      // Only the turn in flight does this — a pin aimed at an earlier question
      // is a deliberate jump and must survive whatever the run appends.
      if (next === 0 && runState.isRunning) {
        pinnedIndex = null;
        stick = true;
      }
    }
    spacer = next;
  }

  async function pinTo(index: number) {
    pinnedIndex = index;
    stick = false;
    userIntent = false;
    // Two flushes, both load-bearing: the question's own bubble must exist
    // before it can be measured, and the slack that measurement produces must
    // be in the DOM before the scroll — otherwise the target is out of range
    // and the browser clamps it back.
    await tick();
    recomputeSpacer();
    await tick();
    const top = anchorOffset(index);
    if (top === null || !scrollEl) {
      return;
    }
    scrollEl.scrollTo({ top: Math.max(0, top - TOP_PAD), behavior: "smooth" });
  }

  // The rail highlights the last question that has passed the top edge.
  let activeRaf = 0;
  function scheduleActiveUpdate() {
    if (activeRaf) {
      return;
    }
    activeRaf = requestAnimationFrame(() => {
      activeRaf = 0;
      if (!scrollEl || !contentEl) {
        return;
      }
      const contentTop = contentEl.getBoundingClientRect().top;
      const threshold = scrollEl.scrollTop + TOP_PAD + 8;
      let active = -1;
      let first = -1;
      for (const [index, el] of anchorEls) {
        if (first < 0 || index < first) {
          first = index;
        }
        if (el.getBoundingClientRect().top - contentTop <= threshold) {
          active = Math.max(active, index);
        }
      }
      // Above the first question the rail still points at it, never at nothing.
      activeNavIndex = active < 0 ? first : active;
    });
  }

  function markUserIntent() {
    userIntent = true;
  }

  function handleScroll() {
    if (userIntent && scrollEl) {
      const atBottom =
        scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight <
        BOTTOM_EPS;
      stick = atBottom;
      // Reaching the bottom means "follow the conversation" — release the pin
      // so its leftover slack collapses instead of sitting there as dead space.
      if (atBottom && pinnedIndex !== null) {
        pinnedIndex = null;
        recomputeSpacer();
      }
    }
    scheduleActiveUpdate();
  }

  // Content growth (streaming text, tool cards, late markdown/image layout) is
  // observed rather than guessed at with timers: re-pin the bottom when
  // following, and keep the spacer sized to whatever is left of the slack.
  $effect(() => {
    const scroller = scrollEl;
    if (!scroller || !contentEl) {
      return;
    }
    const observer = new ResizeObserver(() => {
      scrollWidth = scroller.clientWidth;
      recomputeSpacer();
      if (stick) {
        scrollToBottom();
      }
      scheduleActiveUpdate();
    });
    observer.observe(contentEl);
    observer.observe(scroller);
    return () => observer.disconnect();
  });

  $effect(() => () => cancelAnimationFrame(activeRaf));

  // Session switch, transcript restore, and new messages each land in a
  // different mode, so the policy is applied from the message sequence itself.
  let seenSession = "";
  let seenCount = 0;
  let seenHydrated = false;

  function applyScrollPolicy(
    id: string,
    messages: AgentMessage[],
    hydrated: boolean,
  ) {
    // Switching sessions reuses this component instance, which still holds the
    // previous conversation's scroll position: land at the bottom, synchronously.
    if (id !== seenSession) {
      seenSession = id;
      seenCount = messages.length;
      seenHydrated = hydrated;
      pinnedIndex = null;
      stick = true;
      userIntent = false;
      spacer = 0;
      // Mounting into a turn already in flight (the first send of a session
      // mounts this component, and sessions can be switched mid-run): the
      // question that turn is answering belongs at the top, same as if the
      // send had happened here.
      const last = messages.length - 1;
      if (last >= 0 && messages[last].role === "user" && runState.isRunning) {
        void pinTo(last);
      } else {
        scrollToBottom();
      }
      scheduleActiveUpdate();
      return;
    }
    // Restore replaces the whole sequence at once; that is history arriving,
    // not a turn being taken, so it must not pin its last question.
    if (hydrated && !seenHydrated) {
      seenHydrated = true;
      seenCount = messages.length;
      pinnedIndex = null;
      stick = true;
      spacer = 0;
      scrollToBottom();
      scheduleActiveUpdate();
      return;
    }
    if (messages.length === seenCount) {
      return;
    }
    const previousCount = seenCount;
    seenCount = messages.length;
    for (let i = messages.length - 1; i >= previousCount; i -= 1) {
      if (messages[i].role === "user") {
        void pinTo(i);
        scheduleActiveUpdate();
        return;
      }
    }
    if (stick) {
      scrollToBottom();
    }
    scheduleActiveUpdate();
  }

  $effect(() => {
    const messages = runState.messages;
    const hydrated = runState.hydrated;
    const id = sessionId;
    untrack(() => applyScrollPolicy(id, messages, hydrated));
  });

  // Rail entries: every question, paired with the reply that followed it.
  const navItems = $derived.by(() => {
    const items: MessageNavItem[] = [];
    const messages = runState.messages;
    for (let i = 0; i < messages.length; i += 1) {
      const message = messages[i];
      if (message.role !== "user") {
        continue;
      }
      let answer = "";
      for (let j = i + 1; j < messages.length && !answer; j += 1) {
        const next = messages[j];
        if (next.role === "user") {
          break;
        }
        if (next.role === "assistant") {
          answer = assistantText(next);
        }
      }
      items.push({ index: i, question: userText(message), answer });
    }
    return items;
  });

  // A single question is its own navigation; the rail earns its space from two.
  // Below the width threshold it would overlap the transcript instead of
  // floating beside it.
  const showNavRail = $derived(
    navItems.length > 1 && scrollWidth >= RAIL_MIN_WIDTH,
  );
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

<!-- The scroller sits in a positioned shell so the nav rail can float in the
     free space beside the centred column without taking layout from it. -->
<div class="relative flex min-h-0 flex-1 flex-col">
  <!-- The message stream is content: bubbles, markdown replies, tool-card bodies
       and error text must all be selectable (buttons stay unselectable globally). -->
  <div
    bind:this={scrollEl}
    class="flex-1 overflow-y-auto select-text scroll-column"
    onscroll={handleScroll}
    onwheel={markUserIntent}
    ontouchmove={markUserIntent}
    onpointerdown={markUserIntent}
  >
    <div bind:this={contentEl} class="chat-column py-4 space-y-6">
      <!-- Hook firings that preceded every message (a prompt rule on the first turn). -->
      {#each hookNoticesAfter(-1) as entry}
        {@render hookNoticeRow(entry.notice)}
      {/each}

      <!-- Committed messages. messages is append-only (the reducer only appends
           or finalizes in place, never reorders), so index keys reuse DOM safely;
           cards key by toolCallId so their state never shifts with the index. -->
      {#each runState.messages as message, i (i)}
        {#if message.role === "user"}
          <!-- Scroll anchor: the pin target on send, and the rail's jump target. -->
          <div class="flex justify-end" use:anchor={i}>
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

    <!-- Slack that lets the newest question reach the top of the viewport;
         zero unless a pin is holding it there. Outside the measured column so
         its height never feeds back into the measurement. -->
    <div style="height: {spacer}px" aria-hidden="true"></div>
  </div>

  {#if showNavRail}
    <MessageNavRail
      items={navItems}
      activeIndex={activeNavIndex}
      onSelect={(index) => void pinTo(index)}
    />
  {/if}
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
