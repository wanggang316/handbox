<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import { Loader } from "@lucide/svelte";
  import { roundButton, type RoundButtonVariants } from "./variants";
  import { cn } from "./utils";

  interface Props {
    label: string;
    icon?: typeof IconType;
    iconSize?: number;
    variant?: RoundButtonVariants["variant"];
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

  const inactive = $derived(disabled || loading);

  function handleClick(event: MouseEvent) {
    if (!disabled && !loading) {
      onclick?.(event);
    }
  }
</script>

<button
  class={cn(roundButton({ variant }), size, fontSize, rounded, customClass)}
  data-slot="round-button"
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
