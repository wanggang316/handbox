<script lang="ts">
  import type { Icon as IconType } from "@lucide/svelte";
  import { Loader } from "@lucide/svelte";

  export let label: string;
  export let icon: typeof IconType | undefined = undefined;
  export let iconSize: number = 16;
  export let bgColor: string = "bg-primary";
  export let hoverColor: string = "hover:opacity-90";
  export let textColor: string = "text-primary-content";
  export let size: string = "h-10";
  export let rounded: string = "rounded-full";
  export let fontSize: string = "text-[16px]";
  export let disabled: boolean = false;
  export let loading: boolean = false;
  export let customClass: string = "";
  export let onclick: ((event: MouseEvent) => void) | undefined = undefined;

  function handleClick(event: MouseEvent) {
    if (!disabled && !loading) {
      onclick?.(event);
    }
  }
</script>

<button
  class="{size} {bgColor} {textColor} {rounded} {fontSize} flex items-center justify-center gap-1.5 disabled:bg-base-300 {customClass}"
  class:hover:opacity-90={!disabled &&
    !loading &&
    hoverColor === "hover:opacity-90"}
  class:hover:bg-opacity-90={!disabled &&
    !loading &&
    hoverColor !== "hover:opacity-90"}
  class:opacity-50={disabled || loading}
  class:cursor-not-allowed={disabled || loading}
  class:pointer-events-none={disabled || loading}
  on:click={handleClick}
  disabled={disabled || loading}
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
