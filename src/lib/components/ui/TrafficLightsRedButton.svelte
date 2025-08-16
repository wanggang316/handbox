<script lang="ts">
  import { X } from '@lucide/svelte';

  interface Props {
    onClick?: () => void;
  }

  let { onClick }: Props = $props();

  let isHovered = $state(false);
</script>

<button 
  class="w-3 h-3 rounded-full flex items-center justify-center transition-all duration-200"
  style="background-color: {isHovered ? '#FF5F56' : '#E3E3E2'}"
  aria-label="Close"
  onclick={onClick}
  onmouseenter={() => isHovered = true}
  onmouseleave={() => isHovered = false}
>
  {#if isHovered}
    <X size={8} style="color: #730000; stroke-width: 2.5" />
  {/if}
</button>
