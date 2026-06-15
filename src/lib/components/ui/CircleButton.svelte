<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";

  type Variant = "neutral" | "secondary";

  type VariantClasses = {
    bg: string;
    text: string;
    hover: string;
  };

  // Authoritative variant → class map. Reproduces the exact rest + hover
  // colors per variant (neutral = the former default).
  const VARIANT_CLASSES: Record<Variant, VariantClasses> = {
    neutral: {
      bg: "bg-neutral",
      text: "text-neutral-content",
      hover: "hover:bg-neutral/90",
    },
    secondary: {
      bg: "bg-base-200",
      text: "text-base-content",
      hover: "hover:bg-base-300",
    },
  };

  interface Props {
    icon: typeof IconType;
    iconSize?: number;
    ariaLabel: string;
    variant?: Variant;
    size?: string;
    rounded?: string;
    disabled?: boolean;
    customClass?: string;
    onclick?: (event: MouseEvent) => void;
  }

  let {
    icon,
    iconSize = 16,
    ariaLabel,
    variant = "neutral",
    size = "w-10 h-10",
    rounded = "rounded-full",
    disabled = false,
    customClass = "",
    onclick = undefined,
  }: Props = $props();

  const colors = $derived(VARIANT_CLASSES[variant]);

  function handleClick(event: MouseEvent) {
    if (!disabled) {
      onclick?.(event);
    }
  }
</script>

<button
  class="{size} {colors.bg} {colors.hover} {colors.text} {rounded} flex items-center justify-center transition-colors {customClass}"
  class:opacity-50={disabled}
  class:cursor-not-allowed={disabled}
  aria-label={ariaLabel}
  onclick={handleClick}
  {disabled}
>
  {#if icon}
    {@const Icon = icon}
    <Icon size={iconSize} />
  {/if}
</button>
