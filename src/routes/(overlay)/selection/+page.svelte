<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { onMount, tick } from "svelte";
  import {
    Copy,
    Languages,
    Search,
    Star,
    X as CloseIcon,
  } from "@lucide/svelte";
  import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/window";
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
  let showSelectionPanel = $state(false);
  let showTranslatePanel = $state(false);
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
  let translatePanelContainer = $state<HTMLDivElement | null>(null);
  let resizeFrameId = 0;
  let lastWindowSize = { width: 0, height: 0 };
  let resizeObserver: ResizeObserver | null = null;
  const WINDOW_EDGE_PADDING = 12;

  onMount(() => {
    overlayWindow = getCurrentWebviewWindow();
    overlayWebview = getCurrentWebview();
    overlayWindow
      ?.setResizable(true)
      .catch((error) =>
        console.error("Failed to set overlay resizable:", error),
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
        scheduleWindowResize();
      }
    };
    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      document.removeEventListener("visibilitychange", handleVisibility);
      resizeObserver?.disconnect();
      resizeObserver = null;
      overlayWebview = null;
      overlayWindow = null;
    };
  });

  function setupResizeObserver() {
    const container = menuContainer;
    if (!container || typeof ResizeObserver === "undefined") return;
    resizeObserver?.disconnect();
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

  function getContainerSize(container: HTMLElement) {
    const rect = container.getBoundingClientRect();
    const width = Math.ceil(
      Math.max(rect.width, container.scrollWidth),
    );
    const height = Math.ceil(
      Math.max(rect.height, container.scrollHeight),
    );
    return { width, height };
  }

  async function applyOverlaySize(width: number, height: number) {
    overlayWindow ??= getCurrentWebviewWindow();
    overlayWebview ??= getCurrentWebview();
    const windowHandle = overlayWindow;
    if (!windowHandle) {
      console.log("[applyOverlaySize] No window handle");
      return;
    }

    const size = new LogicalSize(width, height);
    try {
      await invoke("selection_overlay_resize", { width, height });
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
    const container = menuContainer;
    if (!container) {
      console.log("[syncWindowSize] No container");
      return;
    }
    overlayWindow ??= getCurrentWebviewWindow();
    const windowHandle = overlayWindow;
    if (!windowHandle) {
      console.log("[syncWindowSize] No window handle");
      return;
    }

    await tick();
    const { width, height } = getContainerSize(container);
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
    await applyOverlaySize(width, height);
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
    const maxX = monitor.position.x + monitor.size.width - nextWidth - WINDOW_EDGE_PADDING;
    const maxY = monitor.position.y + monitor.size.height - nextHeight - WINDOW_EDGE_PADDING;

    const clampedX =
      maxX < minX ? minX : Math.min(Math.max(position.x, minX), maxX);
    const clampedY =
      maxY < minY ? minY : Math.min(Math.max(position.y, minY), maxY);

    if (clampedX !== position.x || clampedY !== position.y) {
      await windowHandle.setPosition(new PhysicalPosition(clampedX, clampedY));
    }
  }

  async function syncWindowSizeForTranslatePanel() {
    await tick();
    if (!translatePanelContainer) {
      console.log("[syncWindowSizeForTranslatePanel] No container");
      return;
    }
    const container = menuContainer ?? translatePanelContainer;
    const { width, height } = getContainerSize(container);
    console.log("[syncWindowSizeForTranslatePanel] Container size:", { width, height });

    if (!width || !height) {
      console.log("[syncWindowSizeForTranslatePanel] Invalid size");
      return;
    }

    lastWindowSize = { width, height };

    await applyOverlaySize(width, height);
    console.log("[syncWindowSizeForTranslatePanel] Window resized successfully");
  }

  $effect(() => {
    if (menuContainer && !resizeObserver) {
      setupResizeObserver();
    }
  });

  $effect(() => {
    menuContainer;
    showSelectionPanel;
    showTranslatePanel;
    isTranslating;
    translateResult;
    translateError;
    selectedText;
    selectedTextRaw;
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
    showTranslatePanel = false;
    translateResult = null;
    translateError = null;
    isTranslating = false;
  }

  function resetPanels() {
    showSelectionPanel = false;
    resetTranslationState();
  }

  async function hideOverlay() {
    showMenu = false;
    payload = null;
    resetPanels();
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
    showSelectionPanel = false;
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
        showSelectionPanel = true;
        showTranslatePanel = false;
        await syncWindowSize();
      }
    } catch (error) {
      console.error("Failed to load last selection payload:", error);
    } finally {
      isLoadingSelection = false;
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

    showTranslatePanel = true;
    showSelectionPanel = false;
    translateResult = null;
    translateError = null;
    isTranslating = true;
    await syncWindowSizeForTranslatePanel();

    try {
      const response = await translateWord({ term: selectedText });
      translateResult = response;
      await tick();
      await syncWindowSizeForTranslatePanel();
    } catch (error) {
      console.error("Failed to translate selection:", error);
      const normalized = showAppError(error, { fallbackMessage: "翻译失败" });
      translateError = normalized.message;
      await tick();
      await syncWindowSizeForTranslatePanel();
    } finally {
      isTranslating = false;
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

  function closeTranslatePanel() {
    showTranslatePanel = false;
    if (selectedText) {
      showMenu = true;
    }
    scheduleWindowResize();
  }

  function closeSelectionPanel() {
    showSelectionPanel = false;
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

<div
  bind:this={menuContainer}
  class="w-[360px] max-w-[92vw] rounded-[18px] bg-white px-2.5 py-1 flex flex-col gap-1.5 cursor-grab active:cursor-grabbing"
    onpointerdown={handlePointerDown}
    onpointermove={handlePointerMove}
    onpointerup={handlePointerUp}
    onpointercancel={handlePointerUp}
  >
    <div class="flex items-center gap-1.5">
      <button
        class="inline-flex items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
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
        class="inline-flex items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
        onclick={handleCopyText}
        disabled={!selectedText}
      >
        <Copy size={12} />
        复制
      </button>
      <button
        class="inline-flex items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
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
        class="inline-flex items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
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
    </div>

    {#if showSelectionPanel}
      <div
        class="rounded-2xl bg-slate-50 p-3"
        role="dialog"
        aria-label="选区内容"
        tabindex="0"
      >
        <div class="flex items-center justify-between">
          <div class="text-[11px] uppercase tracking-wide text-slate-500">
            选区内容
          </div>
          <button
            class="p-1 rounded-lg hover:bg-slate-200 text-slate-500 hover:text-slate-800"
            onclick={closeSelectionPanel}
            aria-label="关闭选区内容"
          >
            <CloseIcon size={14} />
          </button>
        </div>
        <div class="mt-1 text-[11px] text-slate-500 truncate">
          {payload?.sourceAppName ?? "—"} · {payload?.sourceWindowTitle ?? "—"}
        </div>
        <div
          class="mt-2 rounded-xl bg-white px-3 py-2 text-xs text-slate-700 min-h-[80px] max-h-40 overflow-auto"
        >
          {selectedTextRaw || selectedText || "暂无内容"}
        </div>
      </div>
    {/if}

    {#if showTranslatePanel}
      <div
        bind:this={translatePanelContainer}
        class="rounded-2xl bg-slate-50 p-3"
        role="dialog"
        aria-label="翻译结果"
        tabindex="0"
      >
        <div class="flex items-center justify-between">
          <div class="text-[11px] uppercase tracking-wide text-slate-500">
            翻译结果
          </div>
          <button
            class="p-1 rounded-lg hover:bg-slate-200 text-slate-500 hover:text-slate-800"
            onclick={closeTranslatePanel}
            aria-label="关闭翻译结果"
          >
            <CloseIcon size={14} />
          </button>
        </div>
        <div class="mt-1 text-[11px] text-slate-500 truncate">
          {selectedText}
        </div>
         <div
           class="mt-2 rounded-xl bg-white px-3 py-2 text-xs text-slate-700 max-h-[400px] overflow-auto"
         >
          {#if isTranslating}
            <div class="flex items-center gap-2 text-slate-500">
              <div
                class="w-3 h-3 border border-t-transparent rounded-full animate-spin"
              ></div>
              <span>翻译中…</span>
            </div>
          {:else if translateError}
            <div class="text-red-600 text-xs">{translateError}</div>
          {:else if translateResult}
            <div class="text-slate-900 text-sm font-semibold">
              {translateResult.translation}
            </div>
            {#if translateResult.phonetic}
              <div class="mt-1 text-[11px] text-slate-500">
                [{translateResult.phonetic}]
              </div>
            {/if}
            {#if translateResult.explanation}
              <div class="mt-1 text-[11px] text-slate-500">
                {translateResult.explanation}
              </div>
            {/if}
            <div class="mt-2 pt-2 border-t border-slate-200 text-[11px] text-slate-500">
              目标语言: {translateResult.targetLanguage}
            </div>
          {:else}
            <div class="text-slate-500 text-xs">暂无翻译结果</div>
          {/if}
        </div>
      </div>
    {/if}
</div>
