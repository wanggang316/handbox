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

  // Remember the last main-area route so settings' "back to app" can return to it.
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

    // Settings render inside the main window: the native menu (⌘,) and other
    // webview windows emit this event via open_settings_window; navigate here.
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

    // Only sync the already-initialized language to document.lang here. Never
    // write localStorage from this startup snapshot — the backend backfill below
    // is the sole passive writer; otherwise multi-window reloads overwrite each
    // other and flicker.
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
        // Passive cross-window sync: the initiator already wrote localStorage;
        // update memory + document.lang only, never write back, to avoid a
        // broadcast loop.
        if (event.newValue && allowedLanguages.has(event.newValue as Language)) {
          uiState.syncLanguageFromExternal(event.newValue as Language);
        }
      }
    };
    window.addEventListener("storage", handleStorageChange);

    // Heavy preloads run only in the main window: the hidden helper windows each
    // boot the same SPA, and duplicate providers/auth IPC would contend with the
    // main window's first paint. Settings load in every window (light local read;
    // theme and selection translation depend on it).
    const isMainWindow =
      !isTauriEnvironment() || getCurrentWindow().label === "main";

    if (isMainWindow) {
      providerActions.loadProviderConfigs().catch((error) => {
        console.error("Failed to load provider configs:", error);
      });

      // Preload providers with models so child pages don't refetch
      providerActions.loadProvidersWithModels(false).catch((error) => {
        console.error("Failed to load providers:", error);
      });
    }

    // Preload settings for child pages; once loaded, backfill the authoritative
    // language persisted by the backend.
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

<Toast />

<style></style>
