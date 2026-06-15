<script lang="ts">
  import type { Icon as IconType } from '@lucide/svelte';

  type Variant = 'ghost';

  type VariantClasses = {
    bg: string;
    hover: string;
    text: string;
  };

  // Authoritative variant → class map. `ghost` reproduces the exact former
  // default (transparent rest, base-300 hover, base-content text).
  const VARIANT_CLASSES: Record<Variant, VariantClasses> = {
    ghost: {
      bg: 'bg-transparent',
      hover: 'hover:bg-base-300',
      text: 'text-base-content',
    },
  };

  interface Props {
    icon: typeof IconType;
    iconSize?: number;
    strokeWidth?: number;
    ariaLabel?: string;
    size?: string;
    rounded?: string;
    variant?: Variant;
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

  const colors = $derived(VARIANT_CLASSES[variant]);
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
  class="{size} {colors.bg} {colors.hover} {colors.text} {rounded} flex items-center justify-center transition-colors {customClass}"
  class:opacity-80={disabled}
  class:cursor-not-allowed={disabled}
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
