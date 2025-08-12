<script lang="ts">
import { page } from '$app/stores';
import { onMount } from 'svelte';
import { browser } from '$app/environment';

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

let sidebarOpen = $state(false);

// 获取当前路由的图标
function getIconPath(icon: string): string {
  switch(icon) {
    case 'chat':
      return 'M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z';
    case 'artifact':
      return 'M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547A1.934 1.934 0 004 17.693v3.621l2.053-.410a6 6 0 013.86-.517l.318.158a6 6 0 003.86.517L16.947 21v-3.621c0-.987.428-1.92 1.216-2.558z';
    case 'search':
      return 'M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z';
    case 'settings':
      return 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z';
    default:
      return '';
  }
}

// 检查当前路由是否为活跃状态
function isActive(href: string): boolean {
  if (browser && $page) {
    return $page.url.pathname.startsWith(href);
  }
  return false;
}

onMount(() => {
  // 默认重定向到聊天页面
  if (browser && $page.url.pathname === '/') {
    window.location.href = '/chat';
  }
});
</script>

<div class="app">
  <!-- 侧边栏 -->
  <nav class="sidebar" class:open={sidebarOpen}>
    <div class="sidebar-header">
      <h1>HandBox</h1>
      <button 
        class="sidebar-toggle"
        onclick={() => sidebarOpen = !sidebarOpen}
      >
        <svg width="24" height="24" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
        </svg>
      </button>
    </div>
    
    <ul class="nav-list">
      {#each navigation as item}
        <li>
          <a 
            href={item.href} 
            class="nav-item"
            class:active={isActive(item.href)}
          >
            <svg width="20" height="20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d={getIconPath(item.icon)} />
            </svg>
            <span class="nav-text">{item.name}</span>
          </a>
        </li>
      {/each}
    </ul>
  </nav>

  <!-- 主内容区域 -->
  <main class="main-content">
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

.sidebar {
  width: 240px;
  background-color: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  transition: transform 0.3s ease;
}

.sidebar-header {
  padding: 1rem;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.sidebar-header h1 {
  font-size: 1.25rem;
  font-weight: 600;
  margin: 0;
}

.sidebar-toggle {
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.sidebar-toggle:hover {
  background-color: var(--bg-hover);
}

.nav-list {
  list-style: none;
  padding: 0;
  margin: 0;
  flex: 1;
  padding-top: 1rem;
}

.nav-item {
  display: flex;
  align-items: center;
  padding: 0.75rem 1rem;
  color: var(--text-secondary);
  text-decoration: none;
  transition: all 0.2s;
  margin: 0 0.5rem;
  border-radius: 6px;
}

.nav-item:hover {
  background-color: var(--bg-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background-color: var(--bg-accent);
  color: var(--text-accent);
}

.nav-item svg {
  margin-right: 0.75rem;
  flex-shrink: 0;
}

.nav-text {
  font-weight: 500;
}

.main-content {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .sidebar {
    position: fixed;
    left: 0;
    top: 0;
    height: 100vh;
    z-index: 1000;
    transform: translateX(-100%);
  }
  
  .sidebar.open {
    transform: translateX(0);
  }
  
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