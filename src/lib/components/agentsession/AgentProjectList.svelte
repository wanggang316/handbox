<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    ChevronRight,
    Folder,
    FolderOpen,
    Inbox,
    Plus,
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

  function handleSessionClick(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
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
  <button
    class="w-full flex items-center gap-2 py-0.5 pl-7 pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 {session.id ===
    activeId
      ? 'bg-base-300 text-base-content'
      : ''}"
    onclick={() => handleSessionClick(session)}
  >
    <span class="truncate flex-1">{session.name}</span>
    <span class="flex-shrink-0 text-[11px] text-base-content/40">
      {formatRelativeTime(sessionActivityKey(session))}
    </span>
  </button>
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
          onclick={() => agentProjectCollapse.toggle(group.project.id)}
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
          onclick={() => agentProjectCollapse.toggle(UNGROUPED_COLLAPSE_KEY)}
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
