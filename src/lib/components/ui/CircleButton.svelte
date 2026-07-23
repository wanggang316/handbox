<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import { circleButton, type CircleButtonVariants } from "./variants";
  import { cn } from "./utils";

  interface Props {
    icon: typeof IconType;
    iconSize?: number;
    ariaLabel: string;
    variant?: CircleButtonVariants["variant"];
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

  function handleClick(event: MouseEvent) {
    if (!disabled) {
      onclick?.(event);
    }
  }
</script>

<button
  class={cn(circleButton({ variant }), size, rounded, customClass)}
  data-slot="circle-button"
  aria-label={ariaLabel}
  onclick={handleClick}
  {disabled}
>
  {#if icon}
    {@const Icon = icon}
    <Icon size={iconSize} />
  {/if}
</button>
