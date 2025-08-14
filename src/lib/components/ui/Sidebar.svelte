<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';

  export let initialWidth = 347;
  export let minWidth = 240;
  export let maxWidth = 600;
  export let storageKey: string | null = null;
  export let containerClass: string = '';

  export let width = initialWidth;
  let dragging = false;
  let container: HTMLDivElement;
  const dispatch = createEventDispatcher();

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

  let originalUserSelect = '';
  let originalWebkitUserSelect = '';
  let originalCursor = '';

  function startDrag(event: PointerEvent) {
    event.preventDefault();
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    dragging = true;

    // 禁用全局文本选择并显示调整光标
    const bodyStyle = document.body.style as CSSStyleDeclaration;
    originalUserSelect = bodyStyle.userSelect;
    // @ts-ignore - webkit prefixed property for Safari
    originalWebkitUserSelect = (bodyStyle as any).webkitUserSelect;
    originalCursor = bodyStyle.cursor;
    bodyStyle.userSelect = 'none';
    // @ts-ignore
    (bodyStyle as any).webkitUserSelect = 'none';
    bodyStyle.cursor = 'col-resize';

    dispatch('resizeStart');
  }

  let rafId: number | null = null;
  let pendingWidth: number | null = null;

  function onDrag(event: PointerEvent) {
    if (!dragging) return;
    event.preventDefault();
    const rect = container.getBoundingClientRect();
    const next = event.clientX - rect.left;
    pendingWidth = clamp(next);
    if (rafId === null) {
      rafId = requestAnimationFrame(() => {
        if (pendingWidth !== null) {
          width = pendingWidth;
          dispatch('resizing', { width });
          pendingWidth = null;
        }
        rafId = null;
      });
    }
  }

  function endDrag(event: PointerEvent) {
    dragging = false;
    event.preventDefault();
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    if (storageKey) {
      localStorage.setItem(storageKey, String(width));
    }
    // 恢复全局文本选择和光标
    const bodyStyle = document.body.style as CSSStyleDeclaration;
    bodyStyle.userSelect = originalUserSelect;
    // @ts-ignore
    (bodyStyle as any).webkitUserSelect = originalWebkitUserSelect;
    bodyStyle.cursor = originalCursor;
    dispatch('resizeEnd', { width });
  }

  function resetWidth() {
    width = clamp(initialWidth);
    if (storageKey) localStorage.setItem(storageKey, String(width));
  }
</script>

<div bind:this={container} class={`relative flex-shrink-0 ${containerClass}`} style={`width:${width}px; height:100%`}>
  <slot />
  <div
    class="absolute right-0 top-0 h-full w-1.5 cursor-col-resize bg-transparent hover:bg-gray-200 active:bg-gray-300 z-[10001]"
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



