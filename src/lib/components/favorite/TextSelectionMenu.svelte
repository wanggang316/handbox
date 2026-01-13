<script lang="ts">
  import { Star } from "@lucide/svelte";
  import { favoriteStore } from "$lib/states";
  import * as favoriteApi from "$lib/api/favorite";
  import type { UUID } from "$lib/types";

  interface Props {
    messageId: UUID;
    chatId: UUID;
    content: string;
    role: 'user' | 'assistant' | 'system';
  }

  let {
    messageId,
    chatId,
    content,
    role,
  }: Props = $props();

  let showMenu = $state(false);
  let menuX = $state(0);
  let menuY = $state(0);
  let selectedText = $state("");
  let isFavoriting = $state(false);

  function handleSelection() {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) {
      showMenu = false;
      return;
    }

    const range = selection.getRangeAt(0);
    const rect = range.getBoundingClientRect();

    selectedText = selection.toString().trim();
    if (selectedText.length > 0) {
      menuX = rect.left + rect.width / 2;
      menuY = rect.top - 50;
      showMenu = true;
    } else {
      showMenu = false;
    }
  }

  function getTextRange(content: string, selectedText: string): { start: number; end: number } | null {
    // 尝试精确匹配
    let index = content.indexOf(selectedText);
    if (index !== -1) {
      return { start: index, end: index + selectedText.length };
    }

    // 尝试去除首尾空格后匹配
    const trimmedSelected = selectedText.trim();
    const trimmedContent = content.trim();
    index = trimmedContent.indexOf(trimmedSelected);
    if (index !== -1) {
      // 计算在原始content中的位置
      const contentStart = content.indexOf(trimmedContent);
      if (contentStart !== -1) {
        return { start: contentStart + index, end: contentStart + index + trimmedSelected.length };
      }
    }

    // 尝试去除所有空白字符后匹配
    const normalizedContent = content.replace(/\s+/g, ' ').trim();
    const normalizedSelected = selectedText.replace(/\s+/g, ' ').trim();
    const normalizedIndex = normalizedContent.indexOf(normalizedSelected);
    if (normalizedIndex !== -1) {
      // 找到第一个非空白的位置
      const firstNonSpace = content.indexOf(normalizedSelected[0]);
      if (firstNonSpace !== -1) {
        return { start: firstNonSpace, end: firstNonSpace + normalizedSelected.length };
      }
    }

    console.warn('TextRange: Could not find selected text in content');
    console.warn('Selected text:', selectedText);
    console.warn('Content:', content);
    return null;
  }

  function handleClickOutside(e: MouseEvent) {
    if (e.target instanceof HTMLElement && !e.target.closest(".text-selection-menu")) {
      showMenu = false;
    }
  }

  async function handleFavoriteText() {
    if (!selectedText) return;

    isFavoriting = true;
    try {
      const textRange = getTextRange(content, selectedText);
      if (!textRange) {
        console.error("Failed to find text range");
        alert("无法在消息内容中找到选中的文本，可能是因为文本格式或编码差异。");
        return;
      }

      await favoriteStore.toggleFavorite(
        messageId,
        chatId,
        JSON.stringify(textRange),
        role,
        "text",
        [],
        undefined,
        content,
        false, // 文本收藏不应该影响消息的收藏按钮状态
      );
      showMenu = false;
    } catch (error) {
      console.error("Failed to favorite text:", error);
    } finally {
      isFavoriting = false;
    }
  }
</script>

<svelte:window on:click={handleClickOutside} />

<div class="relative text-selection-menu">
  <div
    class="select-text"
    onmouseup={handleSelection}
  >
    <slot />
  </div>

  {#if showMenu}
    <div
      class="fixed z-[10030] bg-base-100 border border-base-300 rounded-lg shadow-xl px-3 py-2 flex items-center gap-2"
      style="left: {menuX}px; top: {menuY}px; transform: translateX(-50%);"
    >
      <span class="text-xs text-base-content/70 truncate max-w-[200px]">
        {selectedText}
      </span>
      <button
        class="flex items-center gap-1 px-2 py-1 text-xs rounded bg-primary text-primary-content hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
        onclick={handleFavoriteText}
        disabled={isFavoriting}
      >
        {#if isFavoriting}
          <div class="w-3 h-3 border border-t-transparent rounded-full animate-spin"></div>
        {:else}
          <Star size={12} />
          收藏文字
        {/if}
      </button>
    </div>
  {/if}
</div>
