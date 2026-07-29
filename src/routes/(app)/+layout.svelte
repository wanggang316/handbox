<script lang="ts">
  import "../../app.css";
  import { onMount } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { listen } from "@tauri-apps/api/event";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import MainSidebar from "$lib/components/sidebar/MainSidebar.svelte";
  import TitleBar from "$lib/components/ui/TitleBar.svelte";
  import { uiState } from "$lib/states/ui.svelte";
  import { updateState } from "$lib/states/update.svelte";
  import UpdateDialog from "$lib/components/update/UpdateDialog.svelte";
  import ResizableSidebar from "$lib/components/ui/ResizableSidebar.svelte";

  const SIDEBAR_AUTO_HIDE_WIDTH = 600; // auto-hide sidebar below this window width
  const SIDEBAR_INITIAL_WIDTH = 240;
  const SIDEBAR_MIN_WIDTH = 200;
  const SIDEBAR_MAX_WIDTH = 300;

  let sidebarWidth = $state(SIDEBAR_INITIAL_WIDTH);
  let isDragging = $state(false);
  let windowWidth = $state(0);
  let autoHidden = $state(false); // hidden by auto-hide, not by the user
  let userOverrideInNarrowMode = $state(false); // user manually opened the sidebar while narrow

  $effect(() => {
    uiState.setSidebarWidth(sidebarWidth);
  });

  function toggleSidebar() {
    uiState.toggleSidebar();
    autoHidden = false; // manual toggle overrides auto-hide

    if (windowWidth < SIDEBAR_AUTO_HIDE_WIDTH && uiState.sidebarOpen) {
      userOverrideInNarrowMode = true;
    }
    else if (windowWidth >= SIDEBAR_AUTO_HIDE_WIDTH || !uiState.sidebarOpen) {
      userOverrideInNarrowMode = false;
    }

    if (browser) {
      localStorage.setItem("sidebar.open", JSON.stringify(uiState.sidebarOpen));
    }
  }

  function handleResize() {
    if (browser) {
      const prevWindowWidth = windowWidth;
      windowWidth = window.innerWidth;

      if (windowWidth < SIDEBAR_AUTO_HIDE_WIDTH) {
        if (uiState.sidebarOpen && !autoHidden && !userOverrideInNarrowMode) {
          uiState.setSidebarOpen(false);
          autoHidden = true;
        }
      } else if (prevWindowWidth < SIDEBAR_AUTO_HIDE_WIDTH) {
        userOverrideInNarrowMode = false;
        if (autoHidden) {
          uiState.setSidebarOpen(true);
          autoHidden = false;
        }
      }
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === "b") {
      event.preventDefault();
      toggleSidebar();
    }
  }

  function restoreSidebarState() {
    if (browser) {
      const saved = localStorage.getItem("sidebar.open");
      if (saved !== null) {
        uiState.setSidebarOpen(JSON.parse(saved));
      }
    }
  }

  onMount(() => {
    restoreSidebarState();

    const savedWidth = localStorage.getItem("main.sidebar.width");
    if (savedWidth) {
      sidebarWidth = parseInt(savedWidth);
    }

    // App updates: load version + prefs, then start cross-window auto-check (main window only)
    let updateUnlisten: (() => void) | null = null;
    updateState
      .load()
      .then(() => updateState.startAutoCheck())
      .then((unlisten) => {
        updateUnlisten = unlisten;
      })
      .catch((error) => {
        console.error("Failed to init update checker:", error);
      });

    // Quick Action handoff: the backend fronts this window and broadcasts
    // `quick-action-open-agent` (payload = session id); navigate to that session
    // whatever the current route. Registered in onMount so events arriving
    // during cold start are not missed.
    let openAgentUnlisten: (() => void) | null = null;
    let openAgentStale = false; // discard a late unlisten if unmount beats listen()
    if (isTauriEnvironment()) {
      listen<string>("quick-action-open-agent", (event) => {
        void goto(`/agent?id=${event.payload}`);
      })
        .then((unlisten) => {
          if (openAgentStale) {
            unlisten();
            return;
          }
          openAgentUnlisten = unlisten;
        })
        .catch((error) => {
          console.error("Failed to listen for quick-action open-agent:", error);
        });
    }

    if (browser) {
      windowWidth = window.innerWidth;
      handleResize();
      window.addEventListener("keydown", handleKeydown);
      window.addEventListener("resize", handleResize);
      return () => {
        window.removeEventListener("keydown", handleKeydown);
        window.removeEventListener("resize", handleResize);
        updateUnlisten?.();
        openAgentStale = true;
        openAgentUnlisten?.();
      };
    }
  });

  let { children } = $props();
</script>

<div class="app">
  <TitleBar
    sidebarOpen={uiState.sidebarOpen}
    showToggleButton={true}
    onToggle={toggleSidebar}
  />

  <div
    class="sidebar-wrapper"
    class:dragging={isDragging}
    class:open={uiState.sidebarOpen}
    style={`width:${uiState.sidebarOpen ? sidebarWidth : 0}px`}
    aria-hidden={!uiState.sidebarOpen}
  >
    <ResizableSidebar
      onResizeStart={() => {
        isDragging = true;
      }}
      onResizing={(w) => {
        sidebarWidth = w;
      }}
      onResizeEnd={(w) => {
        isDragging = false;
        sidebarWidth = w;
      }}
      bind:width={sidebarWidth}
      initialWidth={SIDEBAR_INITIAL_WIDTH}
      minWidth={SIDEBAR_MIN_WIDTH}
      maxWidth={SIDEBAR_MAX_WIDTH}
      storageKey="main.sidebar.width"
      containerClass=""
    >
      <MainSidebar />
    </ResizableSidebar>
  </div>

  <main
    class="main-content"
    class:sidebar-hidden={!uiState.sidebarOpen}
  >
    {@render children()}
  </main>

  <UpdateDialog />
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
    width: 100vw;
    background-color: var(--bg-page);
    color: var(--base-content);
    position: relative;
    overflow: hidden;
    overscroll-behavior: none;
  }

  .sidebar-wrapper {
    flex-shrink: 0;
    min-width: 0;
    transition: width 0s linear, margin var(--dur-base) ease-in-out;
    overflow: hidden;
  }

  /* Top/bottom margin only while open; no left/right margin so the sidebar's
     own horizontal padding stays symmetric against window and content border. */
  .sidebar-wrapper.open {
    margin: 0.5rem 0 0.5rem 0;
  }

  .sidebar-wrapper:not(.dragging) {
    transition: width var(--dur-base) ease-in-out, margin var(--dur-base) ease-in-out;
  }

  .main-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    transition: width var(--dur-base) ease-in-out;
    /* Content card hugs top/right/bottom window edges; hairline + rounded corners on the left */
    background-color: var(--bg-card);
    border-left: 1px solid var(--hairline);
    border-top-left-radius: 0.75rem;
    border-bottom-left-radius: 0.75rem;
  }

  .main-content.sidebar-hidden {
    border-left: 0;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }

  @media (max-width: 768px) {
    .main-content {
      margin-left: 0;
    }
  }

  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }
  :global(body) {
    margin: 0;
    /* font-family inherits from html (Geist Variable) — see app.css */
  }
</style>
