<script lang="ts">
  import { untrack } from "svelte";
  import { browser } from "$app/environment";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { Bot } from "@lucide/svelte";
  import { t } from "$lib/i18n";
  import { uiState } from "$lib/states/ui.svelte";
  import {
    agentSessionState,
    agentSessionActions,
  } from "$lib/states/agentSession.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentApprovalStore } from "$lib/states/agentApproval.svelte";
  import { agentProjectState } from "$lib/states/agentProject.svelte";
  import AgentSessionHeader from "$lib/components/agentsession/AgentSessionHeader.svelte";
  import AgentInput from "$lib/components/agentsession/AgentInput.svelte";
  import AgentTimeline from "$lib/components/agentsession/AgentTimeline.svelte";
  import AgentApprovalModal from "$lib/components/agentsession/AgentApprovalModal.svelte";
  import AppPanel from "$lib/components/agentsession/AppPanel.svelte";
  import { reconstructAppArtifact } from "$lib/components/agentsession/renderApp";
  import { agentAppPanel } from "$lib/states/agentAppPanel.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import type {
    AgentApprovalRequest,
    ApprovalDecision,
  } from "$lib/types/agentSession";

  let sessionId = $derived(
    browser && $page.url ? $page.url.searchParams.get("id") || "" : "",
  );

  // Sidebar's AgentProjectList already fetched projects; read-only here to pick landing copy.
  const hasProjects = $derived(agentProjectState.projects.length > 0);

  // Remember the last opened session so switching back to Agent mode can restore it
  $effect(() => {
    if (sessionId) {
      uiState.setLastAgentSessionId(sessionId);
    }
  });

  // In-flight dedup for list fetches: when the list is truly empty, loadSessions'
  // new `sessions` array ref re-triggers the effect below and would refetch
  // forever. Reset on completion; only the in-flight window is deduped.
  let probedSessionId = "";

  // Stale restore pointer (id points at a deleted session, etc.): clear the
  // current session and lastAgentSessionId, then return to the landing page.
  // replaceState so back doesn't revisit the dead id and redirect again.
  function handleMissingSession(id: string) {
    agentSessionState.currentSession = null;
    // untrack: called synchronously inside the effect below; keep the pointer
    // read-write out of its dependencies to avoid redundant reruns.
    untrack(() => {
      if (uiState.lastAgentSessionId === id) {
        uiState.setLastAgentSessionId(null);
      }
    });
    goto("/agent", { replaceState: true });
  }

  // Sync the store's current session with ?id=. The list may not be loaded yet
  // (direct open of /agent?id=): fetch it first, then locate. Render even if the
  // session's working directory no longer exists on disk — never crash here.
  $effect(() => {
    if (!browser) {
      return;
    }
    if (!sessionId) {
      agentSessionState.currentSession = null;
      return;
    }
    const id = sessionId;
    if (agentSessionActions.setCurrentById(id)) {
      return;
    }
    // Not in the in-memory list: the list was never loaded (direct open) or is a
    // stale snapshot (e.g. a quick-action handoff session persisted after this
    // window's last loadSessions). Refetch from disk once and locate; only if
    // the id is still missing afterwards treat it as a stale pointer — never
    // bounce a loadable valid id.
    if (probedSessionId === id) {
      // Fetch for this id is in flight; .then below decides staleness — avoids a refetch loop
      return;
    }
    probedSessionId = id;
    agentSessionActions
      .loadSessions()
      .then(() => {
        probedSessionId = "";
        // Only treat as stale if the user is still on this id; fetch failures
        // go to catch and are never misread as staleness.
        if (sessionId === id && !agentSessionActions.setCurrentById(id)) {
          handleMissingSession(id);
        }
      })
      .catch((error) => {
        probedSessionId = "";
        console.error("Failed to load agent sessions:", error);
      });
  });

  const currentSession = $derived(agentSessionState.currentSession);

  // Seed the committed transcript on open (keyed per sessionId)
  $effect(() => {
    if (!browser || !sessionId) {
      return;
    }
    agentRunStore.loadTranscript(sessionId).catch((error) => {
      console.error("Failed to load agent transcript:", error);
    });
  });

  // On session switch, paint the shell (Header + Input) first and mount the heavy
  // AgentTimeline (markdown / highlight / katex parsing) a frame later; otherwise
  // Svelte paints only after rendering the whole page and switching feels slow.
  //
  // The gate must be derived (readySessionId === sessionId), not a boolean reset
  // in an effect: effects run after DOM update, so the switch-time render would
  // still re-render the full timeline once before unmounting it. Derived unmounts
  // the timeline in the same flush sessionId changes; double rAF lets the shell
  // paint before the timeline mounts.
  let readySessionId = $state("");
  const timelineReady = $derived(
    readySessionId !== "" && readySessionId === sessionId,
  );
  $effect(() => {
    const id = sessionId;
    if (!browser || !id) {
      return;
    }
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(() => {
        readySessionId = id;
      });
    });
    return () => {
      cancelAnimationFrame(raf1);
      cancelAnimationFrame(raf2);
    };
  });

  // Run view-model (reactive getter); used here only for empty-state checks
  const runState = $derived(
    sessionId ? agentRunStore.runStateFor(sessionId) : null,
  );

  const isEmpty = $derived(
    !runState ||
      (runState.messages.length === 0 &&
        !runState.streamingText &&
        !runState.thinkingText &&
        !runState.error &&
        !runState.isRunning),
  );

  // Claude-style new-chat layout: greeting + composer vertically centered.
  // messageCount === 0 means there are no persisted turns to restore, so a
  // fresh session renders it immediately — no spinner phase; sessions with
  // history keep the bottom-anchored composer while the transcript loads.
  const showCenteredComposer = $derived(
    isEmpty &&
      ((currentSession?.messageCount ?? 0) === 0 || runState?.hydrated === true),
  );

  // Pending approval for the current session (a dangerous tool call pauses the run)
  const pendingApproval = $derived(
    sessionId ? agentApprovalStore.pendingFor(sessionId) : null,
  );

  // render_app artifact, derived by replaying the transcript (create/update
  // collapse into one). Shared by the live and restored paths. Null when the
  // session made no render_app call, which keeps the panel hidden.
  const appArtifact = $derived(
    runState ? reconstructAppArtifact(runState.messages, runState.toolCalls) : null,
  );

  // Visible only for the current session, so switching sessions hides it.
  const showAppPanel = $derived(
    appArtifact !== null && agentAppPanel.openSessionId === sessionId,
  );

  // New render_app content arriving mid-run opens the panel. Streaming events
  // recompute the artifact into a fresh object constantly, so dedupe on a
  // content key (toolCallId + length): one open per distinct content, meaning a
  // panel the user closed is not reopened until a *new* create/update lands.
  // The key stays out of reactive state — it is bookkeeping only.
  let autoOpenedKey = "";
  $effect(() => {
    const contentKey = appArtifact
      ? `${appArtifact.toolCallId}:${appArtifact.content.length}`
      : "";
    if (!contentKey) {
      return;
    }
    if (!runState?.isRunning) {
      // Outside a run the key is only a baseline: restored/pre-existing
      // content must not auto-open the panel when the next run starts.
      autoOpenedKey = contentKey;
      return;
    }
    if (contentKey === autoOpenedKey) {
      return;
    }
    autoOpenedKey = contentKey;
    agentAppPanel.open(sessionId);
  });

  // allow_once permits this call, allow_always permits the tool for the session,
  // deny cancels the tool and the run continues with a denied result. Pass the
  // request the modal is showing so the store responds to that exact requestId
  // (no refetch-by-sessionId race).
  function handleApprovalRespond(
    request: AgentApprovalRequest,
    decision: ApprovalDecision,
  ) {
    void agentApprovalStore.respondTo(request, decision);
  }
