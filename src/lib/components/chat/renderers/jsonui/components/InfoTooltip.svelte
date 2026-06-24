<script lang="ts">
  import type { BaseComponentProps } from "@json-render/svelte";
  import { HelpCircle } from "@lucide/svelte";
  import { fly } from "svelte/transition";

  // A help trigger: a HelpCircle icon that reveals `content` in a hover popover.
  // `content` renders through Svelte text binding only (never @html). The popover
  // is fixed-positioned at a high z-index so it escapes any clipping ancestor,
  // mirroring the source ui/InfoTooltip.
  interface InfoTooltipProps {
    content: string;
  }

  let { props }: BaseComponentProps<InfoTooltipProps> = $props();

  let isOpen = $state(false);
  let buttonElement: HTMLButtonElement | undefined = $state();
  let tooltipStyle = $state("");

  function updatePosition() {
    if (!buttonElement) return;
    const rect = buttonElement.getBoundingClientRect();
    const top = rect.top - 12;
    const left = rect.left + rect.width / 2;
    tooltipStyle = `top: ${top}px; left: ${left}px;`;
  }

  function show() {
    isOpen = true;
    updatePosition();
  }

  function hide() {
    isOpen = false;
  }
</script>

<div class="relative inline-block">
  <button
    bind:this={buttonElement}
    type="button"
    onmouseenter={show}
    onmouseleave={hide}
    class="text-base-content/40 transition-colors hover:text-base-content/70"
    aria-label="显示帮助信息"
  >
    <HelpCircle size={14} />
  </button>
</div>

{#if isOpen}
  <div
    role="tooltip"
    onmouseenter={show}
    onmouseleave={hide}
    style={tooltipStyle}
    class="fixed z-[9999] w-64 -translate-x-1/2 -translate-y-full rounded-lg border border-base-300 bg-base-100 p-3 shadow-lg"
    transition:fly={{ y: 8, duration: 200, opacity: 0 }}
  >
    <p
      class="text-xs leading-relaxed text-base-content/80 whitespace-pre-wrap break-words"
    >
      {props.content}
    </p>
  </div>
{/if}
