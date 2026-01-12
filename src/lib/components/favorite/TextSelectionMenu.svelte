<script lang="ts">
  import { Star } from "@lucide/svelte";
  import { favoriteStore } from "$lib/states";
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

  function handleClickOutside(e: MouseEvent) {
    if (e.target instanceof HTMLElement && !e.target.closest(".text-selection-menu")) {
      showMenu = false;
    }
  }

  async function handleFavoriteText() {
    if (!selectedText) return;

    isFavoriting = true;
    try {
      await favoriteStore.toggleFavorite(
        messageId,
        chatId,
        content,
        role,
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
