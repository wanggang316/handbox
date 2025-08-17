<script lang="ts">
  import TrafficLightsRedButton from './TrafficLightsRedButton.svelte';

  export let open = false;
  export let title = '';
  export let onClose: () => void = () => {};
</script>

{#if open}
  <div 
    class="fixed inset-0 bg-black/50 flex items-center justify-center z-[10002]" 
    role="dialog" 
    aria-modal="true"
    tabindex="-1"
    onclick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    onkeydown={(e) => { if (e.key === 'Escape') onClose(); }}
  >
    <div class="bg-white w-[650px] max-w-4xl shadow-2xl overflow-hidden relative" style="border-radius: 20px;">
      <!-- Overlay 标题视图 -->
      <div class="absolute top-0 left-0 z-20 flex items-center px-5 py-4">
        <TrafficLightsRedButton onClick={onClose} />
        {#if title}
          <h3 class="text-base font-medium text-gray-600 ml-4">{title}</h3>
        {/if}
      </div>
      
      <!-- 内容区域 -->
      <div class="px-0 py-0">
        <slot />
      </div>
    </div>
  </div>
{/if}


