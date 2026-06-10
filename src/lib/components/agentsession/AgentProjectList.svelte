<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import {
    ChevronRight,
    Folder,
    FolderOpen,
    Hash,
    Inbox,
    PencilLine,
    Plus,
    Trash2,
  } from "@lucide/svelte";
  import {
    agentProjectState,
    agentProjectActions,
  } from "$lib/states/agentProject.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import {
    agentProjectCollapse,
    UNGROUPED_COLLAPSE_KEY,
  } from "$lib/states/agentProjectCollapse.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { groupSessions, sessionActivityKey } from "$lib/utils/agentGrouping";
  import { formatRelativeTime } from "$lib/utils/date";
  import type { AgentSession } from "$lib/types";

  interface Props {
    activeId?: string;
  }

  let { activeId = "" }: Props = $props();

  // 分组与排序完全交给 foundation selectors，组件内不重新实现。
  const grouped = $derived(
    groupSessions(agentProjectState.projects, agentSessionState.sessions),
  );
  const isEmpty = $derived(
    grouped.groups.length === 0 && grouped.ungrouped.length === 0,
  );

  // 初次挂载且 store 无数据时显示加载占位，待两路数据都拉完再渲染，
  // 避免闪现空态或「会话先到、项目未到」造成的未分组桶误现；
  // store 已有数据（模式切换重挂载）则立即渲染并在后台刷新。
  let initialLoadDone = $state(
    agentProjectState.projects.length > 0 ||
      agentSessionState.sessions.length > 0,
  );

  onMount(() => {
    // 每次挂载重拉项目与会话（对齐旧 AgentSessionList 的刷新行为）。
    // 两个 action 内部已记录错误，这里只负责结束加载占位。
    Promise.allSettled([
      agentProjectActions.loadProjects(),
      agentSessionActions.loadSessions(),
    ]).then(() => {
      initialLoadDone = true;
    });
  });

  // active session 所属分组的折叠 key（未分组桶用保留 key；
  // 无 active / 数据未就绪 / 未匹配时为 undefined）。
  const activeGroupId = $derived.by(() => {
    if (!activeId) return undefined;
    for (const group of grouped.groups) {
      if (group.sessions.some((s) => s.id === activeId)) {
        return group.project.id;
      }
    }
    if (grouped.ungrouped.some((s) => s.id === activeId)) {
      return UNGROUPED_COLLAPSE_KEY;
    }
    return undefined;
  });

  // 打开 / 切换到某 session 时自动展开其所属分组（含未分组桶）。
  // 折叠态的读取放进 untrack：本 effect 只跟踪 activeGroupId 的变化，
  // 手动折叠 active 组是合法操作，不会被这里立即弹回。
  $effect(() => {
    const groupId = activeGroupId;
    if (groupId !== undefined) {
      untrack(() => agentProjectCollapse.expand(groupId));
    }
  });

  function handleSessionClick(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }

  // ============================================
  // 右键菜单（session 行）
  // ============================================
  // 统一一个 contextMenu state、按 kind 区分目标：同屏天然只有一个菜单
  // （再次右键直接覆盖旧菜单），后续项目组头菜单以新 kind 加入此联合即可。
  interface SessionContextMenu {
    kind: "session";
    session: AgentSession;
    x: number;
    y: number;
  }
  type ContextMenu = SessionContextMenu;

  let contextMenu = $state<ContextMenu | null>(null);

  function handleSessionContextMenu(event: MouseEvent, session: AgentSession) {
    event.preventDefault();
    // 阻止冒泡到 window 的 oncontextmenu（那里会关掉菜单）。
    event.stopPropagation();
    contextMenu = {
      kind: "session",
      session,
      x: event.clientX,
      y: event.clientY,
    };
  }

  // 点击 / 在菜单外右键时关闭菜单（行上的右键已 stopPropagation，不会误关）。
  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest(".context-menu")) {
      contextMenu = null;
    }
  }

  // ============================================
  // 内联重命名
  // ============================================
  // 输入态按 session id 存（renamingSessionId 定位目标行）：keyed each 重排时
  // 输入框随行移动、内容保留，提交始终写回 renamingSessionId 指向的会话。
  let renamingSessionId = $state("");
  let renameValue = $state("");

  function startRename() {
    if (contextMenu?.kind !== "session") return;
    const session = contextMenu.session;
    renamingSessionId = session.id;
    renameValue = session.name;
    contextMenu = null;

    // 等输入框挂载后聚焦并全选（data-session-id 定位）。
    setTimeout(() => {
      const input = document.querySelector(
        `input[data-session-id="${session.id}"]`,
      ) as HTMLInputElement | null;
      if (input) {
        input.focus();
        input.select();
      }
    }, 0);
  }

  // 确认重命名：纯空白或未变更不写入。先收起输入框再提交，使 Enter 与 blur
  // 的双触发在第二次进入时因 renamingSessionId 已清空而天然幂等。
  // rename 经 store 原位替换且排序键只看消息活动（lastMessageAt/createdAt），
  // 提交后该行保持原位（GROUP-023）。
  async function confirmRename() {
    const id = renamingSessionId;
    const next = renameValue.trim();
    const session = agentSessionState.sessions.find((s) => s.id === id);
    cancelRename();
    if (session && next && next !== session.name) {
      try {
        await agentSessionActions.renameSession(id, next);
      } catch (error) {
        console.error("Failed to rename agent session:", error);
      }
    }
  }

  function cancelRename() {
    renamingSessionId = "";
    renameValue = "";
  }

  function handleRenameKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      confirmRename();
    } else if (event.key === "Escape") {
      cancelRename();
    }
  }

  // ============================================
  // 复制 ID / 删除
  // ============================================
  async function handleCopyId() {
    if (contextMenu?.kind !== "session") return;
    const id = contextMenu.session.id;
    contextMenu = null;
    try {
      await navigator.clipboard.writeText(id);
    } catch (error) {
      console.error("Failed to copy session id:", error);
    }
  }

  // 一键删除，无确认。后端 agent_session_delete 先 abort 再删；删除成功后
  // 清理该会话的运行状态并立 tombstone，拦截 abort 收尾产生的迟到流事件
  // （GROUP-018：不重建已删条目、无 NOT_FOUND console 噪音）。
  async function handleDelete() {
    if (contextMenu?.kind !== "session") {
      contextMenu = null;
      return;
    }
    const target = contextMenu.session;
    contextMenu = null;
    try {
      await agentSessionActions.deleteSession(target.id);
      agentRunStore.removeSession(target.id);
      // 删除的是当前打开的会话则回到 Agent 落地页。
      if (activeId === target.id) {
        goto("/agent");
      }
    } catch (error) {
      console.error("Failed to delete agent session:", error);
    }
  }

  // 组头整行（文件夹图标 / 名称 / 空白）单击切换折叠；组头上的内嵌控件
  // （未来的 hover「+」、右键菜单触发器等）标记 data-group-control 即可
  // 豁免，不会误触 toggle。
  function handleGroupHeaderClick(event: MouseEvent, groupId: string) {
    if (
      event.target instanceof Element &&
      event.target.closest("[data-group-control]")
    ) {
      return;
    }
    agentProjectCollapse.toggle(groupId);
  }

  // 「+」：选择项目目录并创建项目（后端为 get-or-create by canonical path）。
  async function handleCreateProject() {
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const dir = await openDialog({ directory: true });
      if (typeof dir === "string") {
        await agentProjectActions.createProject(dir);
      }
    } catch (error) {
      console.error("Failed to create agent project:", error);
    }
  }
