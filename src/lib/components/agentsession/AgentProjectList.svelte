<script module lang="ts">
  /**
   * The session list's scroll offset, kept outside the component so it survives
   * a teardown: entering settings unmounts the whole app layout, and a reader
   * whose session sits far down a long list should not be dumped back at the top.
   */
  let listScrollTop = 0;
</script>

<script lang="ts">
  import { tick, untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { goto } from "$app/navigation";
  import {
    Archive,
    ArchiveRestore,
    ChevronRight,
    Copy,
    Folder,
    FolderOpen,
    Loader2,
    MessagesSquare,
    PencilLine,
    Pin,
    PinOff,
    Plus,
    Sparkles,
    Trash2,
  } from "@lucide/svelte";
  import SessionHoverCard from "$lib/components/agentsession/SessionHoverCard.svelte";
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
    partitionArchivedSessions,
    sessionActivityKey,
  } from "$lib/utils/agentGrouping";
  import type { AgentSessionBucket } from "$lib/utils/agentGrouping";
  import { formatRelativeTime } from "$lib/utils/date";
  import { normalizeError } from "$lib/utils/error";
  import { onDestroy, onMount } from "svelte";
  import type { AgentSession } from "$lib/types";
  import type { AgentProject } from "$lib/types/agentProject";

  interface Props {
    activeId?: string;
  }

  let { activeId = "" }: Props = $props();

  // Grouping and ordering (Agent → Project → Session) are fully delegated to
  // the selector; the component does not re-implement them. Archived sessions
  // are split off first — they render flat in their own group at the bottom
  // rather than anywhere in the tree.
  const partitioned = $derived(
    partitionArchivedSessions(agentSessionState.sessions),
  );
  const buckets = $derived(
    groupSessionsByAgent(
      agentState.agents,
      agentProjectState.projects,
      partitioned.active,
    ),
  );
  const archivedSessions = $derived(partitioned.archived);
  const isEmpty = $derived(
    buckets.length === 0 && archivedSessions.length === 0,
  );

  // The Archived group starts collapsed on every load and is deliberately kept
  // out of `agentProjectCollapse` (whose contract is "missing = expanded", and
  // whose persistence would defeat the point of tucking sessions away).
  let archivedExpanded = $state(false);

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

  let listEl: HTMLDivElement | undefined;
  let scrollFrame = 0;

  // Set while the restore is driving scrollTop. Programmatic scrolling raises
  // the same events the reader's own does, and a restore that lands clamped
  // against a still-short list would otherwise record the clamped value and
  // lose the position it was reaching for.
  let restoringScroll = false;

  // Real input hands control back: stop asserting the old offset under them.
  let readerTookOver = false;

  function handleListScroll() {
    // Rows slide out from under the cursor, so a hover card would be left
    // pointing at nothing. Dismissed ahead of the early return below: a
    // programmatic restore scrolls the list too, and the rAF throttle must not
    // delay this. Guarded so an idle scroll does not write state every frame.
    if (hoverCard || hoverTimer !== null) cancelHoverCard();

    if (restoringScroll || scrollFrame) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0;
      listScrollTop = listEl?.scrollTop ?? 0;
    });
  }

  function noteReaderInput() {
    readerTookOver = true;
  }

  function raf(): Promise<void> {
    return new Promise((resolve) => requestAnimationFrame(() => resolve()));
  }

  /**
   * The list fills in stages — the warm store paints first, then the three
   * fetches land — so a single assignment clamps against content that is still
   * short. Keep asserting until the offset sticks, or until the budget runs out
   * (the list genuinely shrank, e.g. sessions were deleted while away).
   */
  async function restoreListScroll() {
    const target = listScrollTop;
    if (target <= 0) return;

    restoringScroll = true;
    readerTookOver = false;
    try {
      for (let frame = 0; frame < 40 && !readerTookOver; frame += 1) {
        await tick();
        if (!listEl) return;
        if (listEl.scrollTop !== target) {
          listEl.scrollTop = target;
        }
        if (listEl.scrollTop === target && frame >= 2) break;
        await raf();
      }
    } finally {
      restoringScroll = false;
    }
  }

  onMount(() => {
    loadSidebarData();
    void restoreListScroll();
    return () => cancelAnimationFrame(scrollFrame);
  });

  // A pending hover timer must not fire into a torn-down component.
  onDestroy(() => cancelHoverCard());

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

  // The open session must never be invisible in the sidebar: archiving the one
  // currently on screen (or opening an archived one) reveals the group instead
  // of silently dropping its highlight.
  $effect(() => {
    if (archivedSessions.some((session) => session.id === activeId)) {
      archivedExpanded = true;
    }
  });

  function handleSessionClick(session: AgentSession) {
    goto(`/agent?id=${session.id}`);
  }

  // --- Row hover / focus ----------------------------------------------------

  // The pin + archive controls share the relative time's slot, so which one is
  // rendered is decided in JS rather than by a CSS `:hover` variant: a
  // `display: none` control could never be reached by keyboard, and an
  // always-mounted `opacity-0` one would eat the time label's width for good.
  let activeRowSessionId = $state("");

  function handleRowEnter(event: MouseEvent, session: AgentSession) {
    activeRowSessionId = session.id;
    scheduleHoverCard(event, session);
  }

  function handleRowLeave(event: MouseEvent, session: AgentSession) {
    cancelHoverCard();
    // Keep the controls up only while the KEYBOARD is inside this row.
    // `document.activeElement` would also match the button the mouse just
    // clicked — clicking focuses it — latching the controls open for good
    // once the pointer moves away. `:focus-visible` is exactly the
    // keyboard-only distinction.
    const row = event.currentTarget as HTMLElement;
    if (row.matches(":focus-visible") || row.querySelector(":focus-visible")) {
      return;
    }
    if (activeRowSessionId === session.id) activeRowSessionId = "";
  }

  function handleRowFocusIn(session: AgentSession) {
    activeRowSessionId = session.id;
  }

  // focusout also fires when focus moves from the row onto one of its own
  // controls; only a move that leaves the row retracts them.
  function handleRowFocusOut(event: FocusEvent, session: AgentSession) {
    const row = event.currentTarget as HTMLElement;
    const next = event.relatedTarget;
    if (next instanceof Node && row.contains(next)) return;
    if (activeRowSessionId === session.id) activeRowSessionId = "";
  }

  // Enter / Space open the session; the row is a div (not a button) so the pin
  // and archive buttons can legally nest inside it.
  function handleSessionRowKeydown(event: KeyboardEvent, session: AgentSession) {
    if (event.key !== "Enter" && event.key !== " ") return;
    if (event.target !== event.currentTarget) return; // a control owns its own keys
    event.preventDefault();
    handleSessionClick(session);
  }

  // --- Hover card -----------------------------------------------------------

  // Delay before the card appears, so sweeping the cursor down the list does
  // not flash a card per row.
  const HOVER_CARD_DELAY_MS = 450;
  /** Gap between the sidebar row and the card, and from the viewport edge. */
  const HOVER_CARD_MARGIN = 8;
  /** Kept in sync with the card's own height budget for the bottom clamp. */
  const HOVER_CARD_MAX_HEIGHT = 160;

  interface HoverCard {
    session: AgentSession;
    x: number;
    y: number;
  }

  let hoverCard = $state<HoverCard | null>(null);
  let hoverTimer: ReturnType<typeof setTimeout> | null = null;

  function cancelHoverCard() {
    if (hoverTimer !== null) {
      clearTimeout(hoverTimer);
      hoverTimer = null;
    }
    hoverCard = null;
  }

  // Anchored to the row's right edge in viewport coordinates: the list scrolls,
  // so an absolutely positioned card would be clipped by the sidebar.
  function scheduleHoverCard(event: MouseEvent, session: AgentSession) {
    cancelHoverCard();
    const row = event.currentTarget as HTMLElement;
    hoverTimer = setTimeout(() => {
      hoverTimer = null;
      // A context menu or a rename input owns the interaction; no card on top.
      if (contextMenu || renamingSessionId) return;
      const rect = row.getBoundingClientRect();
      hoverCard = {
        session,
        x: rect.right + HOVER_CARD_MARGIN,
        // Ride the row's top edge, lifted just enough to clear the viewport
        // bottom — and never pushed above its top on a short window.
        y: Math.max(
          HOVER_CARD_MARGIN,
          Math.min(
            rect.top,
            window.innerHeight - HOVER_CARD_MAX_HEIGHT - HOVER_CARD_MARGIN,
          ),
        ),
      };
    }, HOVER_CARD_DELAY_MS);
  }

  // --- Pin / archive --------------------------------------------------------

  // Both are optimistic in the store, so the row reorders immediately; a
  // failure rolls back there and surfaces here in the shared error bar.
  async function togglePinned(session: AgentSession) {
    cancelHoverCard();
    contextMenu = null;
    createErrorMessage = null;
    try {
      await agentSessionActions.setPinned(session.id, !session.pinned);
    } catch (error) {
      const normalized = normalizeError(error, t("agent.list.pinFailed"));
      createErrorMessage = normalized.hint ?? normalized.message;
    }
  }

  async function toggleArchived(session: AgentSession) {
    cancelHoverCard();
    contextMenu = null;
    createErrorMessage = null;
    // Archiving the open session keeps it open in the main pane; only its
    // sidebar placement changes, so there is no navigation here.
    try {
      await agentSessionActions.setArchived(session.id, !session.archived);
    } catch (error) {
      const normalized = normalizeError(error, t("agent.list.archiveFailed"));
      createErrorMessage = normalized.hint ?? normalized.message;
    }
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
    // The menu owns the interaction from here; a card underneath it would only
    // fight for the same space.
    cancelHoverCard();
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
    cancelHoverCard();
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
  dotIndent: string,
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
    {@const showControls = activeRowSessionId === session.id}
    <div
      class="relative w-full flex items-center gap-2 py-1 {rowIndent} pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content hover:bg-base-300 cursor-default select-none {session.id ===
      activeId
        ? 'bg-base-300 text-base-content'
        : ''}"
      role="button"
      tabindex="0"
      onclick={() => handleSessionClick(session)}
      onkeydown={(event) => handleSessionRowKeydown(event, session)}
      oncontextmenu={(event) => handleSessionContextMenu(event, session)}
      onmouseenter={(event) => handleRowEnter(event, session)}
      onmouseleave={(event) => handleRowLeave(event, session)}
      onfocusin={() => handleRowFocusIn(session)}
      onfocusout={(event) => handleRowFocusOut(event, session)}
    >
      {#if agentRunStore.isRunning(session.id)}
        <!-- Same breathing dot as the timeline's in-run progress indicator,
             absolutely positioned in the indent gutter so it never shifts the
             title. The wrapper centers via flex: the dot's own animation owns
             `transform`, so translate-based centering would be overridden. -->
        <span
          class="absolute {dotIndent} inset-y-0 flex items-center"
          aria-hidden="true"
        >
          <span
            class="h-2 w-2 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
          ></span>
        </span>
      {/if}
      <span class="truncate flex-1">{session.name}</span>
      {#if generatingTitleId === session.id}
        <Loader2
          size={12}
          class="flex-shrink-0 animate-spin text-base-content/40"
        />
      {:else if showControls}
        <!-- Hover / focus swaps the relative time for the row's own controls. -->
        <span class="flex flex-shrink-0 items-center gap-0.5">
          {#if !session.archived}
            <button
              class="p-0.5 rounded text-base-content/55 hover:text-base-content hover:bg-base-content/10"
              title={session.pinned ? t("agent.list.unpin") : t("agent.list.pin")}
              aria-label={session.pinned
                ? t("agent.list.unpin")
                : t("agent.list.pin")}
              onclick={(event) => {
                event.stopPropagation();
                togglePinned(session);
              }}
            >
              {#if session.pinned}
                <PinOff size={14} />
              {:else}
                <Pin size={14} />
              {/if}
            </button>
          {/if}
          <button
            class="p-0.5 rounded text-base-content/55 hover:text-base-content hover:bg-base-content/10"
            title={session.archived
              ? t("agent.list.unarchive")
              : t("agent.list.archive")}
            aria-label={session.archived
              ? t("agent.list.unarchive")
              : t("agent.list.archive")}
            onclick={(event) => {
              event.stopPropagation();
              toggleArchived(session);
            }}
          >
            {#if session.archived}
              <ArchiveRestore size={14} />
            {:else}
              <Archive size={14} />
            {/if}
          </button>
        </span>
      {:else}
        {#if session.pinned}
          <Pin size={12} class="flex-shrink-0 text-base-content/45" />
        {/if}
        <span class="flex-shrink-0 text-[11px] text-base-content/55">
          {formatRelativeTime(sessionActivityKey(session))}
        </span>
      {/if}
    </div>
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
        {@render sessionRow(session, "pl-12", "pl-10", "left-9")}
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
  <div
    bind:this={listEl}
    class="flex-1 overflow-y-auto space-y-1.5 px-2 pt-2"
    onscroll={handleListScroll}
    onwheel={noteReaderInput}
    ontouchmove={noteReaderInput}
  >
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
                  {@render sessionRow(child.session, "pl-7", "pl-5", "left-4")}
                {/if}
              {/each}
            </div>
          {/if}
        </div>
      {/each}

      <!-- Archived sessions: flat, out of the Agent → Project tree, collapsed
           by default and absent entirely while nothing is archived. -->
      {#if archivedSessions.length > 0}
        <div class="space-y-0.5">
          <button
            class="w-full flex items-center gap-1.5 py-1 pl-2 pr-2 text-left rounded-md text-[12px] leading-[18px] font-normal text-base-content/70 hover:text-base-content hover:bg-base-300 select-none"
            aria-expanded={archivedExpanded}
            onclick={() => (archivedExpanded = !archivedExpanded)}
          >
            <Archive size={14} class="flex-shrink-0 text-base-content/60" />
            <span class="truncate flex-1">{t("agent.list.archived")}</span>
            <span class="flex-shrink-0 text-[11px] text-base-content/45">
              {archivedSessions.length}
            </span>
            <ChevronRight
              size={14}
              class="flex-shrink-0 text-base-content/40 transition-transform duration-[var(--dur-fast)] {archivedExpanded
                ? 'rotate-90'
                : ''}"
            />
          </button>
          {#if archivedExpanded}
            <div class="space-y-0.5" transition:slide={{ duration: 160 }}>
              {#each archivedSessions as session (session.id)}
                {@render sessionRow(session, "pl-7", "pl-5", "left-4")}
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {/if}
  </div>
</div>

<!-- Session summary shown beside the hovered row (informational, never focusable). -->
{#if hoverCard}
  <SessionHoverCard
    session={hoverCard.session}
    x={hoverCard.x}
    y={hoverCard.y}
  />
{/if}

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
  {@const menuSession = contextMenu.session}
  <div
    class="context-menu fixed z-[var(--z-dropdown)] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl px-1 py-1 min-w-36"
    style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
  >
    <!-- Generate title: only offered when the session has messages to distill. -->
    {#if menuSession.messageCount > 0}
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

    <!-- Same two actions as the row's hover controls, for right-click users. -->
    {#if !menuSession.archived}
      <button
        class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
        onclick={() => togglePinned(menuSession)}
      >
        {#if menuSession.pinned}
          <PinOff size={14} />
          {t("agent.list.unpin")}
        {:else}
          <Pin size={14} />
          {t("agent.list.pin")}
        {/if}
      </button>
    {/if}

    <button
      class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
      onclick={() => toggleArchived(menuSession)}
    >
      {#if menuSession.archived}
        <ArchiveRestore size={14} />
        {t("agent.list.unarchive")}
      {:else}
        <Archive size={14} />
        {t("agent.list.archive")}
      {/if}
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
