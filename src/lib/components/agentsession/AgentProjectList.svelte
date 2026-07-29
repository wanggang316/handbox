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

  // Grouping and ordering (Agent → Project → Session) are fully delegated to
  // the selector; the component does not re-implement them.
  const buckets = $derived(
    groupSessionsByAgent(
      agentState.agents,
      agentProjectState.projects,
      agentSessionState.sessions,
    ),
  );
  const isEmpty = $derived(buckets.length === 0);

  // A project can appear under multiple agent buckets, so collapse state is
  // keyed by bucket+project to keep each occurrence independent.
  function projectCollapseKey(bucketKey: string, projectId: string): string {
    return `${bucketKey}::${projectId}`;
  }

  // Show a loading placeholder until all three fetches settle, to avoid a
  // flash of empty state or mis-bucketing when sessions arrive before agents/
  // projects. A warm store renders immediately and refreshes in the background.
  let initialLoadDone = $state(
    agentProjectState.projects.length > 0 ||
      agentSessionState.sessions.length > 0,
  );

  // Set when any fetch fails: rendering with sessions but without agents/
  // projects would mis-bucket them into "Chats", so show an error bar with
  // retry instead of the grouped list.
  let loadError = $state(false);

  // Refetch agents/projects/sessions on every mount (retry reuses this).
  // Actions log their own errors; settled results here drive failure visibility.
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

  // Location of the active session (bucket key + optional project collapse
  // key); undefined when there is no active session or no match.
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

  // Auto-expand the active session's bucket and project group. Collapse reads
  // go through untrack: the effect only tracks activeLocation, so manually
  // collapsing the active group is not immediately reverted.
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

  // One contextMenu state discriminated by kind: only one menu can be on
  // screen (a new right-click overwrites the old), so session and project
  // menus are mutually exclusive by construction.
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
    // Don't bubble to the window oncontextmenu, which would close the menu.
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

  // Close the menu on click or right-click outside it (row right-clicks stopPropagation).
  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest(".context-menu")) {
      contextMenu = null;
    }
  }

  // Inline rename state is keyed by session id: the input follows its row
  // through keyed-each reorders and commits always target renamingSessionId.
  let renamingSessionId = $state("");
  let renameValue = $state("");

  function startRename() {
    if (contextMenu?.kind !== "session") return;
    const session = contextMenu.session;
    renamingSessionId = session.id;
    renameValue = session.name;
    contextMenu = null;

    // Focus and select once the input mounts (located via data-session-id).
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

  // Whitespace-only or unchanged names are not written. Clearing the input
  // state before committing makes the Enter + blur double-fire idempotent.
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

  // Session id whose title is being generated; its row swaps the relative time
  // for a spinner as progress feedback.
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
      // Show the concrete message, not the generic hint, which would mask the
      // actual failure reason.
      createErrorMessage = `${t("agent.list.generateTitleFailed")}: ${normalized.message}`;
    } finally {
      generatingTitleId = null;
    }
  }

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

  // One-click delete, no confirmation. The backend aborts before deleting; on
  // success the run state is cleared and a tombstone set to swallow late
  // stream events from the abort teardown.
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
      // Deleting the open session returns to the Agent landing page.
      if (activeId === target.id) {
        goto("/agent");
      }
    } catch (error) {
      console.error("Failed to delete agent session:", error);
    }
  }

  // Project rename mirrors session rename: state keyed by project id so the
  // input follows its header through reorders and commits target renamingProjectId.
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

  // Same semantics as session rename: whitespace-only or unchanged names are
  // not written; clearing state first makes the Enter + blur double-fire idempotent.
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

  // Native confirm via dynamic import, with a browser confirm fallback.
  async function confirmNative(message: string): Promise<boolean> {
    try {
      const { confirm } = await import("@tauri-apps/plugin-dialog");
      return await confirm(message);
    } catch (error) {
      console.warn("Native confirm unavailable, falling back:", error);
      return window.confirm(message);
    }
  }

  // Delete project: the confirm text carries the real member-session count
  // (snapshotted across all agents before confirming); cancel has zero side
  // effects. On confirm the backend aborts then cascades, the store removes
  // member sessions, per-session run state is cleared with tombstones, and an
  // active member session navigates back to the landing page.
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

  // Clicking a group header toggles collapse; embedded controls marked with
  // data-group-control are exempt from the toggle.
  function handleGroupHeaderClick(event: MouseEvent, groupId: string) {
    if (
      event.target instanceof Element &&
      event.target.closest("[data-group-control]")
    ) {
      return;
    }
    agentProjectCollapse.toggle(groupId);
  }

  // The header is a role="button" div because HTML forbids nesting the real
  // "+" button inside a <button>. Enter/Space toggle collapse; focus on slot
  // controls defers to them (same exemption as click).
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

  // Non-blocking inline error bar shared by session-create and project-delete
  // failures (prefers the AppError hint; cleared on the next attempt).
  let createErrorMessage = $state<string | null>(null);

  // Bucket-header "+": create a session (no project) from the agent definition;
  // the backend resolves capabilities and working-dir policy from it.
  async function handleCreateSessionForAgent(
    event: MouseEvent,
    bucket: AgentSessionBucket,
  ) {
    event.stopPropagation();
    const agentId = bucket.agent?.id;
    if (!agentId) return; // The "Chats" bucket has no source agent, so no create entry.
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

  // Project-header "+": create a session from the agent definition attached to
  // the project; the backend overrides the working dir with project.path.
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
    <!-- Rename input: Enter/blur commits, Escape cancels. -->
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

<!-- Project subgroup: collapsible header + session list; the host bucket is passed in for create attribution. -->
{#snippet projectGroup(
  bucket: AgentSessionBucket,
  project: AgentProject,
  sessions: AgentSession[],
)}
  {@const key = projectCollapseKey(bucket.key, project.id)}
  {@const collapsed = agentProjectCollapse.isCollapsed(key)}
  {#if renamingProjectId === project.id}
    <!-- Project rename row replaces the header; the input sits in a data-group-control exemption span. -->
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
      <!-- Hover "+" creates a session for this agent + project. -->
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
  {#if createErrorMessage}
    <div
      class="mx-2 mt-2 mb-1 px-2 py-1 rounded-md bg-error/10 text-error text-[12px] leading-[18px] flex-shrink-0"
    >
      {createErrorMessage}
    </div>
  {/if}

  <!-- Grouped list (Agent → Project → Session; sessions without a source agent
       fall into the trailing Chats bucket). -->
  <div class="flex-1 overflow-y-auto space-y-1.5 px-2 pt-2">
    {#if !initialLoadDone}
      <div class="px-2 py-1 text-[12px] leading-[18px] text-base-content/50">
        {t("common.loading")}
      </div>
    {:else if loadError}
      <!-- Partial load failure: skip grouped rendering to avoid mis-bucketing into Chats. -->
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
        <div class="space-y-0.5">
          <!-- Bucket header: an Agent (icon + name + hover "+") or the Chats bucket (no create). -->
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

<!-- Context menu, dispatched by kind (session row / project header). -->
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
    <!-- Generate title: only offered when the session has messages to distill. -->
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

<!-- Close the menu on outside click / right-click (row right-clicks stopPropagation). -->
<svelte:window onclick={handleClickOutside} oncontextmenu={handleClickOutside} />
