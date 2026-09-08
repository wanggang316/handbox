<script lang="ts">
  /**
   * Sticky header for a settings detail page (a provider, a custom tool).
   *
   * It exists because the settings layout owns the scroller, so a header that
   * merely sat at the top of the page scrolled away with the content. `sticky`
   * pins it to the top of that scroller — which the layout already keeps clear
   * of the window's drag region — and the blurred background is what the page
   * scrolls under.
   *
   * `actions` stays reachable for the same reason: a Save button at the bottom
   * of a long form is a Save button you have to scroll back to.
   */
  import { ChevronLeft } from "@lucide/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    /** Accessible name for the back button. */
    backLabel: string;
    onBack: () => void;
    /** Right-aligned controls (save, delete …). */
    actions?: Snippet;
  }

  let { title, backLabel, onBack, actions }: Props = $props();
</script>

<header
  class="sticky top-0 z-20 flex items-center gap-3 bg-[color:var(--bg-canvas)]/85 px-6 pt-1.5 pr-8 pb-3 backdrop-blur-sm"
>
  <Button
    variant="secondary"
    size="icon"
    shape="pill"
    ariaLabel={backLabel}
    onclick={onBack}
  >
    <ChevronLeft size={20} />
  </Button>

  <h1 class="min-w-0 flex-1 truncate text-xl font-semibold text-base-content">
    {title}
  </h1>

  {#if actions}
    <div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
  {/if}
</header>
