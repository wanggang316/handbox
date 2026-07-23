<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';
  import { iconButton, type IconButtonVariants } from './variants';
  import { cn } from './utils';

  interface Props {
    icon: typeof IconType;
    iconSize?: number;
    strokeWidth?: number;
    ariaLabel?: string;
    size?: string;
    rounded?: string;
    variant?: IconButtonVariants['variant'];
    disabled?: boolean;
    customClass?: string;
    onclick?: (event: MouseEvent) => void;
    elementRef?: (el: HTMLButtonElement | null) => void;
    title?: string;
  }

  let {
    icon,
    iconSize = 20,
    strokeWidth = 2,
    ariaLabel = '',
    size = 'w-7 h-7',
    rounded = 'rounded-md',
    variant = 'ghost',
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
  class={cn(iconButton({ variant }), size, rounded, customClass)}
  data-slot="icon-button"
  aria-label={ariaLabel}
  onclick={handleClick}
  title={title}
  {disabled}
  bind:this={buttonEl}
>
  {#if icon}
    {@const Icon = icon}
    <Icon size={iconSize} strokeWidth={strokeWidth} />
  {/if}
</button>
