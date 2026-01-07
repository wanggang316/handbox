<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';

  interface Props {
    icon: typeof IconType;
    iconSize?: number;
    ariaLabel?: string;
    size?: string;
    rounded?: string;
    bgColor?: string;
    hoverColor?: string;
    textColor?: string;
    disabled?: boolean;
    customClass?: string;
    onclick?: (event: MouseEvent) => void;
    elementRef?: (el: HTMLButtonElement | null) => void;
    title?: string;
  }

  let {
    icon,
    iconSize = 20,
    ariaLabel = '',
    size = 'w-7 h-7',
    rounded = 'rounded-md',
    bgColor = 'bg-transparent',
    hoverColor = 'hover:bg-base-300',
    textColor = 'text-base-content',
    disabled = false,
    customClass = '',
    onclick,
    elementRef,
    title = '',
  }: Props = $props();
  let buttonEl: HTMLButtonElement | null = null;
  $effect(() => {
    elementRef?.(buttonEl);
  });

  function handleClick(event: MouseEvent) {
    if (!disabled) {
      onclick?.(event);
    }
  }
</script>

<button
  class="{size} {bgColor} {hoverColor} {textColor} {rounded} flex items-center justify-center transition-colors {customClass}"
  class:opacity-80={disabled}
  class:cursor-not-allowed={disabled}
  aria-label={ariaLabel}
  onclick={handleClick}
  title={title}
  {disabled}
  bind:this={buttonEl}
>
  {#if icon}
    <svelte:component this={icon} size={iconSize} />
  {/if}
</button>
