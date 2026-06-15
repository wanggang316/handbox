<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import { Loader } from "@lucide/svelte";

  type Variant = "primary" | "accent" | "danger" | "secondary";

  type VariantClasses = {
    bg: string;
    text: string;
    hover: string;
  };

  // Authoritative variant → class map (lifted from ConfirmModal's former
  // color helper). Reproduces the exact rest + hover colors per variant.
  const VARIANT_CLASSES: Record<Variant, VariantClasses> = {
    primary: {
      bg: "bg-primary",
      text: "text-primary-content",
      hover: "hover:opacity-90",
    },
    accent: {
      bg: "bg-accent",
      text: "text-accent-content",
      hover: "hover:bg-accent/90",
    },
    danger: {
      bg: "bg-error",
      text: "text-base-100",
      hover: "hover:bg-error/90",
    },
    secondary: {
      bg: "bg-base-300",
      text: "text-base-content/80",
      hover: "hover:bg-base-300/80",
    },
  };

  interface Props {
    label: string;
    icon?: typeof IconType;
    iconSize?: number;
    variant?: Variant;
    size?: string;
    rounded?: string;
    fontSize?: string;
    disabled?: boolean;
    loading?: boolean;
    customClass?: string;
    onclick?: (event: MouseEvent) => void;
  }

  let {
    label,
    icon = undefined,
    iconSize = 16,
    variant = "primary",
    size = "h-10",
    rounded = "rounded-full",
    fontSize = "text-[16px]",
    disabled = false,
    loading = false,
    customClass = "",
    onclick = undefined,
  }: Props = $props();

  const colors = $derived(VARIANT_CLASSES[variant]);
  const inactive = $derived(disabled || loading);
  // Hover class only applies while interactive; reproduces the exact
  // per-variant hover token (e.g. hover:bg-accent/90).
  const hoverClass = $derived(inactive ? "" : colors.hover);

  function handleClick(event: MouseEvent) {
    if (!disabled && !loading) {
      onclick?.(event);
    }
  }
</script>

<button
  class="{size} {colors.bg} {colors.text} {rounded} {fontSize} {hoverClass} flex items-center justify-center gap-1.5 disabled:bg-base-300 {customClass}"
  class:opacity-50={inactive}
  class:cursor-not-allowed={inactive}
  class:pointer-events-none={inactive}
  onclick={handleClick}
  disabled={inactive}
>
  {#if loading}
    <Loader size={iconSize} class="animate-spin" />
  {:else}
    {#if icon}
      {@const Icon = icon}
      <Icon size={iconSize} />
    {/if}
    {label}
  {/if}
</button>
