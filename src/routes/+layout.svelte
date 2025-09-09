<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { uiState } from "$lib/states/ui.svelte";
  import { providerActions } from "$lib/states/provider.svelte";
  import Toast from "$lib/components/ui/Toast.svelte";

  let { children } = $props();

  onMount(() => {
    // 初始化主题
    if (browser) {
      const savedTheme = localStorage.getItem('theme');
      if (savedTheme && ['light', 'dark', 'system'].includes(savedTheme)) {
        uiState.setTheme(savedTheme as 'light' | 'dark' | 'system');
      } else {
        uiState.setTheme('system');
      }
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleSystemThemeChange = () => {
        if (uiState.theme === 'system') {
          uiState.setTheme('system');
        }
      };
      mediaQuery.addEventListener('change', handleSystemThemeChange);
      
      // 初始化供应商配置模板
      providerActions.loadProviderConfigs().catch(error => {
        console.error('Failed to load provider configs:', error);
      });
    }
  });
</script>

{@render children()}

<!-- 全局 Toast 组件 -->
<Toast />

<style></style>
