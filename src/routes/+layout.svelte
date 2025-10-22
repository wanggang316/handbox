<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { uiState } from "$lib/states/ui.svelte";
  import { providerActions } from "$lib/states/provider.svelte";
  import { initAuth, cleanupAuth } from "$lib/states/auth.svelte";
  import Toast from "$lib/components/ui/Toast.svelte";
  import type { Theme, ThemeColor } from "$lib/types/settings";

  let { children } = $props();

  onMount(() => {
    if (!browser) {
      return () => {
        cleanupAuth();
      };
    }

    const allowedThemes = new Set<Theme>(['light', 'dark', 'system']);
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme && allowedThemes.has(savedTheme as Theme)) {
      uiState.setTheme(savedTheme as Theme);
    } else {
      uiState.setTheme('system');
    }

    const savedThemeColor = localStorage.getItem('themeColor');
    if (savedThemeColor) {
      uiState.setThemeColor(savedThemeColor as ThemeColor);
    }

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemThemeChange = () => {
      if (uiState.theme === 'system') {
        uiState.setTheme('system');
      }
    };
    mediaQuery.addEventListener('change', handleSystemThemeChange);

    const handleStorageChange = (event: StorageEvent) => {
      if (event.key === 'theme') {
        if (event.newValue && allowedThemes.has(event.newValue as Theme)) {
          uiState.setTheme(event.newValue as Theme);
        } else if (event.newValue === null) {
          uiState.setTheme('system');
        }
      }

      if (event.key === 'themeColor') {
        const newColor = (event.newValue ?? 'system') as ThemeColor;
        uiState.setThemeColor(newColor);
      }
    };
    window.addEventListener('storage', handleStorageChange);

    providerActions.loadProviderConfigs().catch((error) => {
      console.error('Failed to load provider configs:', error);
    });

    initAuth().catch((error) => {
      console.error('Failed to initialize auth:', error);
    });

    return () => {
      mediaQuery.removeEventListener('change', handleSystemThemeChange);
      window.removeEventListener('storage', handleStorageChange);
      cleanupAuth();
    };
  });
</script>

{@render children()}

<!-- 全局 Toast 组件 -->
<Toast />

<style></style>
