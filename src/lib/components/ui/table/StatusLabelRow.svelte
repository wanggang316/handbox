<script lang="ts">
  import { ChevronRight } from "@lucide/svelte";
  import StatusLabel from "../StatusLabel.svelte";
  import TableBaseRow from "./TableBaseRow.svelte";

  interface Props {
    label: string;
    icon?: string;
    iconSrc?: string;
    isCustomProvider?: boolean;
    status: "enabled" | "disabled" | "pending" | "error";
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
    <div class="w-6 h-6 rounded bg-gradient-to-br {isCustomProvider ? 'from-green-500 to-teal-600' : 'from-blue-500 to-purple-600'} flex items-center justify-center text-white text-xs font-medium">
      {icon}
    </div>
  {/if}
{/snippet}

<button 
  class="w-full {clickable ? 'hover:bg-bg-hover' : ''}"
  {onclick}
  onkeydown={(e) => e.key === "Enter" && onclick?.()}
>
  <TableBaseRow {label} icon={iconSnippet}>
    
    <div class="flex items-center gap-3">
      {#if status == 'enabled'}
        <StatusLabel {status} text={statusText} />
      {/if}

      {#if clickable}
        <ChevronRight size=16
          class="text-slate-400 group-hover:text-slate-600 transition-colors duration-75"
        />
      {/if}
    </div>
  </TableBaseRow>
</button>