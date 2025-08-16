<script lang="ts">
  import "../app.css";
  import { page } from "$app/stores";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import MainSidebar from "$lib/components/sidebar/MainSidebar.svelte";
  import TitleBar from "$lib/components/ui/TitleBar.svelte";
  import { currentPage, sidebarOpen, theme, uiActions } from "$lib/stores/ui";
  import ResizableSidebar from "$lib/components/ui/ResizableSidebar.svelte";

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

  // 侧边栏配置常量
  const SIDEBAR_AUTO_HIDE_WIDTH = 600;  // 自动隐藏侧边栏的最小窗口宽度阈值
  const SIDEBAR_INITIAL_WIDTH = 240;    // 侧边栏初始宽度
  const SIDEBAR_MIN_WIDTH = 200;        // 侧边栏最小宽度
  const SIDEBAR_MAX_WIDTH = 300;        // 侧边栏最大宽度

  let sidebarWidth = $state(SIDEBAR_INITIAL_WIDTH);
  let isDragging = $state(false);
  let windowWidth = $state(0);
  let autoHidden = $state(false); // 标记是否是自动隐藏
  let userOverrideInNarrowMode = $state(false); // 标记用户在窄屏模式下手动打开了侧边栏

  // 切换侧边栏显示状态
  function toggleSidebar() {
    sidebarOpen.update(open => !open);
    autoHidden = false; // 手动操作时清除自动隐藏标记
    
    // 如果在窄屏模式下手动打开侧边栏，标记用户覆盖行为
    if (windowWidth < SIDEBAR_AUTO_HIDE_WIDTH && $sidebarOpen) {
      userOverrideInNarrowMode = true;
    } 
    // 如果在宽屏模式下或手动关闭，清除覆盖标记
    else if (windowWidth >= SIDEBAR_AUTO_HIDE_WIDTH || !$sidebarOpen) {
      userOverrideInNarrowMode = false;
    }
    
    // 保存状态到 localStorage
    if (browser) {
      localStorage.setItem('sidebar.open', JSON.stringify($sidebarOpen));
    }
  }

  // 监听窗口大小变化
  function handleResize() {
    if (browser) {
      const prevWindowWidth = windowWidth;
      windowWidth = window.innerWidth;
      
      // 当窗口宽度小于阈值时的处理
      if (windowWidth < SIDEBAR_AUTO_HIDE_WIDTH) {
        // 只有在以下情况下才自动隐藏侧边栏：
        // 1. 侧边栏当前是打开的
        // 2. 不是已经自动隐藏的状态
        // 3. 用户没有在窄屏模式下手动覆盖（强制打开）
        if ($sidebarOpen && !autoHidden && !userOverrideInNarrowMode) {
          sidebarOpen.set(false);
          autoHidden = true;
        }
      } 
      // 当窗口宽度从小于阈值变为大于等于阈值时
      else if (prevWindowWidth < SIDEBAR_AUTO_HIDE_WIDTH) {
        // 清除窄屏模式下的用户覆盖标记
        userOverrideInNarrowMode = false;
        
        // 如果之前是自动隐藏状态，恢复侧边栏
        if (autoHidden) {
          sidebarOpen.set(true);
          autoHidden = false;
        }
      }
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
        sidebarOpen.set(JSON.parse(saved));
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
    
    // 初始化主题
    if (browser) {
      // 从 localStorage 恢复主题设置
      const savedTheme = localStorage.getItem('theme');
      if (savedTheme && ['light', 'dark', 'system'].includes(savedTheme)) {
        uiActions.setTheme(savedTheme as 'light' | 'dark' | 'system');
      } else {
        // 默认跟随系统主题
        uiActions.setTheme('system');
      }
      
      // 监听系统主题变化
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleSystemThemeChange = () => {
        if ($theme === 'system') {
          uiActions.setTheme('system'); // 重新应用系统主题
        }
      };
      mediaQuery.addEventListener('change', handleSystemThemeChange);
    }
    
    // 添加键盘快捷键监听和窗口大小监听
    if (browser) {
      // 初始化窗口宽度
      windowWidth = window.innerWidth;
      handleResize(); // 初始检查
      
      window.addEventListener('keydown', handleKeydown);
      window.addEventListener('resize', handleResize);
      
      return () => {
        window.removeEventListener('keydown', handleKeydown);
        window.removeEventListener('resize', handleResize);
      };
    }
  });
</script>

  <div class="app">
  <TitleBar sidebarOpen={$sidebarOpen} on:toggle={toggleSidebar} />
  
  <div class="sidebar-wrapper m-2" class:dragging={isDragging} style={`width:${$sidebarOpen ? sidebarWidth : 0}px`} aria-hidden={!$sidebarOpen}>
    <ResizableSidebar
      on:resizeStart={() => { isDragging = true; }}
      on:resizing={(e) => { sidebarWidth = e.detail.width; }}
      on:resizeEnd={(e) => { isDragging = false; sidebarWidth = e.detail.width; }}
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
    class:sidebar-hidden={!$sidebarOpen}
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
    /* height: 100%; */
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
    .main-content {
      margin-left: 0;
    }
  }

  /* 主题变量现在在 app.css 中使用 @theme 指令定义 */

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
