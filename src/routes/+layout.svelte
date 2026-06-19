<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { uiState } from "$lib/states/ui.svelte";
  import { providerActions } from "$lib/states/provider.svelte";
  import { settingsState } from "$lib/states/settings.svelte";
  import { initAuth, cleanupAuth } from "$lib/states/auth.svelte";
  import Toast from "$lib/components/ui/Toast.svelte";
  import type { Theme, Language } from "$lib/types/settings";

  let { children } = $props();

  onMount(() => {
    if (!browser) {
      return () => {
        cleanupAuth();
      };
    }

    const allowedThemes = new Set<Theme>(["light", "dark", "system"]);
    const savedTheme = localStorage.getItem("theme");
    if (savedTheme && allowedThemes.has(savedTheme as Theme)) {
      uiState.setTheme(savedTheme as Theme);
    } else {
      uiState.setTheme("system");
    }

    // 启动时仅把已从 localStorage 初始化的语言同步到 document.lang。
    // 不要在此用启动快照回写 localStorage——后端权威回填（见下）才是唯一
    // 应当写缓存的被动点，否则多窗口 reload 时两者会相互覆盖、反复闪动。
    const allowedLanguages = new Set<Language>(["zh-CN", "en-US"]);
    document.documentElement.lang = uiState.language;

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleSystemThemeChange = () => {
      if (uiState.theme === "system") {
        uiState.setTheme("system");
      }
    };
    mediaQuery.addEventListener("change", handleSystemThemeChange);

    const handleStorageChange = (event: StorageEvent) => {
      if (event.key === "theme") {
        if (event.newValue && allowedThemes.has(event.newValue as Theme)) {
          uiState.setTheme(event.newValue as Theme);
        } else if (event.newValue === null) {
          uiState.setTheme("system");
        }
      } else if (event.key === "language") {
        // 跨窗口被动同步：发起方已写过共享 localStorage，这里只更新内存与
        // document.lang，绝不回写，避免触发新一轮广播形成闪动。
        if (event.newValue && allowedLanguages.has(event.newValue as Language)) {
          uiState.syncLanguageFromExternal(event.newValue as Language);
        }
      }
    };
    window.addEventListener("storage", handleStorageChange);

    providerActions.loadProviderConfigs().catch((error) => {
      console.error("Failed to load provider configs:", error);
    });

    // 预加载 providers with models，这样子页面就不需要重复加载
    providerActions.loadProvidersWithModels(false).catch((error) => {
      console.error("Failed to load providers:", error);
    });

    // 预加载 settings，这样子页面就不需要重复加载；
    // 加载完成后用后端持久化的语言做权威回填。
    settingsState
      .loadSettings()
      .then(() => {
        const lang = settingsState.settings?.general.language;
        if (lang && allowedLanguages.has(lang)) {
          uiState.setLanguage(lang);
        }
      })
      .catch((error) => {
        console.error("Failed to load settings:", error);
      });

    initAuth().catch((error) => {
      console.error("Failed to initialize auth:", error);
    });

    return () => {
      mediaQuery.removeEventListener("change", handleSystemThemeChange);
      window.removeEventListener("storage", handleStorageChange);
      cleanupAuth();
    };
  });
</script>

{@render children()}

<!-- 全局 Toast 组件 -->
<Toast />

<style></style>
