<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";
  import {
    Copy,
    Languages,
    MessageCircle,
    MoreVertical,
    Search,
    Star,
  } from "@lucide/svelte";
  import { openSettingsWindow } from "$lib/api/window";
  import { translateWord } from "$lib/api/word";
  import { favoriteStore } from "$lib/states";
  import { showAppError } from "$lib/utils/error";
  import type { CreateExternalFavoriteDto } from "$lib/types/favorite";

  interface SelectionPayload {
    text: string;
    rawText: string;
    rect?: {
      x: number;
      y: number;
      width: number;
      height: number;
    };
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

  let selectedText = $state("");
  let selectedTextRaw = $state("");
  let payload = $state<SelectionPayload | null>(null);
  let isFavoriting = $state(false);
  let isTranslating = $state(false);
  let overlayWebview = $state<ReturnType<typeof getCurrentWebview> | null>(
    null,
  );
  let unlistenSelectionUpdate: (() => void) | null = null;

  onMount(() => {
    overlayWebview = getCurrentWebview();

    if (overlayWebview) {
      void overlayWebview
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

    return () => {
      unlistenSelectionUpdate?.();
      unlistenSelectionUpdate = null;
      overlayWebview = null;
    };
  });

  function applySelection(next: SelectionPayload) {
    const raw = next.rawText || next.text || "";
    const trimmed = raw.trim();
    if (!trimmed) {
      selectedText = "";
      selectedTextRaw = "";
      payload = null;
      return;
    }
    selectedText = trimmed;
    selectedTextRaw = raw;
    payload = next;
  }

  async function handleCopyText() {
    if (!selectedTextRaw) return;
    try {
      await navigator.clipboard.writeText(selectedTextRaw);
      await hidePanel();
    } catch (error) {
      console.error("Failed to copy text:", error);
      showAppError(error, { fallbackMessage: "复制失败，请重试" });
    }
  }

  async function handleTranslateText() {
    if (!selectedText || isTranslating) return;
    isTranslating = true;
    try {
      // 通知后端切换到翻译面板
      await invoke("selection_show_action_panel", {
        mode: "translate",
        text: selectedText,
      });
    } catch (error) {
      console.error("Failed to show translate panel:", error);
      showAppError(error, { fallbackMessage: "打开翻译面板失败" });
    } finally {
      isTranslating = false;
    }
  }

  async function handleAskText() {
    if (!selectedText) return;
    try {
      await invoke("selection_show_action_panel", {
        mode: "ask",
        text: selectedText,
      });
    } catch (error) {
      console.error("Failed to show ask panel:", error);
      showAppError(error, { fallbackMessage: "打开问AI面板失败" });
    }
  }

  async function handleShowSelection() {
    if (!selectedText) return;
    try {
      await invoke("selection_show_action_panel", {
        mode: "selection",
        text: selectedText,
      });
    } catch (error) {
      console.error("Failed to show selection panel:", error);
      showAppError(error, { fallbackMessage: "打开选区面板失败" });
    }
  }

  async function handleOpenSettings() {
    try {
      await openSettingsWindow("general");
      await hidePanel();
    } catch (error) {
      console.error("Failed to open settings window:", error);
      showAppError(error, { fallbackMessage: "打开设置失败，请重试" });
    }
  }

  async function handleFavoriteText() {
    if (!selectedText || !payload || isFavoriting) return;

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
      await hidePanel();
    } catch (error) {
      console.error("Failed to favorite text:", error);
      showAppError(error, { fallbackMessage: "收藏失败，请重试" });
    } finally {
      isFavoriting = false;
    }
  }

  async function hidePanel() {
    try {
      await invoke("selection_hide_menu_panel");
    } catch (error) {
      console.error("Failed to hide menu panel:", error);
    }
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
  class="rounded-[18px] bg-white border border-slate-200 px-2.5 py-1 inline-flex w-max flex-nowrap items-center gap-1.5"
>
  <button
    class="inline-flex shrink-0 items-center gap-1.5 rounded-[12px] px-2 py-1 text-[12px] font-medium text-slate-700 transition hover:bg-slate-100 hover:text-slate-900 active:bg-slate-200 focus-visible:outline focus-visible:outline-2 focus-visible:outline-slate-300 cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed whitespace-nowrap"
    onclick={handleShowSelection}
    disabled={!selectedText}
  >
    <Search size={12} />
    显示
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
