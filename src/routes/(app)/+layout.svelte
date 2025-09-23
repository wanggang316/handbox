<script lang="ts">
  import "../../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import MainSidebar from "$lib/components/sidebar/MainSidebar.svelte";
  import TitleBar from "$lib/components/ui/TitleBar.svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import { chatActions } from "$lib/states/chat.svelte";
  import ResizableSidebar from "$lib/components/ui/ResizableSidebar.svelte";

  // 侧边栏配置常量
  const SIDEBAR_AUTO_HIDE_WIDTH = 600; // 自动隐藏侧边栏的最小窗口宽度阈值
  const SIDEBAR_INITIAL_WIDTH = 240; // 侧边栏初始宽度
  const SIDEBAR_MIN_WIDTH = 200; // 侧边栏最小宽度
  const SIDEBAR_MAX_WIDTH = 300; // 侧边栏最大宽度

  let sidebarWidth = $state(SIDEBAR_INITIAL_WIDTH);
  let isDragging = $state(false);
  let windowWidth = $state(0);
  let autoHidden = $state(false); // 标记是否是自动隐藏
  let userOverrideInNarrowMode = $state(false); // 标记用户在窄屏模式下手动打开了侧边栏

  // 切换侧边栏显示状态
  function toggleSidebar() {
    uiState.toggleSidebar();
    autoHidden = false; // 手动操作时清除自动隐藏标记

    // 如果在窄屏模式下手动打开侧边栏，标记用户覆盖行为
    if (windowWidth < SIDEBAR_AUTO_HIDE_WIDTH && uiState.sidebarOpen) {
      userOverrideInNarrowMode = true;
    }
    // 如果在宽屏模式下或手动关闭，清除覆盖标记
    else if (windowWidth >= SIDEBAR_AUTO_HIDE_WIDTH || !uiState.sidebarOpen) {
      userOverrideInNarrowMode = false;
    }

    // 保存状态到 localStorage
    if (browser) {
      localStorage.setItem("sidebar.open", JSON.stringify(uiState.sidebarOpen));
    }
  }

  // 监听窗口大小变化
  function handleResize() {
    if (browser) {
      const prevWindowWidth = windowWidth;
      windowWidth = window.innerWidth;

      if (windowWidth < SIDEBAR_AUTO_HIDE_WIDTH) {
        if (uiState.sidebarOpen && !autoHidden && !userOverrideInNarrowMode) {
          uiState.setSidebarOpen(false);
          autoHidden = true;
        }
      } else if (prevWindowWidth < SIDEBAR_AUTO_HIDE_WIDTH) {
        userOverrideInNarrowMode = false;
        if (autoHidden) {
          uiState.setSidebarOpen(true);
          autoHidden = false;
        }
      }
    }
  }

  // 键盘快捷键支持
  function handleKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === "b") {
      event.preventDefault();
      toggleSidebar();
    }
  }

  // 从 localStorage 恢复侧边栏状态
  function restoreSidebarState() {
    if (browser) {
      const saved = localStorage.getItem("sidebar.open");
      if (saved !== null) {
        uiState.setSidebarOpen(JSON.parse(saved));
      }
    }
  }

  onMount(() => {
    // 全局初始化聊天状态
    chatActions.initialize();

    // 恢复侧边栏状态
    restoreSidebarState();

    // 从 localStorage 恢复侧边栏宽度
    const savedWidth = localStorage.getItem("main.sidebar.width");
    if (savedWidth) {
      sidebarWidth = parseInt(savedWidth);
    }

    if (browser) {
      windowWidth = window.innerWidth;
      handleResize();
      window.addEventListener("keydown", handleKeydown);
      window.addEventListener("resize", handleResize);
      return () => {
        window.removeEventListener("keydown", handleKeydown);
        window.removeEventListener("resize", handleResize);
      };
    }
  });

  let { children } = $props();
</script>

<div class="app">
  <TitleBar
    sidebarOpen={uiState.sidebarOpen}
    showToggleButton={true}
    on:toggle={toggleSidebar}
  />

  <div
    class="sidebar-wrapper m-2"
    class:dragging={isDragging}
    style={`width:${uiState.sidebarOpen ? sidebarWidth : 0}px`}
    aria-hidden={!uiState.sidebarOpen}
  >
    <ResizableSidebar
      on:resizeStart={() => {
        isDragging = true;
      }}
      on:resizing={(e) => {
        sidebarWidth = e.detail.width;
      }}
      on:resizeEnd={(e) => {
        isDragging = false;
        sidebarWidth = e.detail.width;
      }}
      bind:width={sidebarWidth}
      initialWidth={SIDEBAR_INITIAL_WIDTH}
      minWidth={SIDEBAR_MIN_WIDTH}
      maxWidth={SIDEBAR_MAX_WIDTH}
      storageKey="main.sidebar.width"
      containerClass=""
    >
      <MainSidebar />
    </ResizableSidebar>
  </div>

  <main class="main-content" class:sidebar-hidden={!uiState.sidebarOpen}>
    {@render children()}
  </main>
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
    width: 100vw;
    background-color: var(--base-100);
    color: var(--base-content);
    position: relative;
    overflow: hidden;
    overscroll-behavior: none;
  }

  .sidebar-wrapper {
    flex-shrink: 0;
    min-width: 0;
    transition: width 0s linear;
    overflow: hidden;
  }

  .sidebar-wrapper:not(.dragging) {
    transition: width 0.25s ease-in-out;
  }

  .main-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transition: width 0.25s ease-in-out;
  }

  @media (max-width: 768px) {
    .main-content {
      margin-left: 0;
    }
  }

  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen,
      Ubuntu, Cantarell, sans-serif;
  }
</style>
