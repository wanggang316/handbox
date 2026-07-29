<script lang="ts">
  /**
   * Agent session header: shows the session's optional project and title.
   * Driven by `agentSessionState.currentSession`, so it appears on reopen.
   */
  import {
    FolderOpen,
    ExternalLink,
    ChevronDown,
    Code2,
    SquareTerminal,
    Check,
  } from "@lucide/svelte";
  import { agentSessionState } from "$lib/states/agentSession.svelte";
  import { agentProjectState } from "$lib/states/agentProject.svelte";
  import {
    listOpenInTargets,
    openInTarget,
    type OpenInTarget,
  } from "$lib/api/openIn";
  import { settingsState } from "$lib/states/settings.svelte";
  import { uiState } from "$lib/states/ui.svelte";

  const session = $derived(agentSessionState.currentSession);

  // Optional project name: projects are loaded by the sidebar AgentProjectList
  // and read reactively here; null (no project / list not ready) shows title only.
  const projectName = $derived.by(() => {
    const projectId = session?.projectId;
    if (!projectId) return null;
    return (
      agentProjectState.projects.find((p) => p.id === projectId)?.name ?? null
    );
  });

  // "Open in ..." split button: opens the session working dir in an external
  // editor / terminal / Finder. Probing, icons and launching live in the
  // backend (commands/open_in.rs); the target list is cached per installed-app
  // set and prefetched on mount so the default-app icon shows immediately.
  let openInMenuOpen = $state(false);
  let openInTargets = $state<OpenInTarget[] | null>(null);
  let openInLoading = $state(false);
  let openInError = $state<string | null>(null);

  // Stored default app id (persisted in agent settings, survives restarts).
  const defaultEditorId = $derived(
    settingsState.settings?.agent?.defaultEditorId ?? null,
  );

  // Resolve the default target: the stored default if still available, else
  // the first editor/terminal, else the first target (usually Finder).
  const resolvedDefault = $derived.by((): OpenInTarget | null => {
    const targets = openInTargets;
    if (!targets || targets.length === 0) return null;
    return (
      targets.find((t) => t.id === defaultEditorId) ??
      targets.find((t) => t.kind !== "system") ??
      targets[0] ??
      null
    );
  });

  function iconForKind(kind: OpenInTarget["kind"]) {
    if (kind === "terminal") return SquareTerminal;
    if (kind === "system") return FolderOpen;
    return Code2;
  }

  async function loadOpenInTargets() {
    if (openInTargets !== null || openInLoading) return;
    openInLoading = true;
    openInError = null;
    try {
      openInTargets = await listOpenInTargets();
    } catch (error) {
      openInError = error instanceof Error ? error.message : String(error);
    } finally {
      openInLoading = false;
    }
  }

  // Prefetch targets and settings when a working dir exists, so the default
  // app icon needs no click. Both loads are idempotent.
  $effect(() => {
    if (session?.workingDir) {
      void loadOpenInTargets();
      void settingsState.loadSettings();
    }
  });

  function toggleOpenInMenu(event: MouseEvent) {
    // Don't bubble to the window click-outside handler, which would instantly close it.
    event.stopPropagation();
    if (openInMenuOpen) {
      closeOpenInMenu();
      return;
    }
    openInMenuOpen = true;
    openInError = null;
    void loadOpenInTargets();
  }

  function closeOpenInMenu() {
    openInMenuOpen = false;
    openInError = null;
  }

  // Open without changing the default. Success closes the menu; failure keeps it open.
  async function openTarget(target: OpenInTarget) {
    const dir = session?.workingDir;
    if (!dir) return;
    try {
      await openInTarget(dir, target.id);
      closeOpenInMenu();
    } catch (error) {
      // Launch failure is not silent: the menu stays with a visible error bar.
      openInError = error instanceof Error ? error.message : String(error);
    }
  }

  // Dropdown pick: open, and remember editor/terminal picks as the default
  // ("default editor" excludes Finder). Persist failure never blocks opening.
  async function pickTarget(target: OpenInTarget) {
    if (target.kind !== "system" && target.id !== defaultEditorId) {
      settingsState
        .updateSettings({
          section: "agent",
          data: { defaultEditorId: target.id },
        })
        .catch((error) => console.error("设置默认编辑器失败:", error));
    }
    await openTarget(target);
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    if (openInMenuOpen) closeOpenInMenu();
  }

  // Close on any click outside the popover.
  function handleWindowClick(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (openInMenuOpen && !target.closest(".openin-popover")) {
      closeOpenInMenu();
    }
  }

  // Close the Open-in menu on session switch: a menu left open over a new
  // session is confusing. The last id is a plain (non-reactive) let so the
  // effect does not retrigger itself.
  let lastOpenInSessionId: string | null = null;
  $effect(() => {
    const id = session?.id ?? null;
    if (id !== lastOpenInSessionId) {
      lastOpenInSessionId = id;
      if (openInMenuOpen) closeOpenInMenu();
    }
  });
