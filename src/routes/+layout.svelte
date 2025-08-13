<script lang="ts">
import '../app.css';
import { page } from '$app/stores';
import { onMount } from 'svelte';
import { browser } from '$app/environment';
import Sidebar from '$lib/components/ui/Sidebar.svelte';
import MainSidebar from '$lib/components/layout/MainSidebar.svelte';
import { currentPage } from '$lib/stores/ui';

// 侧边栏导航配置
const navigation = [
  {
    name: 'Chat',
    href: '/chat',
    icon: 'chat'
  },
  {
    name: 'Artifacts',
    href: '/artifacts',
    icon: 'artifact'
  },
  {
    name: 'Search',
    href: '/search',
    icon: 'search'
  },
  {
    name: 'Settings',
    href: '/settings',
    icon: 'settings'
  }
];

let sidebarOpen = $state(true);

// 检查当前路由是否为活跃状态
function isActive(href: string): boolean {
  if (browser && $page) {
    return $page.url.pathname.startsWith(href);
  }
  return true;
}

onMount(() => {
  if (browser && $page.url.pathname === '/') {
    window.location.href = '/chat';
  }
});
</script>

<div class="app">
    <Sidebar initialWidth={347} minWidth={240} maxWidth={560} storageKey="main.sidebar.width" containerClass="m-2 rounded-2xl overflow-hidden">
      <MainSidebar />
    </Sidebar>

    <main class="main-content" style={$page.url.pathname.startsWith('/chat') ? 'margin-left:0' : ''}>
      <slot />
    </main>
  </div>


<style>
.app {
  display: flex;
  height: 100vh;
  background-color: var(--bg-primary);
  color: var(--text-primary);
}

.main-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .main-content {
    margin-left: 0;
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
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
}
</style>