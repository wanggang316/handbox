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

<div class="flex h-screen bg-[color:var(--bg-page)]">
  <TitleBar sidebarOpen={false} showToggleButton={false} onToggle={handleToggle} />

  <div class="my-2 ml-2 w-56">
    <SettingsSidebar/>
  </div>

  <main
    class="flex-1 overflow-auto bg-[color:var(--bg-card)] border-l border-[var(--hairline)] rounded-tl-xl rounded-bl-xl"
  >
    <!-- 内容列限宽：设置内容不随窗口拉满（对齐 Linear/Codex 的设置页阅读宽度） -->
    <div class="mx-auto w-full max-w-3xl">
      {@render children()}
    </div>
  </main>
</div>
