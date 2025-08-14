<script lang="ts">
  import "../app.css";
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import Sidebar from "$lib/components/ui/Sidebar.svelte";
  import MainSidebar from "$lib/components/layout/MainSidebar.svelte";
  import TitleBar from "$lib/components/layout/TitleBar.svelte";
  import { currentPage } from "$lib/stores/ui";
  import { PanelLeftOpen, PanelLeftClose } from '@lucide/svelte';

  // 侧边栏导航配置
  const navigation = [
    {
      name: "Chat",
      href: "/chat",
      icon: "chat",
    },
    {
      name: "Artifacts",
      href: "/artifacts",
      icon: "artifact",
    },
    {
      name: "Search",
      href: "/search",
      icon: "search",
    },
    {
      name: "Settings",
      href: "/settings",
      icon: "settings",
    },
  ];

  let sidebarOpen = $state(true);
  let sidebarWidth = $state(347);
  let isDragging = $state(false);

  // 切换侧边栏显示状态
  function toggleSidebar() {
    sidebarOpen = !sidebarOpen;
    // 保存状态到 localStorage
    if (browser) {
      localStorage.setItem('sidebar.open', JSON.stringify(sidebarOpen));
    }
  }

  // 键盘快捷键支持
  function handleKeydown(event: KeyboardEvent) {
    // Cmd/Ctrl + B 切换侧边栏
    if ((event.metaKey || event.ctrlKey) && event.key === 'b') {
      event.preventDefault();
      toggleSidebar();
    }
  }

  // 从 localStorage 恢复侧边栏状态
  function restoreSidebarState() {
    if (browser) {
      const saved = localStorage.getItem('sidebar.open');
      if (saved !== null) {
        sidebarOpen = JSON.parse(saved);
      }
    }
  }

  // 检查当前路由是否为活跃状态
  function isActive(href: string): boolean {
    if (browser && $page) {
      return $page.url.pathname.startsWith(href);
    }
    return true;
  }

  onMount(() => {
    if (browser && $page.url.pathname === "/") {
      window.location.href = "/chat";
    }
    // 恢复侧边栏状态
    restoreSidebarState();
    
    // 添加键盘快捷键监听
    if (browser) {
      window.addEventListener('keydown', handleKeydown);
      
      return () => {
        window.removeEventListener('keydown', handleKeydown);
      };
    }
  });
</script>

  <div class="app">
  <TitleBar {sidebarOpen} on:toggle={toggleSidebar} />
  
  <div class="sidebar-wrapper m-2 rounded-2xl overflow-hidden" class:dragging={isDragging} style={`width:${sidebarOpen ? sidebarWidth : 0}px`} aria-hidden={!sidebarOpen}>
    <Sidebar
      on:resizeStart={() => { isDragging = true; }}
      on:resizing={(e) => { sidebarWidth = e.detail.width; }}
      on:resizeEnd={(e) => { isDragging = false; sidebarWidth = e.detail.width; }}
      bind:width={sidebarWidth}
      initialWidth={347}
      minWidth={240}
      maxWidth={560}
      storageKey="main.sidebar.width"
      containerClass=""
    >
      <MainSidebar />
    </Sidebar>
  </div>

  <main
    class="main-content"
    class:sidebar-hidden={!sidebarOpen}
    style={$page.url.pathname.startsWith("/chat") ? "margin-left:0" : ""}
  >
    <slot />
  </main>
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
    width: 100vw;
    background-color: var(--bg-primary);
    color: var(--text-primary);
    position: relative;
    overflow: hidden; /* 禁用应用级别的滚动 */
    overscroll-behavior: none; /* 禁用滚动边界行为 */
  }

  /* 标题栏样式已移动至 TitleBar 组件 */

  /* 侧边栏容器动画 */
  .sidebar-wrapper {
    flex-shrink: 0;
    height: 100%;
    min-width: 0;
    transition: width 0.0s linear; /* 拖拽时无过渡，避免卡顿 */
    overflow: hidden;
  }

  /* 非拖拽态启用平滑过渡 */
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

  /* 响应式设计 */
  @media (max-width: 768px) {
    .sidebar-toggle-button {
      left: 20px;
      top: 12px;
    }
    
    .main-content {
      margin-left: 0;
    }
  }

  @media (max-width: 480px) {
    .sidebar-toggle-button {
      left: 15px;
      top: 10px;
    }
  }

  /* CSS 变量定义 */
  :global(:root) {
    --bg-primary: #ffffff;
    --bg-secondary: #f8fafc;
    --bg-hover: #f1f5f9;
    --bg-accent: #3b82f6;
    --text-primary: #1e293b;
    --text-secondary: #64748b;
    --text-accent: #ffffff;
    --border-color: #e2e8f0;
  }

  :global([data-theme="dark"]) {
    --bg-primary: #0f172a;
    --bg-secondary: #1e293b;
    --bg-hover: #334155;
    --bg-accent: #3b82f6;
    --text-primary: #f8fafc;
    --text-secondary: #94a3b8;
    --text-accent: #ffffff;
    --border-color: #334155;
  }

  /* 全局样式重置 */
  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen,
      Ubuntu, Cantarell, sans-serif;
  }
</style>
