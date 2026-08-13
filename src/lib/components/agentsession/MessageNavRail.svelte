<script lang="ts" module>
  export interface MessageNavItem {
    /** Index into runState.messages — the caller's scroll target. */
    index: number;
    question: string;
    answer: string;
  }
</script>

<script lang="ts">
  // Left-edge navigation rail for the transcript: one tick per user question.
  // Hovering a tick previews the question (plus the start of its reply);
  // clicking pins that question to the top of the viewport.
  //
  // The rail is an overlay, not a column: it floats in the free space beside
  // the centred chat column, so the transcript's own geometry is untouched.
  // The caller hides it when that free space is too narrow.
  import { t } from "$lib/i18n";

  interface Props {
    items: MessageNavItem[];
    /** Index of the question currently at the top of the viewport. */
    activeIndex: number;
    onSelect: (index: number) => void;
  }

  let { items, activeIndex, onSelect }: Props = $props();

  let railEl: HTMLElement;
  let listEl: HTMLDivElement;

  // Hovered/focused tick: its item plus the vertical offset the preview card
  // centres on (measured, so it survives the list's own scrolling).
  let previewItem = $state<MessageNavItem | null>(null);
  let previewTop = $state(0);

  function openPreview(item: MessageNavItem, tick: HTMLElement) {
    const railTop = railEl.getBoundingClientRect().top;
    const rect = tick.getBoundingClientRect();
    previewTop = rect.top + rect.height / 2 - railTop;
    previewItem = item;
  }

  function closePreview() {
    previewItem = null;
  }

  // Keep the active tick visible once the list overflows and scrolls.
  $effect(() => {
    const index = activeIndex;
    listEl
      ?.querySelector(`[data-nav-tick="${index}"]`)
      ?.scrollIntoView({ block: "nearest" });
  });

  /** One-line question text: newlines would break the truncated title row. */
  function questionLine(text: string): string {
    return text.replace(/\s+/g, " ").trim();
  }

  /**
   * Markdown reduced to prose for the preview body. Not a parser — it only
   * drops the syntax that reads as noise at this size (fences, list bullets,
   * emphasis runs, link/image URLs).
   */
  function answerLines(text: string): string {
    return text
      .replace(/```[\s\S]*?```/g, " ")
      .replace(/`([^`]*)`/g, "$1")
      .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
      .replace(/\[([^\]]*)\]\(([^)]*)\)/g, "$1")
      .replace(/^\s{0,3}#{1,6}\s+/gm, "")
      .replace(/^\s{0,3}([-*_])(?:\s*\1){2,}\s*$/gm, " ")
      .replace(/^\s{0,3}>\s?/gm, "")
      .replace(/^\s{0,3}(?:[-*+]|\d+\.)\s+/gm, "")
      .replace(/(\*\*|__|~~|\*|_)/g, "")
      .replace(/\s+/g, " ")
      .trim();
  }
</script>

<nav
  bind:this={railEl}
  aria-label={t("agent.nav.label")}
  class="absolute top-1/2 left-4 z-20 -translate-y-1/2"
>
  <div
    bind:this={listEl}
    class="nav-ticks flex max-h-[52vh] flex-col items-start overflow-y-auto py-1"
  >
    {#each items as item, position (item.index)}
      {@const active = item.index === activeIndex}
      <button
        type="button"
        data-nav-tick={item.index}
        class="group flex h-3 w-8 shrink-0 items-center"
        aria-label={t("agent.nav.jumpTo", { index: position + 1 })}
        aria-current={active ? "true" : undefined}
        onmouseenter={(event) => openPreview(item, event.currentTarget)}
        onfocus={(event) => openPreview(item, event.currentTarget)}
        onmouseleave={closePreview}
        onblur={closePreview}
        onclick={() => onSelect(item.index)}
      >
        <!-- Every tick is the same length; position in the conversation is
             carried by colour alone, so the rail stays a straight edge. -->
        <span
          class="h-[2px] w-5 rounded-full transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)] {active
            ? 'bg-base-content/75'
            : 'bg-base-content/20 group-hover:bg-base-content/45'}"
        ></span>
      </button>
    {/each}
  </div>

  {#if previewItem}
    <!-- Pointer-transparent: the card overlays the transcript, and crossing
         onto it must not count as leaving the tick. -->
    <div
      class="pointer-events-none absolute left-full ml-2 w-[300px] -translate-y-1/2 rounded-xl border border-[var(--hairline)] bg-base-100 px-3 py-2.5 shadow-lg"
      style="top: {previewTop}px"
    >
      <p class="truncate text-[13px] font-medium text-base-content">
        {questionLine(previewItem.question)}
      </p>
      <p
        class="mt-1.5 line-clamp-3 text-[12px] leading-[1.55] text-base-content/50"
      >
        {previewItem.answer
          ? answerLines(previewItem.answer)
          : t("agent.nav.noAnswer")}
      </p>
    </div>
  {/if}
</nav>

<style>
  /* The rail is chrome: it scrolls when the conversation is long, but a
     scrollbar next to 16px ticks would be louder than the ticks themselves. */
  .nav-ticks {
    scrollbar-width: none;
  }

  .nav-ticks::-webkit-scrollbar {
    display: none;
  }
</style>
