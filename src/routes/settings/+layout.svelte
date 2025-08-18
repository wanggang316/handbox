<script lang="ts">
  import "../../app.css";
  import SettingsSidebar from "$lib/components/settings/SettingsSidebar.svelte";
  import TitleBar from "$lib/components/ui/TitleBar.svelte";
  
  let { children } = $props();

  // 设置页面不需要侧边栏切换功能，传递空函数
  function handleToggle() {
    // 空函数 - 设置页面不需要侧边栏切换功能
  }
</script>

<div class="flex h-screen bg-[color:var(--bg-primary)]">
  <TitleBar sidebarOpen={false} showToggleButton={false} on:toggle={handleToggle} />
  
  <div class="m-2 w-40">
    <SettingsSidebar/>
  </div>
  
  <main class="flex-1 overflow-auto">
    {@render children()}
  </main>
</div>
