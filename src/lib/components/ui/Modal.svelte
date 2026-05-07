<script lang="ts">
  import { tick } from "svelte";
  import TrafficLightsRedButton from "./TrafficLightsRedButton.svelte";
  import TitleBar from "./TitleBar.svelte";

  interface Props {
    open?: boolean;
    title?: string;
    showCloseButton?: boolean;
    closeOnBackdropClick?: boolean;
    onClose?: () => void;
    children?: import("svelte").Snippet;
  }

  let {
    open = $bindable(false),
    title = "",
    showCloseButton = true,
    closeOnBackdropClick = false,
    onClose = () => {},
    children,
  }: Props = $props();

  let closing = $state(false);
  let modalElement = $state<HTMLDivElement>();

  export function handleClose() {
    closing = true;
    setTimeout(() => {
      closing = false;
      onClose();
    }, 250);
  }

  function handleBackdropClick(e: MouseEvent) {
    if (closeOnBackdropClick && e.target === e.currentTarget) {
      handleClose();
    }
  }

  $effect(() => {
    if (open && modalElement) {
      tick().then(() => modalElement?.focus());
    }
  });
</script>

{#if open}
  <div
    bind:this={modalElement}
    class="fixed inset-0 flex items-center justify-center z-[10010] animate-backdrop"
    style="background-color: var(--overlay);"
    class:animate-backdrop-close={closing}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onclick={handleBackdropClick}
    onkeydown={(e) => {
      if (e.key === "Escape") handleClose();
    }}
  >
    <TitleBar showToggleButton={false} />
    <!-- 外层容器：不裁切，允许下拉菜单溢出，负责动画 -->
    <div class="relative animate-modal" class:animate-modal-close={closing}>
      <!-- 背景层：surface-1 lift + hairline 边框 (Linear modal spec) -->
      <div
        class="bg-[var(--bg-card)] max-w-[90vw] max-h-[90vh] rounded-xl shadow-2xl overflow-hidden relative pointer-events-none border border-[var(--hairline)]"
        style="z-index: 1;"
      >
        <!-- 预留内容空间 -->
        <div class="px-0 py-0 invisible">
          {#if children}
            {@render children()}
          {/if}
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
              <h3 class="ml-4 text-base font-medium text-base-content/80">
                {title}
              </h3>
            {/if}
          </div>
        {/if}

        <!-- 内容区域：不受背景层裁切影响 -->
        <div class="px-0 py-0">
          {#if children}
            {@render children()}
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .animate-backdrop {
    animation: backdropFadeIn 0.25s ease-out;
  }

  .animate-backdrop-close {
    animation: backdropFadeOut 0.25s ease-out;
  }

  .animate-modal {
    animation: modalSlideIn 0.25s ease-out;
  }

  .animate-modal-close {
    animation: modalSlideOut 0.25s ease-out;
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
      transform: translateY(-30px);
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
      transform: translateY(-30px);
    }
  }
</style>
