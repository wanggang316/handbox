<script lang="ts">
  import { X } from "@lucide/svelte";
  import TitleBar from "./TitleBar.svelte";
  import IconButton from "./IconButton.svelte";

  interface Props {
    open: boolean;
    title?: string;
    showCloseButton?: boolean;
    onClose: () => void;
    children?: import("svelte").Snippet;
  }

  let {
    open = false,
    title = "",
    showCloseButton = true,
    onClose,
    children,
  }: Props = $props();

  let closing = $state(false);
  let drawerElement = $state<HTMLDivElement>();

  function handleClose() {
    closing = true;
    setTimeout(() => {
      closing = false;
      onClose();
    }, 300);
  }

  $effect(() => {
    if (open && drawerElement) {
      drawerElement.focus();
    }
  });
</script>

{#if open}
  <div
    bind:this={drawerElement}
    class="fixed inset-0 z-[10010] flex"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    onkeydown={(e) => {
      if (e.key === "Escape") handleClose();
    }}
  >
    <TitleBar showToggleButton={false} />

    <!-- 背景遮罩 -->
    <div
      role="button"
      tabindex="0"
      class="absolute inset-0 animate-backdrop bg-overlay"
      class:animate-backdrop-close={closing}
      onclick={handleClose}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") handleClose();
      }}
    ></div>

    <!-- 抽屉容器 -->
    <div
      role="presentation"
      class="ml-auto relative bg-base-100 max-w-lg flex flex-col animate-drawer border-l-1 border-base-300"
      class:animate-drawer-close={closing}
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <!-- 头部 -->
      <div
        class="h-[50px] flex items-center justify-between px-4 border-b border-base-content/10"
      >
        <h2 class="text-md text-base-content">{title}</h2>
        {#if showCloseButton}
          <IconButton
            icon={X}
            ariaLabel="关闭"
            customClass="z-[10001]"
            onclick={handleClose}
          />
        {/if}
      </div>

      <!-- 内容区域 -->
      <div class="flex-1 overflow-y-auto">
        {@render children?.()}
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

  .animate-drawer {
    animation: drawerSlideIn 0.3s ease-out;
  }

  .animate-drawer-close {
    animation: drawerSlideOut 0.3s ease-out;
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

  @keyframes drawerSlideIn {
    from {
      transform: translateX(100%);
    }
    to {
      transform: translateX(0);
    }
  }

  @keyframes drawerSlideOut {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(100%);
    }
  }
</style>
