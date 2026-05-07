<script lang="ts">
  import {
    PencilLine,
    Trash2,
    Sparkles,
    Copy,
    Hash,
    LoaderCircle,
    Star,
  } from "@lucide/svelte";
  import * as chatApi from "$lib/api/chat";
  import * as messageApi from "$lib/api/message";
  import { favoriteStore } from "$lib/states";

  interface Chat {
    id: string;
    title: string;
  }

  interface Props {
    chats?: Chat[];
    activeId?: string;
    onChatClick?: (chat: Chat) => void;
    onRename?: (chat: Chat, newName: string) => void;
    onDelete?: (chat: Chat) => void;
    onGenerateTitle?: (chat: Chat, newTitle: string) => void;
  }

  let {
    chats = [],
    activeId = "",
    onChatClick = () => {},
    onRename,
    onDelete,
    onGenerateTitle,
  }: Props = $props();

  // 右键菜单状态
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let selectedChat = $state<Chat | null>(null);

  // 重命名状态
  let isRenaming = $state(false);
  let renamingChatId = $state("");
  let renameValue = $state("");

  // 标题生成状态
  let isGeneratingTitle = $state(false);
  let generatingChatId = $state("");

  // 收藏状态
  let isFavoriting = $state(false);
  let favoritingChatId = $state("");

  // 处理右键点击
  function handleContextMenu(event: MouseEvent, chat: Chat) {
    if (!onRename && !onDelete) return;

    event.preventDefault();
    event.stopPropagation();

    selectedChat = chat;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    showContextMenu = true;
  }

  // 开始重命名
  function startRename() {
    if (!selectedChat) return;

    isRenaming = true;
    renamingChatId = selectedChat.id;
    renameValue = selectedChat.title;
    showContextMenu = false;

    // 在下一个 tick 中聚焦输入框
    setTimeout(() => {
      if (!selectedChat) return;
      const input = document.querySelector(
        `input[data-chat-id="${selectedChat.id}"]`
      ) as HTMLInputElement;
      if (input) {
        input.focus();
        input.select();
      }
    }, 0);
  }

  // 确认重命名
  function confirmRename() {
    const chat = chats.find((c) => c.id === renamingChatId);
    if (
      chat &&
      onRename &&
      renameValue.trim() &&
      renameValue.trim() !== chat.title
    ) {
      onRename(chat, renameValue.trim());
    }
    cancelRename();
  }

  // 取消重命名
  function cancelRename() {
    isRenaming = false;
    renamingChatId = "";
    renameValue = "";
  }

  // 处理删除
  function handleDelete() {
    if (selectedChat && onDelete) {
      onDelete(selectedChat);
    }
    showContextMenu = false;
  }

  // 生成标题
  async function handleGenerateTitle() {
    if (!selectedChat) return;

    showContextMenu = false;

    // 设置 loading 状态
    isGeneratingTitle = true;
    generatingChatId = selectedChat.id;

    try {
      const response = await chatApi.generateChatTitle(selectedChat.id);
      const generatedTitle = response.title.trim();

      if (generatedTitle && onGenerateTitle) {
        onGenerateTitle(selectedChat, generatedTitle);
      }
    } catch (error) {
      console.error("Failed to generate title:", error);
    } finally {
      // 清除 loading 状态
      isGeneratingTitle = false;
      generatingChatId = "";
    }
  }

  // 复制标题
  async function handleCopyTitle() {
    if (!selectedChat) return;

    try {
      await navigator.clipboard.writeText(selectedChat.title);
      console.log("Title copied to clipboard");
    } catch (error) {
      console.error("Failed to copy title:", error);
    }
    showContextMenu = false;
  }

  // 复制聊天ID
  async function handleCopyId() {
    if (!selectedChat) return;

    try {
      await navigator.clipboard.writeText(selectedChat.id);
      console.log("Chat ID copied to clipboard");
    } catch (error) {
      console.error("Failed to copy chat ID:", error);
    }
    showContextMenu = false;
  }

  // 键盘事件处理
  function handleKeydown(event: KeyboardEvent) {
    if (isRenaming) {
      if (event.key === "Enter") {
        confirmRename();
      } else if (event.key === "Escape") {
        cancelRename();
      }
    }
  }

  // 收藏对话
  async function handleFavoriteChat() {
    if (!selectedChat) return;

    showContextMenu = false;
    isFavoriting = true;
    favoritingChatId = selectedChat.id;

    try {
      const messages = await messageApi.getMessages(selectedChat.id, 1, 0);
      if (messages.length > 0) {
        const message = messages[0];
        await favoriteStore.toggleFavorite(
          message.id ?? "",
          selectedChat.id,
          selectedChat.title,
          message.role ?? "assistant",
          "chat",
          [],
          undefined,
          undefined,
        );
      }
    } catch (error) {
      console.error("Failed to favorite chat:", error);
    } finally {
      isFavoriting = false;
      favoritingChatId = "";
    }
  }

  // 点击外部关闭菜单
  function handleClickOutside(event: MouseEvent) {
    // 检查点击是否在右键菜单外部
    const target = event.target as HTMLElement;
    if (!target.closest(".context-menu")) {
      showContextMenu = false;
    }
  }

  // 处理聊天点击
  function handleChatClick(chat: Chat) {
    if (!isRenaming || renamingChatId !== chat.id) {
      onChatClick(chat);
    }
  }
