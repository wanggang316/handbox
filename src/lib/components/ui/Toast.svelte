<script lang="ts">
  import { TriangleAlert, CircleCheck, TriangleAlert as Warning, Info, X } from '@lucide/svelte';
  import { toastStore, toastActions, type ToastMessage } from '$lib/states/toast.svelte';
  import { t } from '$lib/i18n';

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
          container: 'bg-success/10 border-success/30',
          icon: 'text-success',
          title: 'text-success',
          message: 'text-success/80',
          hint: 'text-success/70',
          caption: 'text-success/60',
          close: 'text-success/70 hover:text-success',
          action: 'border-success/40 text-success hover:bg-success/10'
        };
      case 'warning':
        return {
          container: 'bg-warning/10 border-warning/30',
          icon: 'text-warning',
          title: 'text-warning',
          message: 'text-warning/80',
          hint: 'text-warning/70',
          caption: 'text-warning/60',
          close: 'text-warning/70 hover:text-warning',
          action: 'border-warning/40 text-warning hover:bg-warning/10'
        };
      case 'info':
        return {
          container: 'bg-info/10 border-info/30',
          icon: 'text-info',
          title: 'text-info',
          message: 'text-info/80',
          hint: 'text-info/70',
          caption: 'text-info/60',
          close: 'text-info/70 hover:text-info',
          action: 'border-info/40 text-info hover:bg-info/10'
        };
      default: // error
        return {
          container: 'bg-error/10 border-error/30',
          icon: 'text-error',
          title: 'text-error',
          message: 'text-error/80',
          hint: 'text-error/70',
          caption: 'text-error/60',
          close: 'text-error/70 hover:text-error',
          action: 'border-error/40 text-error hover:bg-error/10'
        };
    }
  }

  function getTitle(type: ToastMessage['type']) {
    switch (type) {
      case 'success': return t('ui.toastSuccessTitle');
      case 'warning': return t('ui.toastWarningTitle');
      case 'info': return t('ui.toastInfoTitle');
      default: return t('ui.toastErrorTitle');
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
              {toast.title ?? getTitle(toast.type)}
            </p>
            {#if toast.code}
              <p class="text-[11px] tracking-wide uppercase {styles.caption}">
                {toast.code}
              </p>
            {/if}
            <p class="text-sm mt-1 {styles.message}">
              {toast.message}
            </p>
            {#if toast.hint}
              <p class="text-xs mt-1 leading-relaxed {styles.hint}">
                {toast.hint}
              </p>
            {/if}
          </div>
          {#if toast.requiresAcknowledgement}
            <button
              onclick={() => toastActions.remove(toast.id)}
              class="px-3 py-1 text-xs font-medium rounded-full border transition-colors {styles.action}"
            >
              {toast.acknowledgeLabel ?? t('ui.gotIt')}
            </button>
          {:else}
            <button 
              onclick={() => toastActions.remove(toast.id)}
              class="p-1 -m-1 {styles.close}"
            >
              <X class="h-4 w-4" />
            </button>
          {/if}
        </div>
      </div>
    </div>
  {/each}
</div>
