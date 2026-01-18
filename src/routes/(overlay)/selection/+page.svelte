<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { Copy, Languages, Star, X as CloseIcon } from "@lucide/svelte";
  import { translateWord } from "$lib/api/word";
  import { favoriteStore } from "$lib/states";
  import { showAppError } from "$lib/utils/error";
  import type { CreateExternalFavoriteDto, SelectionRect } from "$lib/types/favorite";

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
  let showTranslatePanel = $state(false);
  let translateResult = $state<{
    translation: string;
    targetLanguage: string;
    phonetic?: string | null;
    explanation?: string | null;
  } | null>(null);
  let translateError = $state<string | null>(null);

  function resetTranslationState() {
    showTranslatePanel = false;
    translateResult = null;
    translateError = null;
    isTranslating = false;
  }

  async function hideOverlay() {
    showMenu = false;
    payload = null;
    resetTranslationState();
    try {
      const currentWindow = getCurrentWebviewWindow();
      await currentWindow.hide();
      // 不要激活其他窗口,保持当前焦点在原应用
    } catch (error) {
      console.error("Failed to hide selection overlay:", error);
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
    console.log("[Selection] Menu should be visible now, showMenu=", showMenu);
    resetTranslationState();
  }

  async function loadLatestSelection() {
    if (isLoadingSelection) return;
    isLoadingSelection = true;
    try {
      const value = await invoke<SelectionPayload | null>("selection_get_last_payload");
      console.log("[Selection] Last payload loaded:", value);
      if (value) {
        applySelection(value);
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
    translateResult = null;
    translateError = null;
    isTranslating = true;
    showMenu = false;

    try {
      const response = await translateWord({ term: selectedText });
      translateResult = response;
    } catch (error) {
      console.error("Failed to translate selection:", error);
      const normalized = showAppError(error, { fallbackMessage: "翻译失败" });
      translateError = normalized.message;
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
  }

</script>

<svelte:head>
  <style>
    :global(body) {
      background: red !important;
      opacity: 1 !important;
    }
  </style>
</svelte:head>

<div class="w-full h-full p-2 bg-red-500">
  <!-- 调试信息 -->
  <div class="text-white text-xs mb-2 bg-black p-1 rounded">
    showMenu: {showMenu}<br>
    selectedText: "{selectedText || 'empty'}"<br>
    selectedTextRaw: "{selectedTextRaw || 'empty'}"
  </div>

  <div
    class="pointer-events-auto bg-yellow-400 border-4 border-black rounded-lg shadow-xl px-3 py-2 flex flex-col gap-2 w-[260px]"
    role="menu"
    aria-label="划词操作"
    tabindex="0"
  >
    <span class="text-xs text-black font-bold truncate max-w-[240px]">
      {selectedText || '等待选择文字...'}
    </span>
    <div class="flex items-center gap-2">
      <button
        class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-white text-black hover:bg-gray-200 border border-gray-300 disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={loadLatestSelection}
        disabled={isLoadingSelection}
      >
        {#if isLoadingSelection}
          <div class="w-3 h-3 border border-t-transparent rounded-full animate-spin"></div>
        {:else}
          显示
        {/if}
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-white text-black hover:bg-gray-200 border border-gray-300 disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={handleCopyText}
        disabled={!selectedText}
      >
        <Copy size={12} />
        复制
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-blue-500 text-white hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={handleTranslateText}
        disabled={!selectedText || isTranslating}
      >
        {#if isTranslating}
          <div class="w-3 h-3 border border-t-transparent rounded-full animate-spin"></div>
        {:else}
          <Languages size={12} />
          翻译
        {/if}
      </button>
      <button
        class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-green-500 text-white hover:bg-green-600 disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={handleFavoriteText}
        disabled={!selectedText || isFavoriting}
      >
        {#if isFavoriting}
          <div class="w-3 h-3 border border-t-transparent rounded-full animate-spin"></div>
        {:else}
          <Star size={12} />
          收藏
        {/if}
      </button>
    </div>
  </div>

  {#if showTranslatePanel}
    <div
      class="pointer-events-auto mt-2 bg-base-100 border border-base-300 rounded-xl shadow-xl p-3 w-[400px] max-w-[90vw] h-[200px] flex flex-col"
      role="dialog"
      aria-label="翻译结果"
      tabindex="0"
    >
      <div class="flex items-center justify-between">
        <div class="text-xs text-base-content/60">翻译结果</div>
        <button
          class="p-1 rounded hover:bg-base-200 text-base-content/60 hover:text-base-content"
          onclick={closeTranslatePanel}
          aria-label="关闭翻译结果"
        >
          <CloseIcon size={14} />
        </button>
      </div>
      <div class="mt-1 text-xs text-base-content/70 truncate">
        {selectedText}
      </div>
      <div class="mt-2 flex-1 min-h-0 rounded-lg bg-base-200 px-3 py-2 text-sm overflow-auto">
        {#if isTranslating}
          <div class="flex items-center gap-2 text-base-content/60">
            <div class="w-3 h-3 border border-t-transparent rounded-full animate-spin"></div>
            <span>翻译中…</span>
          </div>
        {:else if translateError}
          <div class="text-error text-sm">{translateError}</div>
        {:else if translateResult}
          <div class="text-base-content text-sm font-medium">
            {translateResult.translation}
          </div>
          {#if translateResult.phonetic}
            <div class="mt-1 text-xs text-base-content/70">
              [{translateResult.phonetic}]
            </div>
          {/if}
          {#if translateResult.explanation}
            <div class="mt-1 text-xs text-base-content/60">
              {translateResult.explanation}
            </div>
          {/if}
        {:else}
          <div class="text-base-content/60 text-sm">暂无翻译结果</div>
        {/if}
      </div>
      <div class="mt-1 text-[11px] text-base-content/50">
        目标语言:
        {translateResult?.targetLanguage ?? "—"}
      </div>
    </div>
  {/if}
</div>