</script>

<div class="flex flex-col h-full">
  <!-- 标题 -->
  <div class="text-sm text-base-content/70 pb-2 pl-4 flex-shrink-0">聊天</div>

  <!-- 聊天列表 -->
  <div class="flex-1 overflow-y-auto space-y-0.5 px-2">
    {#each chats as chat (chat.id)}
      {#if isRenaming && renamingChatId === chat.id}
        <!-- 重命名输入框 -->
        <div class="relative">
          <input
            data-chat-id={chat.id}
            class="w-full py-0.5 px-2 text-[12px] bg-base-100 border border-base-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
            bind:value={renameValue}
            onkeydown={handleKeydown}
            onblur={confirmRename}
            placeholder="输入新名称"
          />
        </div>
      {:else}
        <!-- 聊天项 -->
        <button
          class="w-full py-0.5 px-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 {chat.id ===
          activeId
            ? 'bg-base-300 text-base-content'
            : ''}"
          onclick={() => handleChatClick(chat)}
          oncontextmenu={(e) => handleContextMenu(e, chat)}
        >
          <div class="flex items-center justify-between">
            <span class="truncate">{chat.title}</span>
            {#if isGeneratingTitle && generatingChatId === chat.id}
              <LoaderCircle
                size={12}
                class="text-base-content/60 animate-spin flex-shrink-0 ml-2"
              />
            {/if}
          </div>
        </button>
      {/if}
    {/each}
  </div>
</div>

<!-- 右键菜单 -->
{#if showContextMenu}
  <div
    class="context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
  >
    {#if onGenerateTitle}
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
        onclick={handleGenerateTitle}
      >
        <Sparkles size={14} />
        生成标题
      </button>
    {/if}

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={handleFavoriteChat}
      disabled={isFavoriting || !selectedChat}
    >
      {#if isFavoriting && selectedChat && favoritingChatId === selectedChat.id}
        <LoaderCircle size={14} class="animate-spin" />
      {:else}
        <Star size={14} />
      {/if}
      收藏对话
    </button>

    {#if onRename}
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
        onclick={startRename}
      >
        <PencilLine size={14} />
        重命名
      </button>
    {/if}

    <!-- 分隔线 -->
    {#if (onGenerateTitle || onRename) && onDelete}
      <div class="border-t border-base-300 my-1 mx-2"></div>
    {/if}

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={handleCopyTitle}
    >
      <Copy size={14} />
      复制标题
    </button>

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={handleCopyId}
    >
      <Hash size={14} />
      复制ID
    </button>

    {#if onDelete}
      <!-- 分隔线 -->
      <div class="border-t border-base-300 my-1 mx-2"></div>
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
        onclick={handleDelete}
      >
        <Trash2 size={14} />
        删除
      </button>
    {/if}
  </div>
{/if}

<!-- 全局事件监听 -->
<svelte:window onclick={handleClickOutside} />
