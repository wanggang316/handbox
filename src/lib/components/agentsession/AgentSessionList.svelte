<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Plus, PencilLine, Trash2, Hash } from "@lucide/svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import AgentSessionCreateModal from "$lib/components/agentsession/AgentSessionCreateModal.svelte";
  import type { AgentSession } from "$lib/types";

  interface Props {
    activeId?: string;
  }

  let { activeId = "" }: Props = $props();

  // 列表与加载状态直接来自 store。
  const sessions = $derived(agentSessionState.sessions);
  const isLoading = $derived(agentSessionState.isLoading);

  let showCreateModal = $state(false);

  // 右键菜单状态
  let showContextMenu = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);
  let selectedSession = $state<AgentSession | null>(null);

  // 重命名状态
  let isRenaming = $state(false);
  let renamingSessionId = $state("");
  let renameValue = $state("");

  onMount(() => {
    agentSessionActions.loadSessions().catch((error) => {
      console.error("Failed to load agent sessions:", error);
    });
  });

  function handleSessionClick(session: AgentSession) {
    if (isRenaming && renamingSessionId === session.id) {
      return;
    }
    goto(`/agent?id=${session.id}`);
  }

  // 处理右键点击
  function handleContextMenu(event: MouseEvent, session: AgentSession) {
    event.preventDefault();
    event.stopPropagation();

    selectedSession = session;
    contextMenuX = event.clientX;
    contextMenuY = event.clientY;
    showContextMenu = true;
  }

  // 开始重命名
  function startRename() {
    if (!selectedSession) return;

    isRenaming = true;
    renamingSessionId = selectedSession.id;
    renameValue = selectedSession.name;
    showContextMenu = false;

    setTimeout(() => {
      if (!selectedSession) return;
      const input = document.querySelector(
        `input[data-session-id="${selectedSession.id}"]`,
      ) as HTMLInputElement;
      if (input) {
        input.focus();
        input.select();
      }
    }, 0);
  }

  // 确认重命名：空白或未变更则不写入。
  async function confirmRename() {
    const session = sessions.find((s) => s.id === renamingSessionId);
    const next = renameValue.trim();
    if (session && next && next !== session.name) {
      try {
        await agentSessionActions.renameSession(session.id, next);
      } catch (error) {
        console.error("Failed to rename agent session:", error);
      }
    }
    cancelRename();
  }

  // 取消重命名
  function cancelRename() {
    isRenaming = false;
    renamingSessionId = "";
    renameValue = "";
  }

  // 处理删除：一键删除，无确认弹窗（与 ChatList 行为一致）。
  async function handleDelete() {
    if (!selectedSession) {
      showContextMenu = false;
      return;
    }
    const target = selectedSession;
    showContextMenu = false;
    try {
      await agentSessionActions.deleteSession(target.id);
      // 删除的是当前打开的会话则回到 Agent 落地页。
      if (activeId === target.id) {
        goto("/agent");
      }
    } catch (error) {
      console.error("Failed to delete agent session:", error);
    }
  }

  // 复制会话 ID
  async function handleCopyId() {
    if (!selectedSession) return;
    try {
      await navigator.clipboard.writeText(selectedSession.id);
    } catch (error) {
      console.error("Failed to copy session id:", error);
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

  // 点击外部关闭菜单
  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest(".context-menu")) {
      showContextMenu = false;
    }
  }

  function handleCreated(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }
</script>

<div class="flex flex-col h-full">
  <!-- 标题 + 新建按钮 -->
  <div
    class="flex items-center justify-between pb-2 pl-4 pr-2 flex-shrink-0"
  >
    <span class="text-sm text-base-content/70">Agent 会话</span>
    <button
      class="p-1 rounded-md text-base-content/60 hover:text-base-content hover:bg-base-300"
      title="新建 Agent 会话"
      aria-label="新建 Agent 会话"
      onclick={() => (showCreateModal = true)}
    >
      <Plus size={16} />
    </button>
  </div>

  <!-- 会话列表 -->
  <div class="flex-1 overflow-y-auto space-y-0.5 px-2">
    {#if isLoading && sessions.length === 0}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        加载中…
      </div>
    {:else if sessions.length === 0}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        还没有 Agent 会话
      </div>
    {:else}
      {#each sessions as session (session.id)}
        {#if isRenaming && renamingSessionId === session.id}
          <!-- 重命名输入框 -->
          <div class="relative">
            <input
              data-session-id={session.id}
              class="w-full py-0.5 px-2 text-[12px] bg-base-100 border border-base-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
              bind:value={renameValue}
              onkeydown={handleKeydown}
              onblur={confirmRename}
              placeholder="输入新名称"
            />
          </div>
        {:else}
          <button
            class="w-full py-0.5 px-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 {session.id ===
            activeId
              ? 'bg-base-300 text-base-content'
              : ''}"
            onclick={() => handleSessionClick(session)}
            oncontextmenu={(e) => handleContextMenu(e, session)}
          >
            <span class="truncate block">{session.name}</span>
          </button>
        {/if}
      {/each}
    {/if}
  </div>
</div>

<!-- 右键菜单 -->
{#if showContextMenu}
  <div
    class="context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenuX}px; top: {contextMenuY}px;"
  >
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={startRename}
    >
      <PencilLine size={14} />
      重命名
    </button>

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={handleCopyId}
    >
      <Hash size={14} />
      复制ID
    </button>

    <!-- 分隔线 -->
    <div class="border-t border-base-300 my-1 mx-2"></div>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
      onclick={handleDelete}
    >
      <Trash2 size={14} />
      删除
    </button>
  </div>
{/if}

<AgentSessionCreateModal bind:open={showCreateModal} onCreated={handleCreated} />

<!-- 全局事件监听 -->
<svelte:window onclick={handleClickOutside} />
