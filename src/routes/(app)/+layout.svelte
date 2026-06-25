<script lang="ts">
  import "../../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import MainSidebar from "$lib/components/sidebar/MainSidebar.svelte";
  import TitleBar from "$lib/components/ui/TitleBar.svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import { chatActions } from "$lib/states/chat.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import UpdateDialog from "$lib/components/update/UpdateDialog.svelte";
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

  // 同步 sidebarWidth 到 uiState
  $effect(() => {
    uiState.setSidebarWidth(sidebarWidth);
  });

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

    // 应用更新：加载当前版本与偏好，建立跨窗口监听并按需自动检查（仅主窗口）
    let updateUnlisten: (() => void) | null = null;
    updateState
      .load()
      .then(() => updateState.startAutoCheck())
      .then((unlisten) => {
        updateUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("Failed to init update checker:", error);
      });

    // Quick Action continue-in-chat 交接：浮层 ⌘↵ 调用后端，后端把本（主）窗口前置
    // 并广播 `quick-action-open-session`（payload = 裸 session-id 字符串）。无论主窗口
    // 当前停在 /chat / 设置 / 裸 /agent / /agent?id=<其它>，都导航到该会话
    // （VAL-CONTINUE-008..010）。在 onMount 即注册，使冷启动（窗口刚被前置首挂）时
    // 抵达的 navigate 事件也能被接住。
    let openSessionUnlisten: (() => void) | null = null;
    let openSessionStale = false; // 卸载早于 listen 解析时，丢弃迟到的 unlisten。
    if (isTauriEnvironment()) {
      listen<string>("quick-action-open-session", (event) => {
        void goto(`/agent?id=${event.payload}`);
      })
        .then((unlisten) => {
          if (openSessionStale) {
            unlisten();
            return;
          }
          openSessionUnlisten = unlisten;
        })
        .catch((error) => {
          console.error("Failed to listen for quick-action open-session:", error);
        });
    }

    if (browser) {
      windowWidth = window.innerWidth;
      handleResize();
      window.addEventListener("keydown", handleKeydown);
      window.addEventListener("resize", handleResize);
      return () => {
        window.removeEventListener("keydown", handleKeydown);
        window.removeEventListener("resize", handleResize);
        updateUnlisten?.();
        openSessionStale = true;
        openSessionUnlisten?.();
      };
    }
  });

  let { children } = $props();
</script>

<div class="app">
  <TitleBar
    sidebarOpen={uiState.sidebarOpen}
    showToggleButton={true}
    onToggle={toggleSidebar}
  />

  <div
    class="sidebar-wrapper"
    class:dragging={isDragging}
    class:open={uiState.sidebarOpen}
    style={`width:${uiState.sidebarOpen ? sidebarWidth : 0}px`}
    aria-hidden={!uiState.sidebarOpen}
  >
    <ResizableSidebar
      onResizeStart={() => {
        isDragging = true;
      }}
      onResizing={(w) => {
        sidebarWidth = w;
      }}
      onResizeEnd={(w) => {
        isDragging = false;
        sidebarWidth = w;
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

  <main
    class="main-content"
    class:sidebar-hidden={!uiState.sidebarOpen}
  >
    {@render children()}
  </main>

  <!-- 应用更新弹框（由侧边栏入口或自动检查触发） -->
  <UpdateDialog />
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
    width: 100vw;
    background-color: var(--bg-page);
    color: var(--base-content);
    position: relative;
    overflow: hidden;
    overscroll-behavior: none;
  }

  .sidebar-wrapper {
    flex-shrink: 0;
    min-width: 0;
    transition: width 0s linear, margin 0.25s ease-in-out;
    overflow: hidden;
  }

  /* 仅在 sidebar 打开时给 top / left / bottom 8px 间距；右侧贴主体 border，无 mr */
  .sidebar-wrapper.open {
    margin: 0.5rem 0 0.5rem 0.5rem;
  }

  .sidebar-wrapper:not(.dragging) {
    transition: width 0.25s ease-in-out, margin 0.25s ease-in-out;
  }

  .main-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transition: width 0.25s ease-in-out;
    /* Linear 主布局：内容卡贴 top/right/bottom 三边窗口，左侧 hairline 边 + 两角圆角 */
    background-color: var(--bg-card);
    border-left: 1px solid var(--hairline);
    border-top-left-radius: 0.75rem;
    border-bottom-left-radius: 0.75rem;
  }

  /* sidebar 关闭：主体撑满整个窗口，去掉左侧 border 和圆角 */
  .main-content.sidebar-hidden {
    border-left: 0;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
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
    /* font-family inherits from html (Geist Variable) — see app.css */
  }
</style>
