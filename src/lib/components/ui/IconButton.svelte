<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';
  import { createEventDispatcher } from 'svelte';

  export let icon: typeof IconType;
  export let iconSize: number = 20;
  export let ariaLabel: string = '';

  // 样式默认：圆角方形、深色图标、透明背景，悬浮显示浅色底
  export let size: string = 'w-7 h-7';
  export let rounded: string = 'rounded-md';
  export let bgColor: string = 'bg-transparent';
  export let hoverColor: string = 'hover:bg-base-300';
  export let textColor: string = 'text-base-content';
  export let disabled: boolean = false;
  export let customClass: string = '';

  const dispatch = createEventDispatcher();

  function handleClick(event: MouseEvent) {
    if (!disabled) {
      dispatch('click', event);
    }
  }
</script>

<button
  class="{size} {bgColor} {hoverColor} {textColor} {rounded} flex items-center justify-center transition-colors {customClass}"
  class:opacity-80={disabled}
  class:cursor-not-allowed={disabled}
  aria-label={ariaLabel}
  on:click={handleClick}
  {disabled}
>
  {#if icon}
    <svelte:component this={icon} size={iconSize} />
  {/if}
</button>


