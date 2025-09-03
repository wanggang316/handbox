<script lang="ts">
  import { TriangleAlert, CircleCheck, TriangleAlert as Warning, Info, X } from '@lucide/svelte';
  import { toastStore, toastActions, type ToastMessage } from '$lib/stores/toast.svelte';

  function getIcon(type: ToastMessage['type']) {
    switch (type) {
      case 'success': return CircleCheck;
      case 'warning': return Warning;
      case 'info': return Info;
      default: return TriangleAlert;
    }
  }

  function getStyles(type: ToastMessage['type']) {
    switch (type) {
      case 'success':
        return {
          container: 'bg-green-50 dark:bg-green-900/90 border-green-200 dark:border-green-800',
          icon: 'text-green-600 dark:text-green-400',
          title: 'text-green-800 dark:text-green-200',
          message: 'text-green-700 dark:text-green-300',
          close: 'text-green-400 hover:text-green-600 dark:text-green-500 dark:hover:text-green-300'
        };
      case 'warning':
        return {
          container: 'bg-yellow-50 dark:bg-yellow-900/90 border-yellow-200 dark:border-yellow-800',
          icon: 'text-yellow-600 dark:text-yellow-400',
          title: 'text-yellow-800 dark:text-yellow-200',
          message: 'text-yellow-700 dark:text-yellow-300',
          close: 'text-yellow-400 hover:text-yellow-600 dark:text-yellow-500 dark:hover:text-yellow-300'
        };
      case 'info':
        return {
          container: 'bg-blue-50 dark:bg-blue-900/90 border-blue-200 dark:border-blue-800',
          icon: 'text-blue-600 dark:text-blue-400',
          title: 'text-blue-800 dark:text-blue-200',
          message: 'text-blue-700 dark:text-blue-300',
          close: 'text-blue-400 hover:text-blue-600 dark:text-blue-500 dark:hover:text-blue-300'
        };
      default: // error
        return {
          container: 'bg-red-50 dark:bg-red-900/90 border-red-200 dark:border-red-800',
          icon: 'text-red-600 dark:text-red-400',
          title: 'text-red-800 dark:text-red-200',
          message: 'text-red-700 dark:text-red-300',
          close: 'text-red-400 hover:text-red-600 dark:text-red-500 dark:hover:text-red-300'
        };
    }
  }

  function getTitle(type: ToastMessage['type']) {
    switch (type) {
      case 'success': return '操作成功';
      case 'warning': return '注意';
      case 'info': return '提示';
      default: return '操作失败';
    }
  }
</script>

<!-- 全局 Toast 容器 -->
<div class="fixed top-4 left-1/2 transform -translate-x-1/2 z-[99999] flex flex-col gap-2">
  {#each toastStore.messages as toast (toast.id)}
    {@const Icon = getIcon(toast.type)}
    {@const styles = getStyles(toast.type)}
    <div 
      class="animate-in slide-in-from-top-2 duration-300"
      role="alert"
    >
      <div class="border rounded-lg shadow-lg p-4 min-w-80 max-w-md backdrop-blur-sm {styles.container}">
        <div class="flex items-start gap-3">
          <Icon class="h-5 w-5 mt-0.5 flex-shrink-0 {styles.icon}" />
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium {styles.title}">
              {getTitle(toast.type)}
            </p>
            <p class="text-sm mt-1 {styles.message}">
              {toast.message}
            </p>
          </div>
          <button 
            onclick={() => toastActions.remove(toast.id)}
            class="p-1 -m-1 {styles.close}"
          >
            <X class="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  {/each}
</div>