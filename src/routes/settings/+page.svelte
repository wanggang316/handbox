<script lang="ts">
  import { onMount } from 'svelte';
  import { 
    User, 
    Palette, 
    Brain, 
    Zap, 
    Keyboard, 
    Info 
  } from '@lucide/svelte';
  
  // 设置页面状态
  let activeSection = $state('account');
  
  // 设置菜单项
  const settingsMenu = [
    { 
      id: 'account', 
      title: '账户', 
      icon: User,
      description: '登录状态、用户资料管理'
    },
    { 
      id: 'general', 
      title: '通用', 
      icon: Palette,
      description: '外观、语言、主题等基础设置'
    },
    { 
      id: 'models', 
      title: '模型', 
      icon: Brain,
      description: '管理AI模型供应商和模型配置'
    },
    { 
      id: 'mcp', 
      title: 'MCP', 
      icon: Zap,
      description: 'Model Context Protocol 服务器管理'
    },
    { 
      id: 'shortcuts', 
      title: '快捷键', 
      icon: Keyboard,
      description: '自定义键盘快捷键'
    },
    { 
      id: 'about', 
      title: '关于', 
      icon: Info,
      description: '版本信息、更新、官网链接'
    }
  ];

  function handleMenuClick(sectionId: string) {
    activeSection = sectionId;
  }
</script>

<div class="settings-layout">
  <!-- 设置侧边栏 -->
  <div class="settings-sidebar">
    <div class="sidebar-header">
      <h2>设置</h2>
    </div>
    
    <div class="sidebar-content">
      <nav class="settings-nav">
        {#each settingsMenu as item (item.id)}
          {@const IconComponent = item.icon}
          <button 
            class="nav-item"
            class:active={activeSection === item.id}
            onclick={() => handleMenuClick(item.id)}
          >
            <div class="nav-icon">
              <IconComponent size={18} />
            </div>
            <div class="nav-content">
              <div class="nav-title">{item.title}</div>
              <div class="nav-desc">{item.description}</div>
            </div>
          </button>
        {/each}
      </nav>
    </div>
  </div>

  <!-- 设置内容区域 -->
  <div class="settings-main">
    <div class="content-container">
      {#if activeSection === 'account'}
        <div class="content-section">
          <h3>账户设置</h3>
          <div class="section-content">
            <p>用户登录、资料管理等功能将在这里实现</p>
          </div>
        </div>
      {:else if activeSection === 'general'}
        <div class="content-section">
          <h3>通用设置</h3>
          <div class="section-content">
            <p>外观主题、语言、界面等设置将在这里实现</p>
          </div>
        </div>
      {:else if activeSection === 'models'}
        <div class="content-section">
          <h3>模型管理</h3>
          <div class="section-content">
            <p>AI模型供应商和模型配置将在这里实现</p>
          </div>
        </div>
      {:else if activeSection === 'mcp'}
        <div class="content-section">
          <h3>MCP 设置</h3>
          <div class="section-content">
            <p>Model Context Protocol 服务器管理将在这里实现</p>
          </div>
        </div>
      {:else if activeSection === 'shortcuts'}
        <div class="content-section">
          <h3>快捷键设置</h3>
          <div class="section-content">
            <p>键盘快捷键自定义将在这里实现</p>
          </div>
        </div>
      {:else if activeSection === 'about'}
        <div class="content-section">
          <h3>关于 HandBox</h3>
          <div class="section-content">
            <p>版本信息、更新检查、官网链接等将在这里实现</p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
/* 设置页面布局 */
.settings-layout {
  display: flex;
  height: 100vh;
  background: var(--bg-primary);
}

/* 设置侧边栏 - 参考 MainSidebar 样式 */
.settings-sidebar {
  width: 280px;
  flex-shrink: 0;
  background: #f8f8f8;
  border-radius: 0 16px 16px 0;
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.sidebar-header {
  padding: 24px 20px 16px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  flex-shrink: 0;
}

.sidebar-header h2 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
  color: var(--text-primary);
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
}

/* 导航菜单样式 */
.settings-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  width: 100%;
  padding: 12px 16px;
  background: transparent;
  border: none;
  border-radius: 12px;
  cursor: pointer;
  text-align: left;
  transition: all 0.2s ease;
  color: var(--text-primary);
}

.nav-item:hover {
  background: rgba(0, 0, 0, 0.05);
}

.nav-item.active {
  background: var(--bg-accent);
  color: white;
}

.nav-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 12px;
  flex-shrink: 0;
}

.nav-content {
  flex: 1;
  min-width: 0;
}

.nav-title {
  font-size: 0.875rem;
  font-weight: 500;
  margin-bottom: 2px;
  line-height: 1.4;
}

.nav-desc {
  font-size: 0.75rem;
  opacity: 0.7;
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nav-item.active .nav-desc {
  opacity: 0.9;
}

/* 主内容区域 */
.settings-main {
  flex: 1;
  background: var(--bg-primary);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.content-container {
  flex: 1;
  overflow-y: auto;
  padding: 32px 40px;
  max-width: 800px;
}

.content-section {
  margin-bottom: 2rem;
}

.content-section h3 {
  margin: 0 0 24px 0;
  font-size: 1.75rem;
  font-weight: 600;
  color: var(--text-primary);
}

.section-content {
  color: var(--text-secondary);
  line-height: 1.6;
}

.section-content p {
  margin: 0 0 1rem 0;
}

/* 响应式设计 */
@media (max-width: 768px) {
  .settings-layout {
    flex-direction: column;
    height: auto;
    min-height: 100vh;
  }
  
  .settings-sidebar {
    width: 100%;
    border-radius: 0;
    height: auto;
    max-height: 40vh;
  }
  
  .sidebar-content {
    overflow-y: visible;
    max-height: none;
  }
  
  .settings-nav {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 8px;
  }
  
  .nav-item {
    flex-direction: column;
    text-align: center;
    padding: 16px 8px;
  }
  
  .nav-icon {
    margin-right: 0;
    margin-bottom: 8px;
  }
  
  .nav-desc {
    white-space: normal;
    text-align: center;
  }
  
  .content-container {
    padding: 24px 16px;
  }
  
  .content-section h3 {
    font-size: 1.5rem;
    margin-bottom: 16px;
  }
}

/* 暗色主题适配 */
@media (prefers-color-scheme: dark) {
  .settings-sidebar {
    background: #2a2a2a;
  }
  
  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
  }
  
  .sidebar-header {
    border-bottom-color: rgba(255, 255, 255, 0.1);
  }
}
</style>