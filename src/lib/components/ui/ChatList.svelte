<script lang="ts">
  import { PencilLine, Trash2 } from '@lucide/svelte';

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
  }

  let {
    chats = [],
    activeId = "",
    onChatClick = () => {},
    onRename,
    onDelete
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
      const input = document.querySelector(`input[data-chat-id="${selectedChat.id}"]`) as HTMLInputElement;
      if (input) {
        input.focus();
        input.select();
      }
    }, 0);
  }

  // 确认重命名
  function confirmRename() {
    const chat = chats.find(c => c.id === renamingChatId);
    if (chat && onRename && renameValue.trim() && renameValue.trim() !== chat.title) {
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

  // 键盘事件处理
  function handleKeydown(event: KeyboardEvent) {
    if (isRenaming) {
      if (event.key === 'Enter') {
        confirmRename();
      } else if (event.key === 'Escape') {
        cancelRename();
      }
    }
  }

  // 点击外部关闭菜单
  function handleClickOutside(event: MouseEvent) {
    // 检查点击是否在右键菜单外部
    const target = event.target as HTMLElement;
    if (!target.closest('.context-menu')) {
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
  <div class="text-sm text-gray-600 pb-2 pl-4 flex-shrink-0">聊天</div>

  <!-- 聊天列表 -->
  <div class="flex-1 overflow-y-auto space-y-1 px-2">
    {#each chats as chat (chat.id)}
      {#if isRenaming && renamingChatId === chat.id}
        <!-- 重命名输入框 -->
        <div class="relative">
          <input
            data-chat-id={chat.id}
            class="w-full p-2 text-[14px] bg-white border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
            bind:value={renameValue}
            onkeydown={handleKeydown}
            onblur={confirmRename}
            placeholder="输入新名称"
          />
        </div>
      {:else}
        <!-- 聊天项 -->
        <button
          class="w-full p-2 text-left rounded-lg text-[14px] leading-[22px] text-gray-700 hover:bg-bg-hover truncate {chat.id === activeId ? 'bg-bg-hover' : ''}"
          onclick={() => handleChatClick(chat)}
          oncontextmenu={(e) => handleContextMenu(e, chat)}
        >
          <span class="truncate">{chat.title}</span>
        </button>
      {/if}
    {/each}
  </div>
</div>

<!-- 右键菜单 -->
{#if showContextMenu}
  <div
    class="context-menu fixed z-[10020] bg-white border border-[#e5e5e5] rounded-xl shadow-xl px-1 py-1 min-w-32"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
  >
    {#if onRename}
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-bg-accent hover:text-text-accent flex items-center gap-2 whitespace-nowrap"
        onclick={startRename}
      >
        <PencilLine size={14} />
        重命名
      </button>
    {/if}
    {#if onDelete}
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-red-50 text-red-600 flex items-center gap-2 whitespace-nowrap"
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