<script lang="ts">
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";

  interface Props {
    /** Short label; the tooltip is single-line by design. */
    content: string;
    /** The trigger — supply the interactive element itself, never a bare label. */
    children: Snippet;
  }

  let { content, children }: Props = $props();

  let open = $state(false);
  let anchor: HTMLSpanElement | undefined = $state();
  let position = $state("");

  // Fixed positioning measured at open time: triggers usually sit inside a
  // scrolling container, which would clip an absolutely positioned tooltip.
  function show() {
    if (!anchor) {
      return;
    }
    const rect = anchor.getBoundingClientRect();
    position = `top: ${rect.top - 6}px; left: ${rect.left + rect.width / 2}px;`;
    open = true;
  }

  function hide() {
    open = false;
  }

  // Listeners are bound imperatively: the wrapper is a positioning anchor with
  // no role of its own — the semantics (and keyboard access) belong to the
  // trigger the caller renders inside it.
  $effect(() => {
    const el = anchor;
    if (!el) {
      return;
    }
    el.addEventListener("mouseenter", show);
    el.addEventListener("mouseleave", hide);
    el.addEventListener("focusin", show);
    el.addEventListener("focusout", hide);
    return () => {
      el.removeEventListener("mouseenter", show);
      el.removeEventListener("mouseleave", hide);
      el.removeEventListener("focusin", show);
      el.removeEventListener("focusout", hide);
    };
  });
</script>

<span bind:this={anchor} class="relative inline-flex">
  {@render children()}
</span>

{#if open}
  <div
    role="tooltip"
    style={position}
    class="fixed z-[var(--z-popover)] -translate-x-1/2 -translate-y-full whitespace-nowrap rounded-md border border-[var(--hairline)] bg-[var(--bg-card)] px-2 py-1 text-xs text-base-content/80 shadow-lg pointer-events-none"
    transition:fly={{ y: 4, duration: 120 }}
  >
    {content}
  </div>
{/if}
