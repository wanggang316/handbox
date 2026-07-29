<script lang="ts">
  import { untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { goto } from "$app/navigation";
  import {
    ChevronRight,
    Copy,
    Folder,
    FolderOpen,
    Loader2,
    MessagesSquare,
    PencilLine,
    Plus,
    Sparkles,
    Trash2,
  } from "@lucide/svelte";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import {
    agentProjectState,
    agentProjectActions,
  } from "$lib/states/agentProject.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { agentProjectCollapse } from "$lib/states/agentProjectCollapse.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { t } from "$lib/i18n";
  import {
    groupSessionsByAgent,
    sessionActivityKey,
  } from "$lib/utils/agentGrouping";
  import type { AgentSessionBucket } from "$lib/utils/agentGrouping";
  import { formatRelativeTime } from "$lib/utils/date";
  import { normalizeError } from "$lib/utils/error";
  import { onMount } from "svelte";
  import type { AgentSession } from "$lib/types";
  import type { AgentProject } from "$lib/types/agentProject";

  interface Props {
    activeId?: string;
  }

  let { activeId = "" }: Props = $props();

  // 分组与排序完全交给 foundation selector（Agent → Project → Session），
  // 组件内不重新实现。
  const buckets = $derived(
    groupSessionsByAgent(
      agentState.agents,
      agentProjectState.projects,
      agentSessionState.sessions,
    ),
  );
  const isEmpty = $derived(buckets.length === 0);

  // 项目子组折叠 key：同一 Project 可能挂在多个 Agent 桶下，需按 桶+项目 复合
  // 记忆折叠态，避免在 A 桶折叠 X 项目会连带折叠 B 桶下的 X。
  function projectCollapseKey(bucketKey: string, projectId: string): string {
    return `${bucketKey}::${projectId}`;
  }

  // 初次挂载且 store 无数据时显示加载占位，待三路数据都拉完再渲染，
  // 避免闪现空态或「会话先到、Agent/项目未到」造成的误归桶；
  // store 已有数据（模式切换重挂载）则立即渲染并在后台刷新。
  let initialLoadDone = $state(
    agentProjectState.projects.length > 0 ||
      agentSessionState.sessions.length > 0,
  );

  // 任一路加载失败即置位：Agent / projects 拉取失败而 sessions 成功时若照常渲染，
  // 会话会被错误归入「Chats」桶（伪呈现），故失败时不进入分组渲染，改显示
  // 错误条 + 重试。
  let loadError = $state(false);

  // 每次挂载重拉 Agent / 项目 / 会话，保证侧栏数据新鲜（重试按钮复用同一逻辑）。
  // 各 action 内部已记录错误，这里捕获 settled 结果用于失败可见化。
  async function loadSidebarData() {
    const results = await Promise.allSettled([
      agentActions.loadAgents(),
      agentProjectActions.loadProjects(),
      agentSessionActions.loadSessions(),
    ]);
    loadError = results.some((result) => result.status === "rejected");
    initialLoadDone = true;
  }

  onMount(() => {
    loadSidebarData();
  });

  // active session 所在位置（桶 key + 可选项目折叠 key）；无 active / 数据未就绪 /
  // 未匹配时为 undefined。
  const activeLocation = $derived.by(() => {
    if (!activeId) return undefined;
    for (const bucket of buckets) {
      for (const child of bucket.children) {
        if (child.kind === "session" && child.session.id === activeId) {
          return { bucketKey: bucket.key, projectKey: undefined };
        }
        if (
          child.kind === "project" &&
          child.sessions.some((s) => s.id === activeId)
        ) {
          return {
            bucketKey: bucket.key,
            projectKey: projectCollapseKey(bucket.key, child.project.id),
          };
        }
      }
    }
    return undefined;
  });

  // 打开 / 切换到某 session 时自动展开其所属桶与项目子组。
  // 折叠态的读取放进 untrack：本 effect 只跟踪 activeLocation 的变化，
  // 手动折叠 active 组是合法操作，不会被这里立即弹回。
  $effect(() => {
    const loc = activeLocation;
    if (loc) {
      untrack(() => {
        agentProjectCollapse.expand(loc.bucketKey);
        if (loc.projectKey) agentProjectCollapse.expand(loc.projectKey);
      });
    }
  });

  function handleSessionClick(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }

  // ============================================
  // 右键菜单（session 行 / 项目组头）
  // ============================================
  // 统一一个 contextMenu state、按 kind 区分目标：同屏天然只有一个菜单
  // （再次右键直接覆盖旧菜单），项目菜单与 session 菜单天然互斥。
  interface SessionContextMenu {
    kind: "session";
    session: AgentSession;
    x: number;
    y: number;
  }
  interface ProjectContextMenu {
    kind: "project";
    project: AgentProject;
    x: number;
    y: number;
  }
  type ContextMenu = SessionContextMenu | ProjectContextMenu;

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

  function handleProjectContextMenu(event: MouseEvent, project: AgentProject) {
    event.preventDefault();
    event.stopPropagation();
    contextMenu = {
      kind: "project",
      project,
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
  // 内联重命名（session）
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
  // 生成标题（右键手动触发）
  // ============================================
  // 正在生成标题的 session id：该会话行以 spinner 替换相对时间做进行中反馈。
  let generatingTitleId = $state<string | null>(null);

  async function handleGenerateTitle() {
    if (contextMenu?.kind !== "session") return;
    const session = contextMenu.session;
    contextMenu = null;
    createErrorMessage = null;
    generatingTitleId = session.id;
    try {
      await agentSessionActions.generateTitle(session.id);
    } catch (error) {
      console.error("Failed to generate session title:", error);
      const normalized = normalizeError(
        error,
        t("agent.list.generateTitleFailed"),
      );
      // 展示具体 message（真实原因），而非通用 hint —— 否则「应用内部错误，请
      // 重新启动应用」这类兜底 hint 会遮盖掉实际失败原因。
      createErrorMessage = `${t("agent.list.generateTitleFailed")}: ${normalized.message}`;
    } finally {
      generatingTitleId = null;
    }
  }

  // ============================================
  // 复制 ID / 删除（session）
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
  // 清理该会话的运行状态并立 tombstone，拦截 abort 收尾产生的迟到流事件。
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

  // ============================================
  // 项目重命名（组头内联输入框）
  // ============================================
  // 与 session 重命名同构：按 project id 存输入态，keyed each 重排时输入框
  // 随组头移动、提交始终写回 renamingProjectId 指向的项目。
  let renamingProjectId = $state("");
  let renameProjectValue = $state("");

  function startProjectRename() {
    if (contextMenu?.kind !== "project") return;
    const project = contextMenu.project;
    renamingProjectId = project.id;
    renameProjectValue = project.name;
    contextMenu = null;

    setTimeout(() => {
      const input = document.querySelector(
        `input[data-project-id="${project.id}"]`,
      ) as HTMLInputElement | null;
      if (input) {
        input.focus();
        input.select();
      }
    }, 0);
  }

  // 语义对齐 session 重命名：Enter 提交 / 含变更失焦提交 / Esc 取消 /
  // 纯空白或未变更不写入。先收起输入框再提交，Enter 与 blur 双触发幂等。
  async function confirmProjectRename() {
    const id = renamingProjectId;
    const next = renameProjectValue.trim();
    const project = agentProjectState.projects.find((p) => p.id === id);
    cancelProjectRename();
    if (project && next && next !== project.name) {
      try {
        await agentProjectActions.renameProject(id, next);
      } catch (error) {
        console.error("Failed to rename agent project:", error);
      }
    }
  }

  function cancelProjectRename() {
    renamingProjectId = "";
    renameProjectValue = "";
  }

  function handleProjectRenameKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      confirmProjectRename();
    } else if (event.key === "Escape") {
      cancelProjectRename();
    }
  }

  // ============================================
  // 项目复制路径 / 删除
  // ============================================
  async function handleCopyProjectPath() {
    if (contextMenu?.kind !== "project") return;
    const path = contextMenu.project.path;
    contextMenu = null;
    try {
      await navigator.clipboard.writeText(path);
    } catch (error) {
      console.error("Failed to copy project path:", error);
    }
  }

  // 原生 confirm（对齐 states/auth.svelte.ts 的动态 import + 浏览器兜底）。
  async function confirmNative(message: string): Promise<boolean> {
    try {
      const { confirm } = await import("@tauri-apps/plugin-dialog");
      return await confirm(message);
    } catch (error) {
      console.warn("Native confirm unavailable, falling back:", error);
      return window.confirm(message);
    }
  }

  // 删除项目：confirm 文案带该项目真实 session 数（confirm 前从 store 取快照，
  // 跨所有 Agent 统计）；取消 = 全保留零副作用。确认后 store 联动移除该项目会话
  // 并清 currentSession（后端先 abort 后级联），随后逐会话清运行状态 + 立
  // tombstone；若 active session 属于该项目则回 Agent 落地页。
  async function handleProjectDelete() {
    if (contextMenu?.kind !== "project") {
      contextMenu = null;
      return;
    }
    const project = contextMenu.project;
    contextMenu = null;

    const memberSessions = agentSessionState.sessions.filter(
      (session) => session.projectId === project.id,
    );
    const confirmed = await confirmNative(
      t("agent.list.deleteProjectConfirm", {
        name: project.name,
        count: memberSessions.length,
      }),
    );
    if (!confirmed) return;

    const containsActive =
      activeId !== "" && memberSessions.some((s) => s.id === activeId);
    createErrorMessage = null;
    try {
      await agentProjectActions.deleteProject(project.id);
      for (const session of memberSessions) {
        agentRunStore.removeSession(session.id);
      }
      if (containsActive) {
        goto("/agent");
      }
    } catch (error) {
      console.error("Failed to delete agent project:", error);
      const normalized = normalizeError(
        error,
        t("agent.list.deleteProjectFailed"),
      );
      createErrorMessage = normalized.hint ?? normalized.message;
    }
  }

  // 组头整行单击切换折叠；组头上的内嵌控件（hover「+」直建 session 等）
  // 标记 data-group-control 即可豁免，不会误触 toggle。
  function handleGroupHeaderClick(event: MouseEvent, groupId: string) {
    if (
      event.target instanceof Element &&
      event.target.closest("[data-group-control]")
    ) {
      return;
    }
    agentProjectCollapse.toggle(groupId);
  }

  // 组头是 role="button" 的 div（HTML 禁止 button 嵌套，而控件槽里的 hover「+」
  // 是真按钮）：Enter / Space 保持折叠切换语义；焦点落在槽内控件上时交还控件
  // 自身处理（豁免规则同 click）。
  function handleGroupHeaderKeydown(event: KeyboardEvent, groupId: string) {
    if (event.key !== "Enter" && event.key !== " ") return;
    if (
      event.target instanceof Element &&
      event.target.closest("[data-group-control]")
    ) {
      return;
    }
    event.preventDefault();
    agentProjectCollapse.toggle(groupId);
  }

  // 直建会话失败 / 删除项目失败共用的非阻塞内联错误条（优先展示 AppError 的 hint，
  // 下一次实际尝试时清除）。
  let createErrorMessage = $state<string | null>(null);

  // Agent 组头 hover「+」：以该 Agent 定义直建一个会话（无项目）。
  // createSessionFromDefinition 由后端按 definition 裁决能力集 / 工作目录策略。
  async function handleCreateSessionForAgent(
    event: MouseEvent,
    bucket: AgentSessionBucket,
  ) {
    event.stopPropagation();
    const agentId = bucket.agent?.id;
    if (!agentId) return; // 「Chats」桶无来源 Agent，不提供直建入口。
    contextMenu = null;
    createErrorMessage = null;
    try {
      const session =
        await agentSessionActions.createSessionFromDefinition(agentId);
      agentProjectCollapse.expand(bucket.key);
      goto(`/agent?id=${session.id}`);
    } catch (error) {
      console.error("Failed to create agent session:", error);
      const normalized = normalizeError(
        error,
        t("agent.list.createSessionFailed"),
      );
      createErrorMessage = normalized.hint ?? normalized.message;
    }
  }

  // Agent > 项目子组 hover「+」：以该 Agent 定义直建一个挂到该项目的会话
  // （agentDefinitionId + projectId 同时归属）。工作目录由后端以 project.path 覆盖。
  async function handleCreateSessionInProject(
    event: MouseEvent,
    bucket: AgentSessionBucket,
    project: AgentProject,
  ) {
    event.stopPropagation();
    const agentId = bucket.agent?.id;
    if (!agentId) return;
    contextMenu = null;
    createErrorMessage = null;
    try {
      const session = await agentSessionActions.createSessionFromDefinition(
        agentId,
        { projectId: project.id },
      );
      agentProjectCollapse.expand(bucket.key);
      agentProjectCollapse.expand(projectCollapseKey(bucket.key, project.id));
      goto(`/agent?id=${session.id}`);
    } catch (error) {
      console.error("Failed to create agent session in project:", error);
      const normalized = normalizeError(
        error,
        t("agent.list.createSessionFailed"),
      );
      createErrorMessage = normalized.hint ?? normalized.message;
    }
  }
</script>

{#snippet sessionRow(
  session: AgentSession,
  rowIndent: string,
  inputIndent: string,
)}
  {#if renamingSessionId === session.id}
    <!-- 重命名输入框：随 keyed each 行移动；Enter 提交 / blur 提交 / Esc 取消 -->
    <div class="{inputIndent} pr-2">
      <input
        data-session-id={session.id}
        class="w-full py-0.5 px-2 text-[12px] bg-base-100 border border-base-300 rounded-md"
        bind:value={renameValue}
        onkeydown={handleRenameKeydown}
        onblur={confirmRename}
        placeholder={t("agent.list.renamePlaceholder")}
      />
    </div>
  {:else}
    <button
      class="w-full flex items-center gap-2 py-1 {rowIndent} pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content hover:bg-base-300 {session.id ===
      activeId
        ? 'bg-base-300 text-base-content'
        : ''}"
      onclick={() => handleSessionClick(session)}
      oncontextmenu={(event) => handleSessionContextMenu(event, session)}
    >
      <span class="truncate flex-1">{session.name}</span>
      {#if generatingTitleId === session.id}
        <Loader2
          size={12}
          class="flex-shrink-0 animate-spin text-base-content/40"
        />
      {:else}
        <span class="flex-shrink-0 text-[11px] text-base-content/55">
          {formatRelativeTime(sessionActivityKey(session))}
        </span>
      {/if}
    </button>
  {/if}
{/snippet}

<!-- 项目子组：组头（可折叠）+ 会话列表；宿主桶通过 bucket 传入以支持直建归属。 -->
{#snippet projectGroup(
  bucket: AgentSessionBucket,
  project: AgentProject,
  sessions: AgentSession[],
)}
  {@const key = projectCollapseKey(bucket.key, project.id)}
  {@const collapsed = agentProjectCollapse.isCollapsed(key)}
  {#if renamingProjectId === project.id}
    <!-- 项目重命名输入行：替换组头按钮，输入框包在 data-group-control 豁免区内。 -->
    <div class="w-full flex items-center gap-1.5 py-1 pl-7 pr-2 text-[12px] leading-[18px]">
      {#if collapsed}
        <Folder size={14} class="flex-shrink-0 text-base-content/60" />
      {:else}
        <FolderOpen size={14} class="flex-shrink-0 text-base-content/60" />
      {/if}
      <span data-group-control class="flex-1 min-w-0">
        <input
          data-project-id={project.id}
          class="w-full py-0.5 px-2 text-[12px] bg-base-100 border border-base-300 rounded-md"
          bind:value={renameProjectValue}
          onkeydown={handleProjectRenameKeydown}
          onblur={confirmProjectRename}
          placeholder={t("agent.list.renamePlaceholder")}
        />
      </span>
    </div>
  {:else}
    <div
      data-project-id={project.id}
      class="group/proj w-full flex items-center gap-1.5 py-1 pl-7 pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 cursor-default select-none"
      role="button"
      tabindex="0"
      aria-expanded={!collapsed}
      onclick={(event) => handleGroupHeaderClick(event, key)}
      onkeydown={(event) => handleGroupHeaderKeydown(event, key)}
      oncontextmenu={(event) => handleProjectContextMenu(event, project)}
    >
      {#if collapsed}
        <Folder size={14} class="flex-shrink-0 text-base-content/60" />
      {:else}
        <FolderOpen size={14} class="flex-shrink-0 text-base-content/60" />
      {/if}
      <span class="truncate flex-1">{project.name}</span>
      <!-- 右侧控件槽：hover「+」直建该 Agent + 项目的 session。 -->
      <span data-group-control class="flex items-center flex-shrink-0">
        <button
          class="p-0.5 rounded text-base-content/50 opacity-0 group-hover/proj:opacity-100 focus-visible:opacity-100 hover:text-base-content hover:bg-base-content/10 transition-opacity"
          title={t("agent.list.newSession")}
          aria-label={t("agent.list.newSessionInProject", {
            name: project.name,
          })}
          onclick={(event) => handleCreateSessionInProject(event, bucket, project)}
        >
          <Plus size={14} />
        </button>
      </span>
      <ChevronRight
        size={14}
        class="flex-shrink-0 text-base-content/40 transition-transform duration-[var(--dur-fast)] {collapsed
          ? ''
          : 'rotate-90'}"
      />
    </div>
  {/if}
  {#if !collapsed}
    <div class="space-y-0.5" transition:slide={{ duration: 160 }}>
      {#each sessions as session (session.id)}
        {@render sessionRow(session, "pl-12", "pl-10")}
      {/each}
    </div>
  {/if}
{/snippet}

<div class="flex flex-col h-full">
  <!-- 直建会话 / 删除项目失败的非阻塞错误条（下一次实际尝试时自动清除） -->
  {#if createErrorMessage}
    <div
      class="mx-2 mt-2 mb-1 px-2 py-1 rounded-md bg-error/10 text-error text-[12px] leading-[18px] flex-shrink-0"
    >
      {createErrorMessage}
    </div>
  {/if}

  <!-- Agent 分组列表（Agent → Project → Session；无来源 Agent 归入垫底的 Chats 桶）。
       组间用 space-y-1.5 分隔、组内紧凑，形成清晰的分组节奏。 -->
  <div class="flex-1 overflow-y-auto space-y-1.5 px-2 pt-2">
    {#if !initialLoadDone}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        {t("common.loading")}
      </div>
    {:else if loadError}
      <!-- 部分加载失败：不进入分组渲染（避免会话被伪归入「Chats」桶） -->
      <div class="px-2 py-1 text-[12px] leading-[18px] text-error">
        {t("agent.list.loadFailed")}
      </div>
      <button
        class="mx-2 px-2 py-0.5 rounded-md text-[12px] leading-[18px] border border-base-300 text-base-content/70 hover:text-base-content hover:bg-base-300"
        onclick={loadSidebarData}
      >
        {t("common.retry")}
      </button>
    {:else if isEmpty}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        {t("agent.list.emptyHint")}
      </div>
    {:else}
      {#each buckets as bucket (bucket.key)}
        {@const collapsed = agentProjectCollapse.isCollapsed(bucket.key)}
        <!-- 一个 Agent 分组（组头 + 子节点）作为一个整体，组内 space-y-0.5 紧凑排布。 -->
        <div class="space-y-0.5">
          <!-- 桶组头：Agent（Bot 图标 + 名称 + hover「+」直建）或 Chats（MessagesSquare，无直建）。 -->
          <div
            class="group/bucket w-full flex items-center gap-1.5 py-1 pl-2 pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 cursor-default select-none"
            role="button"
            tabindex="0"
            aria-expanded={!collapsed}
            onclick={(event) => handleGroupHeaderClick(event, bucket.key)}
            onkeydown={(event) => handleGroupHeaderKeydown(event, bucket.key)}
          >
            {#if bucket.agent}
              {@const BucketIcon = resolveAgentIcon(bucket.agent.icon)}
              <BucketIcon size={14} class="flex-shrink-0 text-base-content/60" />
              <span class="truncate flex-1">{bucket.agent.name}</span>
              <span data-group-control class="flex items-center flex-shrink-0">
                <button
                  class="p-0.5 rounded text-base-content/50 opacity-0 group-hover/bucket:opacity-100 focus-visible:opacity-100 hover:text-base-content hover:bg-base-content/10 transition-opacity"
                  title={t("agent.list.newSession")}
                  aria-label={t("agent.list.newSession")}
                  onclick={(event) => handleCreateSessionForAgent(event, bucket)}
                >
                  <Plus size={14} />
                </button>
              </span>
            {:else}
              <MessagesSquare
                size={14}
                class="flex-shrink-0 text-base-content/60"
              />
              <span class="truncate flex-1">{t("agent.list.ungrouped")}</span>
            {/if}
            <ChevronRight
              size={14}
              class="flex-shrink-0 text-base-content/40 transition-transform duration-[var(--dur-fast)] {collapsed
                ? ''
                : 'rotate-90'}"
            />
          </div>
          {#if !collapsed}
            <div class="space-y-0.5" transition:slide={{ duration: 160 }}>
              {#each bucket.children as child (child.kind === "project" ? `p:${child.project.id}` : `s:${child.session.id}`)}
                {#if child.kind === "project"}
                  {@render projectGroup(bucket, child.project, child.sessions)}
                {:else}
                  {@render sessionRow(child.session, "pl-7", "pl-5")}
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

<!-- 右键菜单（单一 state 按 kind 分发：session 行 / 项目组头互斥） -->
{#if contextMenu?.kind === "project"}
  <div
    class="context-menu fixed z-[var(--z-dropdown)] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
  >
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
      onclick={startProjectRename}
    >
      <PencilLine size={14} />
      {t("common.rename")}
    </button>

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
      onclick={handleCopyProjectPath}
    >
      <Copy size={14} />
      {t("agent.list.copyPath")}
    </button>

    <!-- 分隔线 -->
    <div class="border-t border-base-300 my-1 mx-2"></div>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
      onclick={handleProjectDelete}
    >
      <Trash2 size={14} />
      {t("agent.list.deleteProject")}
    </button>
  </div>
{:else if contextMenu?.kind === "session"}
  <div
    class="context-menu fixed z-[var(--z-dropdown)] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
  >
    <!-- 生成标题：仅当会话已有消息（有内容可蒸馏）时提供 -->
    {#if contextMenu.session.messageCount > 0}
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
        onclick={handleGenerateTitle}
      >
        <Sparkles size={14} />
        {t("ui.generateTitle")}
      </button>
    {/if}

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
      onclick={startRename}
    >
      <PencilLine size={14} />
      {t("common.rename")}
    </button>

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
      onclick={handleCopyId}
    >
      <Copy size={14} />
      {t("agent.list.copyId")}
    </button>

    <!-- 分隔线 -->
    <div class="border-t border-base-300 my-1 mx-2"></div>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
      onclick={handleDelete}
    >
      <Trash2 size={14} />
      {t("common.delete")}
    </button>
  </div>
{/if}

<!-- 全局事件监听：点击菜单外 / 在菜单外右键关闭菜单（行上右键已 stopPropagation） -->
<svelte:window onclick={handleClickOutside} oncontextmenu={handleClickOutside} />
