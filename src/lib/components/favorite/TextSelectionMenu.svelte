<script lang="ts">
  import { Copy, Languages, Star, X as CloseIcon } from "@lucide/svelte";
  import type { Snippet } from "svelte";
  import { favoriteStore } from "$lib/states";
  import type { UUID } from "$lib/types";
  import { showAppError } from "$lib/utils/error";
  import { getSelectionTextRange } from "$lib/utils/highlightRange";

  interface Props {
    messageId: UUID;
    chatId: UUID;
    content: string;
    role: 'user' | 'assistant' | 'system';
    children?: Snippet;
  }

  let {
    messageId,
    chatId,
    content,
    role,
    children,
  }: Props = $props();

  let showMenu = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);
  let selectedText = $state("");
  let selectedTextRaw = $state("");
  let selectedRange = $state<{ start: number; end: number } | null>(null);
  let isFavoriting = $state(false);
  let isTranslating = $state(false);
  let showTranslatePanel = $state(false);
  let translatePanelX = $state(0);
  let translatePanelY = $state(0);
  let translateResult = $state<{
    translation: string;
    targetLanguage: string;
    phonetic?: string | null;
    explanation?: string | null;
  } | null>(null);
  let translateError = $state<string | null>(null);
  let container: HTMLDivElement | null = null;

  const TRANSLATE_PANEL_WIDTH = 400;
  const TRANSLATE_PANEL_HEIGHT = 200;

  function clampOverlayPosition(
    x: number,
    y: number,
    width: number,
    height: number,
  ) {
    if (typeof window === "undefined") {
      return { x, y };
    }
    const padding = 12;
    const maxX = Math.max(padding, window.innerWidth - width - padding);
    const maxY = Math.max(padding, window.innerHeight - height - padding);
    return {
      x: Math.min(Math.max(x, padding), maxX),
      y: Math.min(Math.max(y, padding), maxY),
    };
  }

  function resetTranslationState() {
    showTranslatePanel = false;
    translateResult = null;
    translateError = null;
    isTranslating = false;
  }

  function applySelection(selection: Selection) {
    const rawText = selection.toString();
    const trimmed = rawText.trim();
    selectedRange = container
      ? getSelectionTextRange(container, selection)
      : null;
    if (!trimmed || !selectedRange) {
      showMenu = false;
      selectedRange = null;
      selectedTextRaw = "";
      selectedText = "";
      return null;
    }
    selectedTextRaw = rawText;
    selectedText = trimmed;
    return selection.getRangeAt(0).getBoundingClientRect();
  }

  function handleSelection() {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) {
      showMenu = false;
      selectedRange = null;
      resetTranslationState();
      return;
    }

    const rect = applySelection(selection);
    if (!rect) return;

    menuX = rect.left + rect.width / 2;
    menuY = rect.top - 50;
    showMenu = true;
    resetTranslationState();
  }

  function handleContextMenu(event: MouseEvent) {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) return;

    if (!applySelection(selection)) return;

    event.preventDefault();
    menuX = event.clientX;
    menuY = event.clientY;
    showMenu = true;
    resetTranslationState();
  }

  function handleClickOutside(e: MouseEvent) {
    if (e.target instanceof HTMLElement && !e.target.closest(".text-selection-menu")) {
      showMenu = false;
      resetTranslationState();
    }
  }

  async function handleFavoriteText() {
    if (!selectedText) return;

    isFavoriting = true;
    try {
      if (!selectedRange) {
        console.error("Failed to find text range");
        alert("无法在消息内容中找到选中的文本，可能是因为文本格式或编码差异。");
        return;
      }

      await favoriteStore.addTextRange(
        messageId,
        chatId,
        selectedRange,
        role,
        content,
      );
      showMenu = false;
    } catch (error) {
      console.error("Failed to favorite text:", error);
    } finally {
      isFavoriting = false;
    }
  }

  async function handleCopyText() {
    if (!selectedTextRaw) return;
    try {
      await navigator.clipboard.writeText(selectedTextRaw);
      showMenu = false;
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
        showMenu = false;
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

    const { x, y } = clampOverlayPosition(
      menuX - TRANSLATE_PANEL_WIDTH / 2,
      menuY + 36,
      TRANSLATE_PANEL_WIDTH,
      TRANSLATE_PANEL_HEIGHT,
    );
    translatePanelX = x;
    translatePanelY = y;
    showTranslatePanel = true;
    translateResult = null;
    translateError = null;
    isTranslating = true;
    showMenu = false;

    try {
      // 翻译功能已迁移到单词本页面，这里暂时禁用
      translateError = "翻译功能已迁移，请使用单词本页面的查词功能";
    } finally {
      isTranslating = false;
    }
  }

  function closeTranslatePanel() {
    showTranslatePanel = false;
  }
</script>

<svelte:window on:click={handleClickOutside} />

<div class="relative text-selection-menu">
  <div
    class="select-text"
    onmouseup={handleSelection}
    oncontextmenu={handleContextMenu}
    bind:this={container}
    role="textbox"
    aria-label="可选文本"
    tabindex="0"
  >
    {@render children?.()}
  </div>

  {#if showMenu}
    <div
      class="fixed z-[10030] bg-base-100 border border-base-300 rounded-lg shadow-xl px-3 py-2 flex flex-col gap-2"
      style="left: {menuX}px; top: {menuY}px; transform: translateX(-50%);"
      onclick={(event) => event.stopPropagation()}
    >
      <span class="text-xs text-base-content/70 truncate max-w-[240px]">
        {selectedText}
      </span>
      <div class="flex items-center gap-2">
        <button
          class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-base-200 text-base-content hover:bg-base-300"
          onclick={handleCopyText}
        >
          <Copy size={12} />
          复制
        </button>
        <button
          class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-accent text-base-100 hover:bg-accent/90 disabled:opacity-50 disabled:cursor-not-allowed"
          onclick={handleTranslateText}
          disabled={isTranslating}
        >
          {#if isTranslating}
            <div class="w-3 h-3 border border-t-transparent rounded-full animate-spin"></div>
          {:else}
            <Languages size={12} />
            翻译
          {/if}
        </button>
        <button
          class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-primary text-primary-content hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
          onclick={handleFavoriteText}
          disabled={isFavoriting}
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
  {/if}

  {#if showTranslatePanel}
    <div
      class="text-selection-translation fixed z-[10031] bg-base-100 border border-base-300 rounded-xl shadow-xl p-3 w-[400px] max-w-[90vw] h-[200px] flex flex-col"
      style="left: {translatePanelX}px; top: {translatePanelY}px;"
      role="dialog"
      aria-label="翻译结果"
      onclick={(event) => event.stopPropagation()}
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