</script>

<div class="flex-1 flex flex-col h-full">
  {#if sessionId}
    <AgentSessionHeader />

    <!-- Left column (timeline + input) plus the optional render_app panel. -->
    <div class="flex-1 flex min-h-0">
      <div class="flex-1 flex flex-col min-w-0">
        {#if isEmpty && !showCenteredComposer}
          <!-- First open with no cache: transcript restore in flight. Revisits are
               served from the per-session cache and skip this branch. -->
          <div class="flex-1 flex items-center justify-center">
            <Spinner size={28} />
          </div>
        {:else if showCenteredComposer}
          <!-- Greeting sits right above the composer; the spacer below the
               composer keeps the pair vertically centered. -->
          <div class="flex-1 flex flex-col items-center justify-end">
            <p class="mb-6 text-xl font-medium text-base-content/80">
              {t("agent.page.emptyGreeting")}
            </p>
          </div>
        {:else if timelineReady}
          <AgentTimeline {sessionId} appTitle={appArtifact?.title} />
        {:else}
          <!-- Placeholder so the shell paints first with a stable flex structure
               (Input stays bottom-anchored) until AgentTimeline mounts next frame. -->
          <div class="flex-1"></div>
        {/if}

        <!--
          `{#key currentSession.id}` remounts AgentInput per session: all transient
          composer state (input / attachments / forced chip / slash overlay) resets
          and never leaks between sessions. A fresh mount is the correct semantics.
        -->
        <div class="shrink-0 chat-column pb-3">
          {#if currentSession}
            {#key currentSession.id}
              <AgentInput session={currentSession} />
            {/key}
          {/if}
        </div>

        {#if showCenteredComposer}
          <!-- Bottom half of the centered layout. The composer div above stays in
               place across the empty → active transition, so AgentInput never
               remounts (focus and IME state survive the first send). -->
          <div class="flex-1"></div>
        {/if}
      </div>

      {#if showAppPanel && appArtifact}
        <AppPanel artifact={appArtifact} onClose={() => agentAppPanel.close()} />
      {/if}
    </div>

    <!-- Approval modal: shown while a dangerous tool call is pending; the
         decision flows back to the backend via the store. Keyed per session. -->
    {#if pendingApproval}
      <AgentApprovalModal
        request={pendingApproval}
        onRespond={handleApprovalRespond}
      />
    {/if}
  {:else}
    <!-- Empty landing page: deliberately no create action here; users go through the sidebar entry -->
    <div class="flex-1 flex flex-col items-center justify-center text-base-content/50">
      <Bot size={48} class="mb-4 opacity-20" />
      {#if hasProjects}
        <p class="text-sm">{t("agent.page.landingWithProjects")}</p>
      {:else}
        <p class="text-sm">{t("agent.page.landingNoProjects")}</p>
      {/if}
    </div>
  {/if}
</div>
