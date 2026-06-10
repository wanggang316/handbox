<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import {
    ChevronRight,
    Copy,
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
  import type { AgentProjectGroup } from "$lib/utils/agentGrouping";
  import { formatRelativeTime } from "$lib/utils/date";
  import type { AgentSession, CreateAgentSessionRequest } from "$lib/types";
  import type { AgentProject } from "$lib/types/agentProject";

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
  // 右键菜单（session 行 / 项目组头）
  // ============================================
  // 统一一个 contextMenu state、按 kind 区分目标：同屏天然只有一个菜单
  // （再次右键直接覆盖旧菜单），项目菜单与 session 菜单天然互斥。
  // 未分组桶组头刻意不挂 oncontextmenu —— 右键它不弹任何自定义菜单
  // （冒泡到 window 只会关闭已开菜单），也绝不会误弹 session 菜单。
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

    // 等输入框挂载后聚焦并全选（input[data-project-id] 定位，
    // 组头按钮此时已被输入行替换，选择器不会撞上按钮）。
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

  // 删除项目：confirm 文案带该组真实 session 数（confirm 前从 store 取
  // 快照）；取消 = 全保留零副作用。确认后 store 联动移除该组会话并清
  // currentSession（后端先 abort 后级联），随后逐会话清运行状态 + 立
  // tombstone（同 session 删除路径，拦截 abort 收尾的迟到流事件）；
  // 若 active session 属于该项目则回 Agent 落地页（/agent 无 id 即落地态）。
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
      `将删除项目“${project.name}”及其 ${memberSessions.length} 个会话，不可恢复。`,
    );
    if (!confirmed) return;

    const containsActive =
      activeId !== "" && memberSessions.some((s) => s.id === activeId);
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
    }
  }

  // 组头整行（文件夹图标 / 名称 / 空白）单击切换折叠；组头上的内嵌控件
  // （hover「+」直建 session 等）标记 data-group-control 即可豁免，
  // 不会误触 toggle。
  function handleGroupHeaderClick(event: MouseEvent, groupId: string) {
    if (
      event.target instanceof Element &&
      event.target.closest("[data-group-control]")
    ) {
      return;
    }
    agentProjectCollapse.toggle(groupId);
  }

  // 组头是 role="button" 的 div（HTML 禁止 button 嵌套，而控件槽里的
  // hover「+」是真按钮）：Enter / Space 保持原生按钮的折叠切换语义；
  // 焦点落在槽内控件上时交还控件自身处理（豁免规则同 click）。
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

  // 顶部「+」建项目 / 组头「+」直建会话共用的失败提示
  // （非阻塞内联错误条，下一次实际尝试时清除）。
  let createErrorMessage = $state<string | null>(null);

  // 「+」：选择项目目录并创建项目（后端为 get-or-create by canonical path）。
  // 取消选择器（返回非 string）= 静默 no-op，不碰任何状态。去重命中与
  // 新建走同一条路：store 按 id 原位替换/插入且保留服务端返回值（不改写
  // 已有显示名/时间戳），随后展开该组并滚动组头进视口。
  async function handleCreateProject() {
    let dir: string | null = null;
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const picked = await openDialog({ directory: true });
      if (typeof picked !== "string") return;
      dir = picked;
    } catch (error) {
      console.error("Failed to open directory picker:", error);
      return;
    }

    createErrorMessage = null;
    try {
      const project = await agentProjectActions.createProject(dir);
      agentProjectCollapse.expand(project.id);
      await tick();
      document
        .querySelector(`[data-project-id="${project.id}"]`)
        ?.scrollIntoView({ block: "nearest" });
    } catch (error) {
      console.error("Failed to create agent project:", error);
      createErrorMessage =
        error instanceof Error ? error.message : "创建项目失败";
    }
  }

  // 组头 hover「+」：零弹窗在该项目下直建「未命名」session。
  //
  // 继承源 = 组内排序首位 session：groupSessions 已按活动键
  // coalesce(lastMessageAt, createdAt) 降序排好，首位即活动键最大者
  // （绝不看 updatedAt）。只复制持久化配置字段 —— modelId+providerId
  // 二元组同源同取、thinkingLevel、enabledTools、systemPrompt、
  // temperature、maxTokens、toolExecutionMode；不带 workingDir（后端以
  // project.path 覆盖），不带任何内容性状态（name/transcript/计数）。
  // 继承源正在流式中也只读 store 里已落库的配置快照，新 session 无任何
  // 运行态（agentRun state 按 session id 隔离）。空项目无继承源 →
  // 配置全部留空走默认，不报错。
  //
  // 成功：createSession 内部 prepend + 设 current，显式展开该组后 goto
  // （折叠组直建也可见，VAL-CREATE-008）。
  // 失败（项目目录已删 VALIDATION_ERROR / 项目刚被删 NOT_FOUND）：走
  // 共用内联错误条、不跳转；store 只在成功后插入，无幽灵行。
  // 连点安全：两次点击各自独立建出两条「未命名」（不去重）；store 在
  // 每次完成时基于最新列表 prepend，两次 await 互不覆盖，currentSession
  // 由最后完成者持有。
  async function handleCreateSessionInProject(
    event: MouseEvent,
    group: AgentProjectGroup,
  ) {
    // 槽内控件已被 handleGroupHeaderClick 豁免；stopPropagation 再加一道
    // 保险，确保不触发折叠切换。
    event.stopPropagation();
    contextMenu = null;
    createErrorMessage = null;

    const source = group.sessions.at(0);
    const request: CreateAgentSessionRequest = {
      name: "未命名",
      projectId: group.project.id,
      modelId: source?.modelId,
      providerId: source?.providerId,
      systemPrompt: source?.systemPrompt,
      thinkingLevel: source?.thinkingLevel,
      temperature: source?.temperature,
      maxTokens: source?.maxTokens,
      enabledTools: source ? [...source.enabledTools] : undefined,
      toolExecutionMode: source?.toolExecutionMode,
    };

    try {
      const session = await agentSessionActions.createSession(request);
      // 显式展开（镜像 handleCreateProject）：直建发生在当前 active 组时，
      // activeGroupId 这个 $derived 重算为同值，Svelte 5 同值不触发下游
      // 反应，上方自动展开的 $effect 不会重跑，必须在此显式 expand。
      agentProjectCollapse.expand(group.project.id);
      goto(`/agent?id=${session.id}`);
    } catch (error) {
      console.error("Failed to create agent session:", error);
      createErrorMessage =
        error instanceof Error ? error.message : "创建会话失败";
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

  <!-- 建项目失败的非阻塞错误条（下一次实际尝试时自动清除） -->
  {#if createErrorMessage}
    <div
      class="mx-2 mb-1 px-2 py-1 rounded-md bg-error/10 text-error text-[12px] leading-[18px] flex-shrink-0"
    >
      {createErrorMessage}
    </div>
  {/if}

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
        {#if renamingProjectId === group.project.id}
          <!-- 项目重命名输入行：替换组头按钮（同 session 重命名的结构），
               输入框包在 data-group-control 豁免区内 —— 即便未来此行恢复
               折叠点击，点击输入框/选中文本也不会触发 toggle（GROUP-025）。 -->
          <div
            class="w-full flex items-center gap-1.5 py-0.5 px-2 text-[12px] leading-[18px]"
          >
            {#if collapsed}
              <Folder size={14} class="flex-shrink-0 text-base-content/60" />
            {:else}
              <FolderOpen
                size={14}
                class="flex-shrink-0 text-base-content/60"
              />
            {/if}
            <span data-group-control class="flex-1 min-w-0">
              <input
                data-project-id={group.project.id}
                class="w-full py-0.5 px-2 text-[12px] bg-base-100 border border-base-300 rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
                bind:value={renameProjectValue}
                onkeydown={handleProjectRenameKeydown}
                onblur={confirmProjectRename}
                placeholder="输入新名称"
              />
            </span>
          </div>
        {:else}
          <!-- 组头宿主是 role="button" 的 div 而非 <button>：控件槽里的
               hover「+」是真按钮，HTML 禁止 button 嵌套。click/Enter/Space
               切换折叠的语义由 handleGroupHeaderClick / Keydown 保持。 -->
          <div
            data-project-id={group.project.id}
            class="group w-full flex items-center gap-1.5 py-0.5 px-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/80 hover:text-base-content hover:bg-base-300 cursor-default select-none"
            role="button"
            tabindex="0"
            aria-expanded={!collapsed}
            onclick={(event) => handleGroupHeaderClick(event, group.project.id)}
            onkeydown={(event) =>
              handleGroupHeaderKeydown(event, group.project.id)}
            oncontextmenu={(event) =>
              handleProjectContextMenu(event, group.project)}
          >
            {#if collapsed}
              <Folder size={14} class="flex-shrink-0 text-base-content/60" />
            {:else}
              <FolderOpen
                size={14}
                class="flex-shrink-0 text-base-content/60"
              />
            {/if}
            <span class="truncate flex-1">{group.project.name}</span>
            <!-- 右侧控件槽：hover「+」直建 session；在 data-group-control
                 豁免区内，点击不会误触折叠。未分组桶组头无此槽。 -->
            <span data-group-control class="flex items-center flex-shrink-0">
              <button
                class="p-0.5 rounded text-base-content/50 opacity-0 group-hover:opacity-100 focus-visible:opacity-100 hover:text-base-content hover:bg-base-content/10 transition-opacity"
                title="新建会话"
                aria-label="在项目 {group.project.name} 中新建会话"
                onclick={(event) =>
                  handleCreateSessionInProject(event, group)}
              >
                <Plus size={14} />
              </button>
            </span>
            <ChevronRight
              size={14}
              class="flex-shrink-0 text-base-content/40 transition-transform {collapsed
                ? ''
                : 'rotate-90'}"
            />
          </div>
        {/if}
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

<!-- 右键菜单（单一 state 按 kind 分发：session 行 / 项目组头互斥） -->
{#if contextMenu?.kind === "project"}
  <div
    class="context-menu fixed z-[10020] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
  >
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={startProjectRename}
    >
      <PencilLine size={14} />
      重命名
    </button>

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-base-100 flex items-center gap-2 whitespace-nowrap"
      onclick={handleCopyProjectPath}
    >
      <Copy size={14} />
      复制路径
    </button>

    <!-- 分隔线 -->
    <div class="border-t border-base-300 my-1 mx-2"></div>
    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-error/10 text-error flex items-center gap-2 whitespace-nowrap"
      onclick={handleProjectDelete}
    >
      <Trash2 size={14} />
      删除项目
    </button>
  </div>
{:else if contextMenu?.kind === "session"}
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