</script>

{#if session}
  <!-- With the sidebar collapsed, pad the header past the traffic lights and
       sidebar toggle (toggle at left:100px, ~30px wide); the padding transition
       matches the sidebar animation. -->
  <header
    class="flex items-center gap-3 px-4 py-2.5 border-b border-base-300 shrink-0 transition-[padding-left] duration-[var(--dur-base)] {uiState.sidebarOpen
      ? ''
      : 'pl-[136px]'}"
  >
    <div class="flex items-center gap-2 min-w-0 text-sm h-7">
      {#if projectName}
        <span class="text-base-content/50 truncate shrink-0 max-w-[40%]">
          {projectName}
        </span>
        <span class="text-base-content/30 shrink-0">/</span>
      {/if}
      <span class="font-medium text-base-content truncate">
        {session.name}
      </span>
    </div>

    <!-- z-[10000]: the TitleBar .drag-region (fixed, height:50px, z-index:9999)
         covers this strip, so button mousedown would drag the window instead of
         clicking. Lift above the drag layer (same as TitleBar's own buttons);
         the popover rises with the container so its interactions aren't eaten. -->
    <div class="relative z-[10000] ml-auto shrink-0 flex items-center gap-1">
      <!-- Open in… split button: left opens the default app (showing its real
           icon), right expands the app list. Only shown when the session has a
           working directory. -->
      {#if session.workingDir}
        <div
          class="flex items-center rounded-md overflow-hidden border border-base-300/60"
        >
          <button
            type="button"
            class="h-7 pl-1.5 pr-1 flex items-center bg-transparent text-base-content hover:bg-base-300 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title={resolvedDefault
              ? `在 ${resolvedDefault.name} 中打开`
              : "Open in…"}
            aria-label="在默认应用中打开"
            disabled={!resolvedDefault}
            onclick={() => resolvedDefault && openTarget(resolvedDefault)}
          >
            {#if resolvedDefault?.icon}
              <img
                src={resolvedDefault.icon}
                alt=""
                class="w-4 h-4 rounded-[3px]"
              />
            {:else}
              <ExternalLink size={15} />
            {/if}
          </button>
          <button
            type="button"
            class="h-7 px-0.5 flex items-center bg-transparent text-base-content/70 hover:bg-base-300 transition-colors border-l border-base-300/60"
            title="选择应用"
            aria-label="选择要打开的应用"
            onclick={toggleOpenInMenu}
          >
            <ChevronDown size={13} />
          </button>
        </div>

        {#if openInMenuOpen}
          <div
            class="openin-popover absolute right-0 top-full mt-2 z-[var(--z-popover)] min-w-48 max-w-[80vw] bg-[var(--bg-card)] border border-[var(--hairline)] rounded-lg shadow-xl p-1 flex flex-col max-h-96 overflow-y-auto"
          >
            {#if openInError}
              <div
                class="m-1 text-xs text-error bg-error/10 border border-error/20 rounded-md px-2 py-1.5 break-words"
              >
                {openInError}
              </div>
            {/if}

            {#if openInLoading}
              <div class="px-2 py-1.5 text-xs text-base-content/50">
                检测可用应用…
              </div>
            {:else if openInTargets && openInTargets.length > 0}
              {#each openInTargets as target (target.id)}
                {@const FallbackIcon = iconForKind(target.kind)}
                {@const isDefault =
                  target.kind !== "system" && target.id === defaultEditorId}
                <button
                  type="button"
                  class="w-full px-2 py-1 text-left text-[13px] rounded-lg hover:bg-primary hover:text-primary-content flex items-center gap-2 whitespace-nowrap"
                  onclick={() => pickTarget(target)}
                >
                  {#if target.icon}
                    <img
                      src={target.icon}
                      alt=""
                      class="w-4 h-4 rounded-[3px] shrink-0"
                    />
                  {:else}
                    <FallbackIcon size={14} class="shrink-0" />
                  {/if}
                  <span class="flex-1 truncate">{target.name}</span>
                  {#if isDefault}
                    <Check size={13} class="shrink-0 opacity-70" />
                  {/if}
                </button>
              {/each}
            {:else if !openInError}
              <div class="px-2 py-1.5 text-xs text-base-content/50">
                未检测到可用应用
              </div>
            {/if}
          </div>
        {/if}
      {/if}
    </div>
  </header>
{/if}

<!-- Close on Escape / outside click (the trigger click already stopPropagation). -->
<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />
