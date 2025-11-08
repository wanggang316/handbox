<script lang="ts">
  import { HelpCircle } from "@lucide/svelte";

  interface Props {
    content: string;
    size?: number;
  }

  let { content, size = 14 }: Props = $props();

  let isOpen = $state(false);
  let buttonElement: HTMLButtonElement | undefined = $state();
  let tooltipStyle = $state("");

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
    const top = rect.top - 8; // 8px margin
    const left = rect.left + rect.width / 2; // 水平居中对齐到按钮中心

    tooltipStyle = `top: ${top}px; left: ${left}px;`;
  }
</script>

<div class="relative inline-block">
  <button
    bind:this={buttonElement}
    type="button"
    onmouseenter={show}
    onmouseleave={hide}
    class="text-base-content/40 hover:text-base-content/70 transition-colors"
    aria-label="显示帮助信息"
  >
    <HelpCircle {size} />
  </button>
</div>

{#if isOpen}
  <div
    role="tooltip"
    onmouseenter={show}
    onmouseleave={hide}
    style={tooltipStyle}
    class="fixed z-[9999] w-64 bg-base-100 border border-base-300 rounded-lg shadow-lg p-3 transform -translate-x-1/2 -translate-y-full"
  >
    <p class="text-xs text-base-content/80 leading-relaxed">
      {content}
    </p>
  </div>
{/if}
