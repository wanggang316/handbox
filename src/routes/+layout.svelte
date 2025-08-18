<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { theme, uiActions } from "$lib/stores/ui";

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
    }
  });
</script>

{@render children()}

<style></style>
