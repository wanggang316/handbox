<script lang="ts">
  import { X } from '@lucide/svelte';

  interface Props {
    onClick?: () => void;
  }

  let { onClick }: Props = $props();

  let isHovered = $state(false);
</script>

<button 
  class="w-3 h-3 rounded-full flex items-center justify-center transition-colors duration-200"
  class:bg-error={isHovered}
  class:bg-base-300={!isHovered}
  aria-label="Close"
  onclick={onClick}
  onmouseenter={() => isHovered = true}
  onmouseleave={() => isHovered = false}
>
  {#if isHovered}
    <X size={8} class="text-error-content" style="stroke-width: 2.5" />
  {/if}
</button>
