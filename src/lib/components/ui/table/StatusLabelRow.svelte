<script lang="ts">
  import { ChevronRight } from "@lucide/svelte";
  import StatusLabel from "../StatusLabel.svelte";
  import TableBaseRow from "./TableBaseRow.svelte";

  interface Props {
    label: string;
    icon?: string;
    iconSrc?: string;
    isCustomProvider?: boolean;
    status: "enabled" | "disabled" | "idle" | "error";
    statusText: string;
    onclick?: () => void;
    clickable?: boolean;
  }

  let {
    label,
    icon,
    iconSrc,
    isCustomProvider = false,
    status,
    statusText,
    onclick,
    clickable = true,
  }: Props = $props();
</script>

{#snippet iconSnippet()}
  {#if iconSrc}
    <img
      src={iconSrc}
      alt="{label} logo"
      class="w-6 h-6 object-contain"
    />
  {:else if icon}
    <div
      class={`w-6 h-6 rounded flex items-center justify-center text-xs font-medium ${isCustomProvider ? 'bg-success text-success-content' : 'bg-primary text-primary-content'}`}
    >
      {icon}
    </div>
  {/if}
{/snippet}

<button 
  class="w-full {clickable ? 'hover:bg-base-300' : ''} group"
  {onclick}
  onkeydown={(e) => e.key === "Enter" && onclick?.()}
>
  <TableBaseRow {label} icon={iconSnippet}>
    
    <div class="flex items-center gap-3">
      <StatusLabel {status} text={statusText} />
      {#if clickable}
        <ChevronRight size=16
          class="text-base-content/50 group-hover:text-base-content transition-colors duration-75"
        />
      {/if}
    </div>
  </TableBaseRow>
</button>
