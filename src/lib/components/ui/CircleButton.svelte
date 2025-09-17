<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';

  export let icon: typeof IconType;
  export let iconSize: number = 16;
  export let ariaLabel: string;
  export let bgColor: string = 'bg-black';
  export let hoverColor: string = 'hover:bg-black/80';
  export let textColor: string = 'text-white';
  export let size: string = 'w-10 h-10';
  export let rounded: string = 'rounded-full';
  export let disabled: boolean = false;
  export let customClass: string = '';

  export let onclick: ((event: MouseEvent) => void) | undefined = undefined;

  function handleClick(event: MouseEvent) {
    if (!disabled) {
      onclick?.(event);
    }
  }
</script>

<button
  class="{size} {bgColor} {hoverColor} {textColor} {rounded} flex items-center justify-center transition-colors {customClass}"
  class:opacity-50={disabled}
  class:cursor-not-allowed={disabled}
  aria-label={ariaLabel}
  on:click={handleClick}
  {disabled}
>
  {#if icon}
    {@const Icon = icon}
    <Icon size={iconSize} />
  {/if}
</button>
