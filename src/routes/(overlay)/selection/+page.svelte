<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount, tick } from "svelte";
  import {
    ChevronDown,
    Copy,
    Languages,
    MessageCircle,
    MoreVertical,
    Pin,
    RotateCcw,
    Search,
    Star,
    Volume2,
    X as CloseIcon,
    Zap,
  } from "@lucide/svelte";
  import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
  import { openSettingsWindow } from "$lib/api/window";
  import { translateWord } from "$lib/api/word";
  import { favoriteStore } from "$lib/states";
  import { showAppError } from "$lib/utils/error";
  import type {
    CreateExternalFavoriteDto,
    SelectionRect,
  } from "$lib/types/favorite";

  interface SelectionPayload {
    text: string;
    rawText: string;
    rect?: SelectionRect;
    sourceAppName?: string;
    sourceBundleId?: string;
    sourcePid?: number;
    sourceAppPath?: string;
    sourceAppVersion?: string;
    sourceWindowTitle?: string;
    sourceUrl?: string;
    sourceDomain?: string;
    sourceTabTitle?: string;
    captureMethod?: string;
    locale?: string;
    inputLanguage?: string;
  }

  let showMenu = $state(false);
  let selectedText = $state("");
  let selectedTextRaw = $state("");
  let payload = $state<SelectionPayload | null>(null);
  let isLoadingSelection = $state(false);
  let isFavoriting = $state(false);
  let isTranslating = $state(false);
  type PanelMode = "translate" | "ask" | "selection" | "lookup";
  const PANEL_WIDTH = 520;
  const PANEL_MIN_HEIGHT = 220;
  const PANEL_MAX_HEIGHT = 420;

  let showPanel = $state(false);
  let panelMode = $state<PanelMode>("translate");
  let translateResult = $state<{
    translation: string;
    targetLanguage: string;
    phonetic?: string | null;
    explanation?: string | null;
  } | null>(null);
  let translateError = $state<string | null>(null);
  let overlayWebview = $state<ReturnType<typeof getCurrentWebview> | null>(
    null,
  );
  let overlayWindow = $state<ReturnType<typeof getCurrentWebviewWindow> | null>(
    null,
  );
  let isDragging = $state(false);
  let dragPointerId = $state<number | null>(null);
  let dragStart = $state({ x: 0, y: 0 });
  let dragWindowStart = $state({ x: 0, y: 0 });
  let menuContainer = $state<HTMLDivElement | null>(null);
  let panelContainer = $state<HTMLDivElement | null>(null);
  let panelHeader = $state<HTMLDivElement | null>(null);
  let panelBody = $state<HTMLDivElement | null>(null);
  let panelBodyContent = $state<HTMLDivElement | null>(null);
  let panelFooter = $state<HTMLDivElement | null>(null);
  let panelHeight = $state(PANEL_MIN_HEIGHT);
  let unlistenSelectionUpdate: (() => void) | null = null;
  let resizeFrameId = 0;
  let lastWindowSize = { width: 0, height: 0 };
  let resizeObserver: ResizeObserver | null = null;
  const WINDOW_EDGE_PADDING = 12;
  let resizeAnchor = $state<"top" | "bottom">("top");

  const panelModeOptions: { value: PanelMode; label: string }[] = [
    { value: "translate", label: "翻译" },
    { value: "ask", label: "问AI" },
    { value: "selection", label: "选区" },
    { value: "lookup", label: "查询" },
  ];

  const panelModeIcons = {
    translate: Languages,
    ask: MessageCircle,
    selection: Search,
    lookup: Search,
  } as const;
  const ActivePanelIcon = $derived(panelModeIcons[panelMode]);

  onMount(() => {
    overlayWindow = getCurrentWebviewWindow();
    overlayWebview = getCurrentWebview();
    if (overlayWindow) {
      void overlayWindow
        .listen<SelectionPayload>("selection_update", (event) => {
          if (!event.payload) return;
          applySelection(event.payload);
        })
        .then((unlisten) => {
          unlistenSelectionUpdate = unlisten;
        })
        .catch((error) =>
          console.error("Failed to listen for selection updates:", error),
        );
    }
    overlayWindow
      ?.setResizable(true)
      .catch((error) =>
        console.error("Failed to set overlay resizable:", error),
      );
    overlayWindow
      ?.setShadow(false)
      .catch((error) =>
        console.error("Failed to disable overlay shadow:", error),
      );
    overlayWindow
      ?.setMinSize(null)
      .catch((error) =>
        console.error("Failed to clear overlay min size:", error),
      );
    overlayWindow
      ?.setMaxSize(null)
      .catch((error) =>
        console.error("Failed to clear overlay max size:", error),
      );
    scheduleWindowResize();

    setupResizeObserver();
    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        void syncLatestSelection();
        scheduleWindowResize();
      }
    };
    document.addEventListener("visibilitychange", handleVisibility);
    void syncLatestSelection();
    return () => {
      document.removeEventListener("visibilitychange", handleVisibility);
      unlistenSelectionUpdate?.();
      unlistenSelectionUpdate = null;
      resizeObserver?.disconnect();
      resizeObserver = null;
      overlayWebview = null;
      overlayWindow = null;
    };
  });

  function getActiveContainer() {
    if (showPanel) return panelContainer;
    if (showMenu) return menuContainer;
    return null;
  }

  function setupResizeObserver() {
    const container = getActiveContainer();
    resizeObserver?.disconnect();
    resizeObserver = null;
    if (!container || typeof ResizeObserver === "undefined") return;
    resizeObserver = new ResizeObserver(() => {
      scheduleWindowResize();
    });
    resizeObserver.observe(container);
    scheduleWindowResize();
  }

  function scheduleWindowResize() {
    if (typeof window === "undefined" || resizeFrameId) return;
    resizeFrameId = window.requestAnimationFrame(() => {
      resizeFrameId = 0;
      void syncWindowSize();
    });
  }

  function getContainerSize(
    container: HTMLElement,
    options?: { maxHeight?: number; maxWidth?: number },
  ) {
    const rect = container.getBoundingClientRect();
    let width = Math.ceil(Math.max(rect.width, container.scrollWidth));
    let height = Math.ceil(Math.max(rect.height, container.scrollHeight));
    if (options?.maxWidth) {
      width = Math.min(width, options.maxWidth);
    }
    if (options?.maxHeight) {
      height = Math.min(height, options.maxHeight);
    }
    return { width, height };
  }

  function getPanelSize() {
    if (!panelContainer) return null;
    if (!panelHeader || !panelBodyContent || !panelFooter) {
      const fallback = getContainerSize(panelContainer, {
        maxHeight: PANEL_MAX_HEIGHT,
      });
      if (panelHeight !== fallback.height) {
        panelHeight = fallback.height;
      }
      return fallback;
    }

    const headerHeight = panelHeader.getBoundingClientRect().height;
    const footerHeight = panelFooter.getBoundingClientRect().height;
    const contentHeight = panelBodyContent.scrollHeight;
    const minBodyHeight = Math.max(
      PANEL_MIN_HEIGHT - headerHeight - footerHeight,
      0,
    );
    const maxBodyHeight = Math.max(
      PANEL_MAX_HEIGHT - headerHeight - footerHeight,
      minBodyHeight,
    );
    const bodyHeight = Math.min(
      Math.max(contentHeight, minBodyHeight),
      maxBodyHeight,
    );
    const height = Math.ceil(headerHeight + footerHeight + bodyHeight);
    if (panelHeight !== height) {
      panelHeight = height;
    }

    return { width: PANEL_WIDTH, height };
  }

  async function applyOverlaySize(
    width: number,
    height: number,
    anchor: "top" | "bottom" = "top",
  ) {
    overlayWindow ??= getCurrentWebviewWindow();
    overlayWebview ??= getCurrentWebview();
    const windowHandle = overlayWindow;
    if (!windowHandle) {
      console.log("[applyOverlaySize] No window handle");
      return;
    }

    const size = new LogicalSize(width, height);
    try {
      await invoke("selection_overlay_resize", { width, height, anchor });
    } catch (error) {
      console.error("Failed to resize overlay via IPC:", error);
    }
    try {
      await windowHandle.setSize(size);
    } catch (error) {
      console.error("Failed to resize selection overlay:", error);
    }

    if (overlayWebview) {
      try {
        await overlayWebview.setSize(size);
      } catch (error) {
        console.error("Failed to resize selection webview:", error);
      }
    }

    try {
      await clampOverlayPosition(width, height);
    } catch (error) {
      console.error("Failed to clamp overlay position:", error);
    }
  }

  async function syncWindowSize() {
    const container = getActiveContainer();
    if (!container) {
      console.log("[syncWindowSize] No container");
      return;
    }

    await tick();
    let width = 0;
    let height = 0;
    if (showPanel) {
      const panelSize = getPanelSize();
      if (panelSize) {
        ({ width, height } = panelSize);
      }
    }
    if (!width || !height) {
      ({ width, height } = getContainerSize(container, {
        maxHeight: showPanel ? PANEL_MAX_HEIGHT : undefined,
      }));
    }
    console.log("[syncWindowSize] Calculated size:", { width, height });
    if (!width || !height) {
      console.log("[syncWindowSize] Invalid size");
      return;
    }
    if (width === lastWindowSize.width && height === lastWindowSize.height) {
      console.log("[syncWindowSize] Size unchanged, skipping");
      return;
    }
    lastWindowSize = { width, height };

    console.log("[syncWindowSize] Setting window size to:", { width, height });
    await applyOverlaySize(width, height, resizeAnchor);
    console.log("[syncWindowSize] Window resized successfully");
  }

  async function clampOverlayPosition(width: number, height: number) {
    if (typeof window === "undefined") return;
    overlayWindow ??= getCurrentWebviewWindow();
    const windowHandle = overlayWindow;
    if (!windowHandle) return;

    const monitor = await windowHandle.currentMonitor();
    if (!monitor) return;

    const scale = window.devicePixelRatio || 1;
    const nextWidth = Math.round(width * scale);
    const nextHeight = Math.round(height * scale);
    const position = await windowHandle.outerPosition();

    const minX = monitor.position.x + WINDOW_EDGE_PADDING;
    const minY = monitor.position.y + WINDOW_EDGE_PADDING;
    const maxX =
      monitor.position.x + monitor.size.width - nextWidth - WINDOW_EDGE_PADDING;
    const maxY =
      monitor.position.y +
      monitor.size.height -
      nextHeight -
      WINDOW_EDGE_PADDING;

    const clampedX =
      maxX < minX ? minX : Math.min(Math.max(position.x, minX), maxX);
    const clampedY =
      maxY < minY ? minY : Math.min(Math.max(position.y, minY), maxY);

    if (clampedX !== position.x || clampedY !== position.y) {
      await windowHandle.setPosition(new PhysicalPosition(clampedX, clampedY));
    }
  }

  $effect(() => {
    showMenu;
    showPanel;
    panelMode;
    menuContainer;
    panelContainer;
    panelHeader;
    panelBody;
    panelBodyContent;
    panelFooter;
    isTranslating;
    translateResult;
    translateError;
    selectedText;
    selectedTextRaw;
    setupResizeObserver();
    scheduleWindowResize();
  });

  function canStartDrag(event: PointerEvent) {
    if (event.button !== 0) return false;
    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button")) {
      return false;
    }
    return true;
  }

  async function handlePointerDown(event: PointerEvent) {
    if (!canStartDrag(event)) return;
    try {
      overlayWindow ??= getCurrentWebviewWindow();
      const windowHandle = overlayWindow;
      if (!windowHandle) return;

      const position = await windowHandle.outerPosition();
      isDragging = true;
      dragPointerId = event.pointerId;

      const scale = window.devicePixelRatio || 1;
      dragStart = { x: event.screenX * scale, y: event.screenY * scale };
      dragWindowStart = { x: position.x, y: position.y };

      const target = event.currentTarget as HTMLElement | null;
      target?.setPointerCapture(event.pointerId);
      event.preventDefault();
    } catch (error) {
      console.error("Failed to start dragging overlay:", error);
    }
  }

  async function handlePointerMove(event: PointerEvent) {
    if (!isDragging || dragPointerId !== event.pointerId) return;
    try {
      overlayWindow ??= getCurrentWebviewWindow();
      const windowHandle = overlayWindow;
      if (!windowHandle) return;

      const scale = window.devicePixelRatio || 1;
      const deltaX = event.screenX * scale - dragStart.x;
      const deltaY = event.screenY * scale - dragStart.y;
      const nextX = Math.round(dragWindowStart.x + deltaX);
      const nextY = Math.round(dragWindowStart.y + deltaY);
      await windowHandle.setPosition(new PhysicalPosition(nextX, nextY));
    } catch (error) {
      console.error("Failed to drag selection overlay:", error);
    }
  }

  function handlePointerUp(event: PointerEvent) {
    if (dragPointerId !== event.pointerId) return;
    isDragging = false;
    dragPointerId = null;
    const target = event.currentTarget as HTMLElement | null;
    try {
      target?.releasePointerCapture(event.pointerId);
    } catch (error) {
      console.error("Failed to release pointer capture:", error);
    }
  }

  function resetTranslationState() {
    translateResult = null;
    translateError = null;
    isTranslating = false;
  }

  async function openPanel(mode: PanelMode) {
    panelMode = mode;
    showPanel = true;
    showMenu = false;
    panelHeight = PANEL_MIN_HEIGHT;
    resizeAnchor = "bottom";
    await tick();
    scheduleWindowResize();
  }

  async function closePanel() {
    showPanel = false;
    showMenu = true;
    resizeAnchor = "bottom";
    await tick();
    scheduleWindowResize();
  }

  async function hideOverlay() {
    showMenu = false;
    showPanel = false;
    payload = null;
    resetTranslationState();
    try {
      await invoke("selection_overlay_hide");
    } catch (error) {
      console.error("Failed to hide selection overlay via IPC:", error);
      try {
        const currentWindow = getCurrentWebviewWindow();
        await currentWindow.hide();
      } catch (fallbackError) {
        console.error(
          "Failed to hide selection overlay window:",
          fallbackError,
        );
      }
    }
  }

  function applySelection(next: SelectionPayload) {
    console.log("[Selection] applySelection called with:", next);
    const raw = next.rawText || next.text || "";
    const trimmed = raw.trim();
    if (!trimmed) {
      console.log("[Selection] Empty text, hiding overlay");
      hideOverlay();
      return;
    }
    selectedText = trimmed;
    selectedTextRaw = raw;
    payload = next;
    showMenu = true;
    showPanel = false;
    resizeAnchor = "top";
    console.log("[Selection] Menu should be visible now, showMenu=", showMenu);
    resetTranslationState();
  }

  async function loadLatestSelection() {
    if (isLoadingSelection) return;
    isLoadingSelection = true;
    try {
      const value = await invoke<SelectionPayload | null>(
        "selection_get_last_payload",
      );
      console.log("[Selection] Last payload loaded:", value);
      if (value) {
        applySelection(value);
        await openPanel("selection");
      }
    } catch (error) {
      console.error("Failed to load last selection payload:", error);
    } finally {
      isLoadingSelection = false;
    }
  }

  async function syncLatestSelection() {
    if (showMenu || showPanel) return;
    try {
      const value = await invoke<SelectionPayload | null>(
        "selection_get_last_payload",
      );
      if (value) {
        applySelection(value);
      }
    } catch (error) {
      console.error("Failed to sync selection payload:", error);
    }
  }

  async function handleCopyText() {
    if (!selectedTextRaw) return;
    try {
      await navigator.clipboard.writeText(selectedTextRaw);
      await hideOverlay();
    } catch (error) {
      console.error("Failed to copy text:", error);
      const textarea = document.createElement("textarea");
      textarea.value = selectedTextRaw;
      textarea.setAttribute("readonly", "");
      textarea.style.position = "absolute";
      textarea.style.left = "-9999px";
      document.body.appendChild(textarea);
      textarea.select();
      try {
        document.execCommand("copy");
        await hideOverlay();
      } catch (fallbackError) {
        console.error("Fallback copy also failed:", fallbackError);
        showAppError(fallbackError, { fallbackMessage: "复制失败，请重试" });
      } finally {
        document.body.removeChild(textarea);
      }
    }
  }

  async function handleTranslateText() {
    if (!selectedText) return;

    resetTranslationState();
    isTranslating = true;
    await openPanel("translate");

    try {
      const response = await translateWord({ term: selectedText });
      translateResult = response;
      await tick();
      await syncWindowSize();
    } catch (error) {
      console.error("Failed to translate selection:", error);
      const normalized = showAppError(error, { fallbackMessage: "翻译失败" });
      translateError = normalized.message;
      await tick();
      await syncWindowSize();
    } finally {
      isTranslating = false;
    }
  }

  async function handleAskText() {
    if (!selectedText) return;
    resetTranslationState();
    await openPanel("ask");
  }

  async function handleOpenSettings() {
    try {
      await openSettingsWindow("general");
      await hideOverlay();
    } catch (error) {
      console.error("Failed to open settings window:", error);
      showAppError(error, { fallbackMessage: "打开设置失败，请重试" });
    }
  }

  async function handleFavoriteText() {
    if (!selectedText || !payload) return;

    isFavoriting = true;
    try {
      const locale = payload.locale ?? navigator.language;
      const request: CreateExternalFavoriteDto = {
        content: selectedText,
        role: "user",
        selectionTextRaw:
          selectedTextRaw && selectedTextRaw !== selectedText
            ? selectedTextRaw
            : undefined,
        sourceAppName: payload.sourceAppName,
        sourceBundleId: payload.sourceBundleId,
        sourcePid: payload.sourcePid,
        sourceAppPath: payload.sourceAppPath,
        sourceAppVersion: payload.sourceAppVersion,
        sourceWindowTitle: payload.sourceWindowTitle,
        sourceUrl: payload.sourceUrl,
        sourceDomain: payload.sourceDomain,
        sourceTabTitle: payload.sourceTabTitle,
        selectionRect: payload.rect,
        captureMethod: payload.captureMethod,
        locale,
        inputLanguage: payload.inputLanguage,
      };
      await favoriteStore.createExternalFavorite(request);
      await hideOverlay();
    } catch (error) {
      console.error("Failed to favorite text:", error);
      showAppError(error, { fallbackMessage: "收藏失败，请重试" });
    } finally {
      isFavoriting = false;
    }
  }

  function handlePanelModeChange(nextMode: PanelMode) {
    if (panelMode === nextMode) return;
    panelMode = nextMode;
    if (panelMode === "translate" && selectedText) {
      resetTranslationState();
      isTranslating = true;
      void translateWord({ term: selectedText })
        .then((response) => {
          translateResult = response;
        })
        .catch((error) => {
          console.error("Failed to translate selection:", error);
          const normalized = showAppError(error, {
            fallbackMessage: "翻译失败",
          });
          translateError = normalized.message;
        })
        .finally(() => {
          isTranslating = false;
          void tick().then(syncWindowSize);
        });
    } else {
      resetTranslationState();
    }
    scheduleWindowResize();
  }
