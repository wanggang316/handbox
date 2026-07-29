<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { afterNavigate, goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import { navigationState } from "$lib/states/navigation.svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import { providerActions } from "$lib/states/provider.svelte";
  import { settingsState } from "$lib/states/settings.svelte";
  import { initAuth, cleanupAuth } from "$lib/states/auth.svelte";
  import Toast from "$lib/components/ui/Toast.svelte";
  import type { Theme, Language } from "$lib/types/settings";

  let { children } = $props();

  // 记录主界面路由：设置页的「返回应用」据此回跳。
  afterNavigate((nav) => {
    const path = nav.to?.url.pathname;
    if (path) navigationState.remember(path);
  });

  // WebView 自带的右键菜单是浏览器语义（Look Up / Translate / Search with
  // Google / Inspect Element），出现在桌面应用里既突兀又暴露 web 外壳。全局压掉，
  // 只在可编辑控件里放行——那里的剪切 / 拷贝 / 粘贴 / 拼写建议是真实需求。
  // 应用自己的右键菜单（如 sidebar 会话行）在各自 handler 里已 preventDefault，
  // 不经过这里。开发期查元素改用 ⌥⌘I。
  function handleContextMenu(event: MouseEvent) {
    const target = event.target as Element | null;
    if (target?.closest("input, textarea, [contenteditable='true']")) return;
    event.preventDefault();
  }

  onMount(() => {
    if (!browser) {
      return () => {
        cleanupAuth();
      };
    }

    // 设置在主窗口内渲染：原生菜单（⌘,）与其他 webview 窗口经
    // open_settings_window 命令定向 emit 本事件，这里承接并导航。
    let unlistenSettingsNavigate: (() => void) | undefined;
    if (isTauriEnvironment()) {
      listen<string>("settings:navigate", (event) => {
        goto(event.payload);
      })
        .then((fn) => (unlistenSettingsNavigate = fn))
        .catch((error) => {
          console.error("Failed to listen settings:navigate:", error);
        });
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

    // 重预加载只在主窗口跑：4 个隐藏辅助窗口（划词×3 / quick action）各 boot 一份
    // 同一 SPA，若全都预载 providers/auth，冷启动期是 5 份重复 IPC 抢主窗口首屏资源。
    // settings 仍全窗口加载（轻量本地读，主题/划词翻译依赖）。
    const isMainWindow =
      !isTauriEnvironment() || getCurrentWindow().label === "main";

    if (isMainWindow) {
      providerActions.loadProviderConfigs().catch((error) => {
        console.error("Failed to load provider configs:", error);
      });

      // 预加载 providers with models，这样子页面就不需要重复加载
      providerActions.loadProvidersWithModels(false).catch((error) => {
        console.error("Failed to load providers:", error);
      });
    }

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

    if (isMainWindow) {
      initAuth().catch((error) => {
        console.error("Failed to initialize auth:", error);
      });
    }

    return () => {
      unlistenSettingsNavigate?.();
      mediaQuery.removeEventListener("change", handleSystemThemeChange);
      window.removeEventListener("storage", handleStorageChange);
      cleanupAuth();
    };
  });
</script>

<svelte:window oncontextmenu={handleContextMenu} />

{@render children()}

<!-- 全局 Toast 组件 -->
<Toast />

<style></style>
