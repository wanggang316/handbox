<script lang="ts">
  import { TriangleAlert, X } from '@lucide/svelte';
  
  const { 
    message = '', 
    show = false, 
    duration = 3000,
    onClose 
  } = $props<{
    message?: string;
    show?: boolean;
    duration?: number;
    onClose?: () => void;
  }>();

  let visible = $state(false);
  let timeoutId: number | undefined;

  $effect(() => {
    if (show && message) {
      visible = true;
      
      // 自动关闭
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      
      timeoutId = setTimeout(() => {
        handleClose();
      }, duration);
    } else if (!show) {
      visible = false;
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutId = undefined;
      }
    }
  });

  function handleClose() {
    visible = false;
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    onClose?.();
  }
</script>

{#if visible}
  <div 
    class="fixed top-4 left-1/2 transform -translate-x-1/2 z-50 animate-in slide-in-from-top-2 duration-300"
    role="alert"
  >
    <div class="bg-red-50 dark:bg-red-900/90 border border-red-200 dark:border-red-800 rounded-lg shadow-lg p-4 min-w-80 max-w-md backdrop-blur-sm">
      <div class="flex items-start gap-3">
        <TriangleAlert class="h-5 w-5 text-red-600 dark:text-red-400 mt-0.5 flex-shrink-0" />
        <div class="flex-1 min-w-0">
          <p class="text-sm font-medium text-red-800 dark:text-red-200">
            操作失败
          </p>
          <p class="text-sm text-red-700 dark:text-red-300 mt-1">
            {message}
          </p>
        </div>
        <button 
          onclick={handleClose}
          class="text-red-400 hover:text-red-600 dark:text-red-500 dark:hover:text-red-300 p-1 -m-1"
        >
          <X class="h-4 w-4" />
        </button>
      </div>
    </div>
  </div>
{/if}