</script>

<svelte:head>
  <style>
    :global(html),
    :global(body) {
      background: transparent !important;
      height: auto !important;
      min-height: 100% !important;
      overflow: visible !important;
    }
  </style>
</svelte:head>

{#if showMenu}
  <div
    bind:this={menuContainer}
    class="rounded-[18px] bg-white border border-slate-200 px-2.5 py-1 inline-flex w-max flex-nowrap items-center gap-1.5 cursor-grab active:cursor-grabbing"
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerUp}
  >
    <button
      class="inline-flex shrink-0 items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed whitespace-nowrap"
      onclick={loadLatestSelection}
      disabled={isLoadingSelection}
    >
      {#if isLoadingSelection}
        <div
          class="w-3 h-3 border border-t-transparent rounded-full animate-spin"
        ></div>
      {:else}
        <Search size={12} />
        显示
      {/if}
    </button>
    <button
      class="inline-flex shrink-0 items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed whitespace-nowrap"
      onclick={handleCopyText}
      disabled={!selectedText}
    >
      <Copy size={12} />
      复制
    </button>
    <button
      class="inline-flex shrink-0 items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed whitespace-nowrap"
      onclick={handleTranslateText}
      disabled={!selectedText || isTranslating}
    >
      {#if isTranslating}
        <div
          class="w-3 h-3 border border-t-transparent rounded-full animate-spin"
        ></div>
      {:else}
        <Languages size={12} />
        翻译
      {/if}
    </button>
    <button
      class="inline-flex shrink-0 items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed whitespace-nowrap"
      onclick={handleAskText}
      disabled={!selectedText}
    >
      <MessageCircle size={12} />
      问AI
    </button>
    <button
      class="inline-flex shrink-0 items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed whitespace-nowrap"
      onclick={handleFavoriteText}
      disabled={!selectedText || isFavoriting}
    >
      {#if isFavoriting}
        <div
          class="w-3 h-3 border border-t-transparent rounded-full animate-spin"
        ></div>
      {:else}
        <Star size={12} />
        收藏
      {/if}
    </button>
    <button
      class="shrink-0 p-1.5 rounded-full text-slate-600 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300"
      onclick={handleOpenSettings}
      aria-label="打开设置"
    >
      <MoreVertical size={14} />
    </button>
  </div>
{/if}

{#if showPanel}
  <div
    bind:this={panelContainer}
    style={`width: ${PANEL_WIDTH}px; height: ${panelHeight}px;`}
    class="rounded-[24px] bg-white border border-slate-200 px-4 py-3 flex flex-col gap-3 cursor-grab active:cursor-grabbing overflow-hidden"
    role="dialog"
    aria-label="操作面板"
    tabindex="0"
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerUp}
  >
    <div bind:this={panelHeader} class="flex items-center justify-between">
      <div class="flex items-center gap-2 text-slate-700">
        <ActivePanelIcon size={18} />
        <div class="relative">
          <select
            class="appearance-none bg-transparent text-[15px] font-semibold text-slate-800 pr-5 focus:outline-none"
            value={panelMode}
            onchange={(event) =>
              handlePanelModeChange(
                (event.currentTarget as HTMLSelectElement).value as PanelMode,
              )}
          >
            {#each panelModeOptions as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
          <ChevronDown
            size={14}
            class="pointer-events-none absolute right-0 top-1/2 -translate-y-1/2 text-slate-400"
          />
        </div>
      </div>
      <div class="flex items-center gap-2 text-slate-500">
        <button
          class="p-1.5 rounded-lg hover:bg-slate-100"
          aria-label="快捷操作"
        >
          <Zap size={16} />
        </button>
        <button
          class="p-1.5 rounded-lg hover:bg-slate-100"
          aria-label="固定面板"
        >
          <Pin size={16} />
        </button>
        <button
          class="p-1.5 rounded-lg hover:bg-slate-100"
          onclick={closePanel}
          aria-label="关闭面板"
        >
          <CloseIcon size={16} />
        </button>
      </div>
    </div>

    <div bind:this={panelBody} class="flex-1 min-h-0 overflow-auto pr-1">
      <div bind:this={panelBodyContent}>
        {#if panelMode === "translate"}
          <div class="text-[11px] text-slate-500 truncate">
            {selectedText}
          </div>
          <div class="mt-3 text-slate-900 text-lg font-semibold">
            {#if isTranslating}
              <div class="flex items-center gap-2 text-slate-500 text-sm">
                <div
                  class="w-3 h-3 border border-t-transparent rounded-full animate-spin"
                ></div>
                <span>翻译中…</span>
              </div>
            {:else if translateError}
              <div class="text-red-600 text-sm">{translateError}</div>
            {:else if translateResult}
              <div class="text-slate-900 text-2xl font-semibold leading-tight">
                {translateResult.translation}
              </div>
              {#if translateResult.phonetic}
                <div class="mt-2 text-[12px] text-slate-500">
                  [{translateResult.phonetic}]
                </div>
              {/if}
              {#if translateResult.explanation}
                <div class="mt-2 text-[12px] text-slate-500">
                  {translateResult.explanation}
                </div>
              {/if}
            {:else}
              <div class="text-slate-500 text-sm">暂无翻译结果</div>
            {/if}
          </div>
        {:else if panelMode === "ask"}
          <div class="text-[11px] text-slate-500 truncate">
            {selectedText}
          </div>
          <div class="mt-3 text-sm text-slate-500">
            聊天功能暂未接入。
          </div>
        {:else if panelMode === "selection"}
          <div class="text-[11px] text-slate-500 truncate">
            {payload?.sourceAppName ?? "—"} · {payload?.sourceWindowTitle ??
              "—"}
          </div>
          <div class="mt-3 text-sm text-slate-800 whitespace-pre-wrap">
            {selectedTextRaw || selectedText || "暂无内容"}
          </div>
        {:else}
          <div class="text-sm text-slate-500">查询功能暂未接入。</div>
        {/if}
      </div>
    </div>

    <div
      bind:this={panelFooter}
      class="flex items-center justify-between border-t border-slate-200 pt-2 text-slate-500"
    >
      <div class="flex items-center gap-1.5">
        <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="复制">
          <Copy size={14} />
        </button>
        <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="重试">
          <RotateCcw size={14} />
        </button>
        <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="朗读">
          <Volume2 size={14} />
        </button>
      </div>
      {#if panelMode === "translate" && translateResult}
        <div class="text-[11px]">
          目标语言: {translateResult.targetLanguage}
        </div>
      {/if}
    </div>
  </div>
{/if}
