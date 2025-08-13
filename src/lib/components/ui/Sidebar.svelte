<script lang="ts">
  import { onMount } from 'svelte';

  export let initialWidth = 347;
  export let minWidth = 240;
  export let maxWidth = 600;
  export let storageKey: string | null = null;
  export let containerClass: string = '';

  let width = initialWidth;
  let dragging = false;
  let container: HTMLDivElement;

  onMount(() => {
    if (storageKey) {
      const saved = localStorage.getItem(storageKey);
      if (saved) {
        const parsed = Number(saved);
        if (!Number.isNaN(parsed)) {
          width = clamp(parsed);
        }
      }
    }
  });

  function clamp(value: number): number {
    return Math.min(Math.max(value, minWidth), maxWidth);
  }

  function startDrag(event: PointerEvent) {
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    dragging = true;
  }

  function onDrag(event: PointerEvent) {
    if (!dragging) return;
    const rect = container.getBoundingClientRect();
    const next = event.clientX - rect.left;
    width = clamp(next);
  }

  function endDrag(event: PointerEvent) {
    dragging = false;
    if (storageKey) {
      localStorage.setItem(storageKey, String(width));
    }
  }

  function resetWidth() {
    width = clamp(initialWidth);
    if (storageKey) localStorage.setItem(storageKey, String(width));
  }
</script>

<div bind:this={container} class={`relative flex-shrink-0 ${containerClass}`} style={`width:${width}px`}>
  <slot />
  <div
    class="absolute right-0 top-0 h-full w-1.5 cursor-col-resize bg-transparent hover:bg-gray-200 active:bg-gray-300"
    role="separator"
    aria-orientation="vertical"
    aria-valuenow={width}
    aria-valuemin={minWidth}
    aria-valuemax={maxWidth}
    on:pointerdown={startDrag}
    on:pointermove={onDrag}
    on:pointerup={endDrag}
    on:pointercancel={endDrag}
    on:dblclick={resetWidth}
    aria-label="调整侧栏宽度"
  ></div>
</div>



