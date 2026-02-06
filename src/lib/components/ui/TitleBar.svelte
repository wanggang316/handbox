<script lang="ts">
  import IconButton from "$lib/components/ui/IconButton.svelte";
  import {
    PanelLeft,
    MessageSquarePlus,
    MessageSquareDashed,
  } from "@lucide/svelte";

  interface Props {
    sidebarOpen?: boolean;
    showToggleButton?: boolean;
    onToggle?: () => void;
    children?: import("svelte").Snippet;
    onNewChat?: () => void;
    onImplicitCreate?: () => void;
  }

  let {
    sidebarOpen = true,
    showToggleButton = true,
    onToggle,
    children,
    onNewChat,
    onImplicitCreate,
  }: Props = $props();

  function handleToggle() {
    onToggle?.();
  }

  async function handleNewChat() {
    onNewChat?.();
  }

  function handleImplicitCreate() {
    onImplicitCreate?.();
  }
</script>

<div class="drag-region" data-tauri-drag-region>
  <!-- 左侧：侧边栏切换按钮 -->
  {#if showToggleButton}
    <div class="sidebar-toggle-button">
      <IconButton
        icon={PanelLeft}
        ariaLabel={sidebarOpen ? "隐藏侧边栏 (⌘B)" : "显示侧边栏 (⌘B)"}
        onclick={handleToggle}
      />
    </div>
    <!-- 中间：头部操作按钮 -->
    <div class="header-actions">
      <IconButton
        icon={MessageSquarePlus}
        iconSize={18}
        ariaLabel="新建会话"
        onclick={handleNewChat}
        customClass="new-chat-button"
        title="新建会话"
      />
      <IconButton
        icon={MessageSquareDashed}
        iconSize={18}
        ariaLabel="临时会话"
        onclick={handleImplicitCreate}
        customClass="implicit-create-button"
        title="临时会话"
      />
    </div>
  {/if}

  {@render children?.()}
  <!-- 如果未来还需要在标题栏放入其他控件，可通过 snippet 注入 -->
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
    opacity: 1;
  }

  /* 头部操作按钮区域 */
  .header-actions {
    position: absolute;
    top: 11px;
    left: 140px; /* 侧边栏切换按钮右侧 */
    display: flex;
    align-items: center;
    gap: 8px;
    pointer-events: auto;
    z-index: 10000;
  }

  /* New Chat 按钮样式 */
  :global(.new-chat-button) {
    min-width: 28px;
    min-height: 28px;
  }

  /* 隐式创建按钮样式 */
  :global(.implicit-create-button) {
    width: 28px;
    height: 28px;
  }

  /* 响应式设计：调整标题栏按钮位置 */
  /* @media (max-width: 500px) {
    .sidebar-toggle-button {
      left: 20px;
      top: 12px;
    }
    .header-actions {
      left: 60px;
    }
  } */

  /* @media (max-width: 480px) {
    .sidebar-toggle-button {
      left: 15px;
      top: 10px;
    }
    .header-actions {
      left: 50px;
    }
  } */
</style>
