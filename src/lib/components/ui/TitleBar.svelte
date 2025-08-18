<script lang="ts">
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import { PanelLeft } from '@lucide/svelte';
  import { createEventDispatcher } from 'svelte';

  export let sidebarOpen: boolean = true;
  export let showToggleButton: boolean = true;

  const dispatch = createEventDispatcher();
  function handleToggle() {
    dispatch('toggle');
  }
</script>

<div class="drag-region" data-tauri-drag-region>
  {#if showToggleButton}
    <div class="sidebar-toggle-button">
      <IconButton
        icon={PanelLeft}
        ariaLabel={sidebarOpen ? "隐藏侧边栏 (⌘B)" : "显示侧边栏 (⌘B)"}
        on:click={handleToggle}
      />
    </div>
  {/if}
  <slot />
  <!-- 如果未来还需要在标题栏放入其他控件，可通过 slot 注入 -->
  
</div>

<style>
  /* 拖拽区域 - 在 titleBarStyle: "Overlay" 模式下使用 */
  .drag-region {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 50px;
    z-index: 9999;
    user-select: none;
    -webkit-user-select: none;
    pointer-events: auto;
  }

  /* 侧边栏切换按钮 */
  .sidebar-toggle-button {
    position: absolute;
    top: 11px;
    left: 100px; /* 位于系统按钮右边 */
    pointer-events: auto;
    z-index: 10000;
    transition: opacity 0.2s ease-in-out;
  }

  .sidebar-toggle-button:hover {
    opacity: 1.0;
  }

  /* 响应式设计：调整标题栏按钮位置 */
  /* @media (max-width: 500px) {
    .sidebar-toggle-button {
      left: 20px;
      top: 12px;
    }
  } */

  /* @media (max-width: 480px) {
    .sidebar-toggle-button {
      left: 15px;
      top: 10px;
    }
  } */
</style>