</script>

{#snippet sessionRow(session: AgentSession)}
  {#if renamingSessionId === session.id}
    <!-- 重命名输入框：随 keyed each 行移动；Enter 提交 / blur 提交 / Esc 取消 -->
    <div class="pl-5 pr-2">
      <input
        data-session-id={session.id}
        class="w-full py-0.5 px-2 text-[12px] bg-base-100 border border-base-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
        bind:value={renameValue}
        onkeydown={handleRenameKeydown}
        onblur={confirmRename}
        placeholder="输入新名称"
      />
    </div>
  {:else}
    <button
      class="w-full flex items-center gap-2 py-0.5 pl-7 pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 {session.id ===
      activeId
        ? 'bg-base-300 text-base-content'
        : ''}"
      onclick={() => handleSessionClick(session)}
      oncontextmenu={(event) => handleSessionContextMenu(event, session)}
    >
      <span class="truncate flex-1">{session.name}</span>
      <span class="flex-shrink-0 text-[11px] text-base-content/40">
        {formatRelativeTime(sessionActivityKey(session))}
      </span>
    </button>
  {/if}
{/snippet}

<div class="flex flex-col h-full">
  <!-- 标题 + 新建项目按钮 -->
  <div class="flex items-center justify-between pb-2 pl-4 pr-2 flex-shrink-0">
    <span class="text-sm text-base-content/70">Agent 会话</span>
    <button
      class="p-1 rounded-md text-base-content/60 hover:text-base-content hover:bg-base-300"
      title="选择项目目录"
      aria-label="选择项目目录"
      onclick={handleCreateProject}
    >
      <Plus size={16} />
    </button>
  </div>

  <!-- 项目分组列表 -->
  <div class="flex-1 overflow-y-auto space-y-0.5 px-2">
    {#if !initialLoadDone}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        加载中…
      </div>
    {:else if isEmpty}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        点击 + 选择项目目录开始
      </div>
    {:else}
      {#each grouped.groups as group (group.project.id)}
        {@const collapsed = agentProjectCollapse.isCollapsed(group.project.id)}
        <button
          class="w-full flex items-center gap-1.5 py-0.5 px-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/80 hover:text-base-content hover:bg-base-300"
          aria-expanded={!collapsed}
          onclick={(event) => handleGroupHeaderClick(event, group.project.id)}
        >
          {#if collapsed}
            <Folder size={14} class="flex-shrink-0 text-base-content/60" />
          {:else}
            <FolderOpen size={14} class="flex-shrink-0 text-base-content/60" />
          {/if}
          <span class="truncate flex-1">{group.project.name}</span>
          <ChevronRight
            size={14}
            class="flex-shrink-0 text-base-content/40 transition-transform {collapsed
              ? ''
              : 'rotate-90'}"
          />
        </button>
        {#if !collapsed}
          {#if group.sessions.length === 0}
            <div
              class="pl-7 pr-2 py-0.5 text-[12px] leading-[18px] text-base-content/40"
            >
              No chats
            </div>
          {:else}
            {#each group.sessions as session (session.id)}
              {@render sessionRow(session)}
            {/each}
          {/if}
        {/if}
      {/each}

      <!-- 未分组桶：仅非空时渲染，固定在最底部 -->
      {#if grouped.ungrouped.length > 0}
        {@const ungroupedCollapsed = agentProjectCollapse.isCollapsed(
          UNGROUPED_COLLAPSE_KEY,
        )}
        <button
          class="w-full flex items-center gap-1.5 py-0.5 px-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/80 hover:text-base-content hover:bg-base-300"
          aria-expanded={!ungroupedCollapsed}
          onclick={(event) =>
            handleGroupHeaderClick(event, UNGROUPED_COLLAPSE_KEY)}
        >
          <Inbox size={14} class="flex-shrink-0 text-base-content/60" />
          <span class="truncate flex-1">未分组</span>
          <ChevronRight
            size={14}
            class="flex-shrink-0 text-base-content/40 transition-transform {ungroupedCollapsed
              ? ''
              : 'rotate-90'}"
          />
        </button>
        {#if !ungroupedCollapsed}
          {#each grouped.ungrouped as session (session.id)}
            {@render sessionRow(session)}
          {/each}
        {/if}
      {/if}
    {/if}
  </div>
</div>

<!-- 右键菜单（单一 state 按 kind 分发；当前仅 session 行） -->
{#if contextMenu?.kind === "session"}
  <div
    class="context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
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

<!-- 全局事件监听：点击菜单外 / 在菜单外右键关闭菜单（行上右键已 stopPropagation） -->
<svelte:window onclick={handleClickOutside} oncontextmenu={handleClickOutside} />
