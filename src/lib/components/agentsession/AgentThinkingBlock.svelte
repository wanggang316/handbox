<script lang="ts">
  import { ChevronDown } from "@lucide/svelte";
  import { renderMarkdown, markdownInteractions } from "$lib/utils";
  import { t } from "$lib/i18n";

  interface Props {
    // Streaming accumulated text, or the thinking block of a committed message.
    thinking: string;
    // Drives the label text and the shimmer; also disables the height clamp.
    isStreaming?: boolean;
  }

  let { thinking, isStreaming = false }: Props = $props();

  // Reasoning is a side channel: collapsed in both states, opened on demand.
  let expanded = $state(false);
  // Once open, long reasoning stays height-clamped until the reader asks for the rest.
  let showAll = $state(false);

  let contentEl = $state<HTMLDivElement>();
  let contentHeight = $state(0);

  // Matches .thinking-clamp's max-height; ~10 lines before the fade.
  const CLAMP_HEIGHT = 240;

  // While streaming, expanding is an explicit "show me the live reasoning", so
  // the clamp is off — otherwise incoming text would grow below the fold and
  // the reader would watch a frozen excerpt.
  const clamped = $derived(!showAll && !isStreaming);
  const overflowing = $derived(!isStreaming && contentHeight > CLAMP_HEIGHT);

  // The content box grows freely inside the clamped (overflow-hidden) wrapper,
  // so observing it keeps the measurement honest while markdown and images settle.
  $effect(() => {
    const el = contentEl;
    if (!el) {
      return;
    }
    const observer = new ResizeObserver(() => {
      contentHeight = el.offsetHeight;
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

  function toggle() {
    expanded = !expanded;
    if (!expanded) {
      showAll = false;
    }
  }

  // Hover tint stays a step lighter than the expanded colour: the pointer rests
  // on the row right after a click, so collapsing must still read as collapsed.
  const labelClass = $derived(
    expanded
      ? "text-base-content"
      : "text-base-content/50 group-hover:text-base-content/75",
  );

  // The chevron is the hover affordance; it points right while collapsed and
  // swings down as the body opens, so the rotation reads as the disclosure.
  const chevronClass = $derived(
    expanded
      ? "rotate-0 opacity-100"
      : "-rotate-90 opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100",
  );
</script>

<div class="mb-4">
  <!-- inline-flex, not flex: a block-level flex button would stretch across the
       whole message column, turning the empty space to its right into a hover
       and click target. -->
  <button
    class="group inline-flex items-center gap-1 my-2 text-left"
    onclick={toggle}
    aria-expanded={expanded}
  >
    <span
      class="text-sm font-medium transition-colors duration-150 {labelClass}"
      class:thinking-shimmer={isStreaming}
    >
      {isStreaming
        ? t("agent.thinkingBlock.streaming")
        : t("agent.thinkingBlock.title")}
    </span>
    <span
      class="flex text-base-content/50 transition-[opacity,transform] duration-150 {chevronClass}"
    >
      <ChevronDown size={14} />
    </span>
  </button>

  {#if expanded}
    <div class="mt-2 mb-6 border-l border-[var(--hairline)]">
      <div
        class="px-4"
        class:thinking-clamp={clamped}
        class:thinking-fade={clamped && overflowing}
      >
        <div
          bind:this={contentEl}
          class="text-sm text-base-content/80 break-words leading-relaxed markdown-content"
          use:markdownInteractions
        >
          {@html renderMarkdown(thinking)}
        </div>
      </div>

      {#if overflowing}
        <button
          class="mt-1 px-4 text-sm text-base-content/50 hover:text-base-content/80"
          onclick={() => (showAll = !showAll)}
        >
          {showAll
            ? t("agent.thinkingBlock.showLess")
            : t("agent.thinkingBlock.showMore")}
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .thinking-clamp {
    max-height: 240px;
    overflow: hidden;
  }

  /* Only when the text actually exceeds the clamp: the last lines dissolve so
     the cut reads as "there is more", not as a hard crop. */
  .thinking-fade {
    -webkit-mask-image: linear-gradient(
      to bottom,
      #000 0%,
      #000 70%,
      transparent 100%
    );
    mask-image: linear-gradient(to bottom, #000 0%, #000 70%, transparent 100%);
  }

  /* Live reasoning: a light sweeps across the glyphs themselves
     (background-clip: text), so the collapsed row signals progress on its own. */
  .thinking-shimmer {
    background-image: linear-gradient(
      100deg,
      color-mix(in oklch, var(--base-content) 40%, transparent) 0%,
      color-mix(in oklch, var(--base-content) 40%, transparent) 35%,
      var(--base-content) 50%,
      color-mix(in oklch, var(--base-content) 40%, transparent) 65%,
      color-mix(in oklch, var(--base-content) 40%, transparent) 100%
    );
    background-size: 250% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: thinking-shimmer 1.8s linear infinite;
  }

  @keyframes thinking-shimmer {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0% 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .thinking-shimmer {
      animation: none;
      background-image: none;
      color: color-mix(in oklch, var(--base-content) 60%, transparent);
    }
  }
</style>
