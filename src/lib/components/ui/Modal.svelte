<script lang="ts">
  import TrafficLightsRedButton from './TrafficLightsRedButton.svelte';
  import TitleBar from './TitleBar.svelte';

  export let open = false;
  export let title = '';
  export let showCloseButton = true;
  export let onClose: () => void = () => {};
  
  let closing = false;
  let modalElement: HTMLDivElement;
  
  export function handleClose() {
    closing = true;
    setTimeout(() => {
      closing = false;
      onClose();
    }, 300);
  }
  
  $: if (open && modalElement) {
    modalElement.focus();
  }
</script>

{#if open}
  <div 
    bind:this={modalElement}
    class="fixed inset-0 bg-base-content/30 flex items-center justify-center z-[10010] animate-backdrop" 
    class:animate-backdrop-close={closing}
    role="dialog" 
    aria-modal="true"
    tabindex="-1"
    onkeydown={(e) => { if (e.key === 'Escape') handleClose(); }}
  >
    <TitleBar showToggleButton={false} />
    <!-- 外层容器：不裁切，允许下拉菜单溢出，负责动画 -->
    <div 
      class="relative animate-modal"
      class:animate-modal-close={closing}
    >
      <!-- 背景层：负责视觉效果和边界裁切，但不影响内容层 -->
      <div 
        class="bg-base-100 max-w-4xl rounded-2xl shadow-2xl overflow-hidden relative pointer-events-none" 
        style="border-radius: 20px; z-index: 1;"
      >
        <!-- 预留内容空间 -->
        <div class="px-0 py-0 invisible">
          <slot />
        </div>
      </div>
      
      <!-- 内容层：独立于背景层，不受裁切影响 -->
      <div 
        class="absolute inset-0 max-w-4xl bg-transparent"
        style="z-index: 2;"
      >
        <!-- Overlay 标题视图 -->
        {#if showCloseButton || title}
          <div class="absolute top-0 left-0 z-20 flex items-center px-5 py-4">
            {#if showCloseButton}
              <TrafficLightsRedButton onClick={handleClose} />
            {/if}
            {#if title}
              <h3 class="text-base font-medium text-base-content/80 ml-4">{title}</h3>
            {/if}
          </div>
        {/if}
        
        <!-- 内容区域：不受背景层裁切影响 -->
        <div class="px-0 py-0">
          <slot />
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .animate-backdrop {
    animation: backdropFadeIn 0.3s ease-out;
  }
  
  .animate-backdrop-close {
    animation: backdropFadeOut 0.3s ease-out;
  }
  
  .animate-modal {
    animation: modalSlideIn 0.3s ease-out;
  }
  
  .animate-modal-close {
    animation: modalSlideOut 0.3s ease-out;
  }

  @keyframes backdropFadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  
  @keyframes backdropFadeOut {
    from {
      opacity: 1;
    }
    to {
      opacity: 0;
    }
  }

  @keyframes modalSlideIn {
    from {
      opacity: 0;
      transform: translateY(-40px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  
  @keyframes modalSlideOut {
    from {
      opacity: 1;
      transform: translateY(0);
    }
    to {
      opacity: 0;
      transform: translateY(-40px);
    }
  }
</style>
