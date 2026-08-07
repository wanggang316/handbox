<script lang="ts">
  import { HelpCircle } from "@lucide/svelte";
  import { fade, fly } from "svelte/transition";
  import { t } from "$lib/i18n";

  interface Props {
    content: string;
    size?: number;
    /** Tailwind width class for the popover; longer content reads better wider. */
    width?: string;
  }

  let { content, size = 14, width = "w-64" }: Props = $props();

  let isOpen = $state(false);
  let buttonElement: HTMLButtonElement | undefined = $state();
  let tooltipElement: HTMLDivElement | undefined = $state();
  let tooltipStyle = $state("");
  let placement = $state<"above" | "below">("above");

  // The tooltip is `position: fixed` with viewport coordinates, but any
  // transformed ancestor (a modal panel centered via translate) would become
  // its containing block and shift those coordinates. Rendering it under
  // <body> keeps the viewport as the reference frame everywhere.
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }

  function show() {
    isOpen = true;
    if (buttonElement) {
      updatePosition();
    }
  }

  function hide() {
    isOpen = false;
  }

  function updatePosition() {
    if (!buttonElement) return;

    const rect = buttonElement.getBoundingClientRect();
    const margin = 8; // minimum distance from the viewport edges
    const gap = 12; // distance between the icon and the tooltip

    // On the first pass the tooltip is not rendered yet, so both sizes read 0
    // and the defaults (above, unclamped) apply; the effect below re-runs this
    // once the element exists and corrects before paint.
    const width = tooltipElement?.offsetWidth ?? 0;
    const height = tooltipElement?.offsetHeight ?? 0;

    const centerX = rect.left + rect.width / 2;
    const left = width
      ? Math.min(
          Math.max(centerX, margin + width / 2),
          window.innerWidth - margin - width / 2,
        )
      : centerX;

    const next: "above" | "below" =
      height && rect.top - gap - height < margin ? "below" : "above";
    const top = next === "above" ? rect.top - gap : rect.bottom + gap;

    placement = next;
    tooltipStyle = `top: ${top}px; left: ${left}px;`;
  }

  $effect(() => {
    if (isOpen && tooltipElement) {
      updatePosition();
    }
  });
</script>

<div class="relative inline-block">
  <button
    bind:this={buttonElement}
    type="button"
    onmouseenter={show}
    onmouseleave={hide}
    class="text-base-content/40 hover:text-base-content/70 transition-colors"
    aria-label={t("ui.showHelp")}
  >
    <HelpCircle {size} />
  </button>
</div>

{#if isOpen}
  <div
    role="tooltip"
    use:portal
    bind:this={tooltipElement}
    onmouseenter={show}
    onmouseleave={hide}
    style={tooltipStyle}
    class="fixed z-[var(--z-popover)] {width} bg-base-100 border border-base-300 rounded-lg shadow-lg p-3 transform -translate-x-1/2 {placement ===
    'above'
      ? '-translate-y-full'
      : ''}"
    transition:fly={{ y: 8, duration: 200, opacity: 0 }}
  >
    <p class="text-xs text-base-content/80 leading-relaxed whitespace-pre-wrap">
      {content}
    </p>
  </div>
{/if}
