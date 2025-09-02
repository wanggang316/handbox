<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { theme, uiActions } from "$lib/stores/ui";
  import { providerActions } from "$lib/states/provider.svelte";

  let { children } = $props();

  onMount(() => {
    // 初始化主题
    if (browser) {
      const savedTheme = localStorage.getItem('theme');
      if (savedTheme && ['light', 'dark', 'system'].includes(savedTheme)) {
        uiActions.setTheme(savedTheme as 'light' | 'dark' | 'system');
      } else {
        uiActions.setTheme('system');
      }
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      const handleSystemThemeChange = () => {
        if ($theme === 'system') {
          uiActions.setTheme('system');
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

<style></style>
