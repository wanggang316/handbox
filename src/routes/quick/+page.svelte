<!--
  Quick Action overlay host: a two-step Raycast-style composer on the unified
  Agent engine, rendered as a rounded card filling a frameless/transparent
  NSPanel. The panel hides instead of destroying the webview, so every summon
  resets to a fresh blank state — one summon = one single-turn document.

  Steps: pick an agent (typing filters agents runnable without a working
  directory; Backspace on empty input returns here), send one message
  (instantiates a real session and streams one turn via the same engine as
  /agent), then answered (input disabled, transcript only; Cmd+Enter hands the
  persisted session to the main window). No model picker, no New, no
  stop/follow-up; Esc closes at any time.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Bot } from "@lucide/svelte";
  import QuickInput from "$lib/components/quickaction/QuickInput.svelte";
  import QuickAgentList from "$lib/components/quickaction/QuickAgentList.svelte";
  import AgentTimeline from "$lib/components/agentsession/AgentTimeline.svelte";
  import type { Agent, UUID } from "$lib/types";
  import { isTauriEnvironment } from "$lib/utils/tauri";
  import { t } from "$lib/i18n";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { providerActions, getAllModels } from "$lib/states/provider.svelte";
  import { settingsState } from "$lib/states/settings.svelte";
  import { resolveAgentDefaultModel } from "$lib/utils/defaultModel";
  import { runAgentStream } from "$lib/api/agentSession";

  let composer = $state<QuickInput | null>(null);

  let value = $state("");
  // highlight index into filteredAgents (pick step)
  let highlightIndex = $state(0);
  // null = still in the pick step
  let selectedAgent = $state<Agent | null>(null);
  // session id once a turn has been sent; null before sending
  let sessionId = $state<UUID | null>(null);
  // in-flight guard: repeated Enter must not create a second session
  let sending = $state(false);
  // in-flight guard: a second Cmd+Enter returns early
  let continuing = $state(false);
  // send-failure message rendered in the footer
  let runError = $state<string | null>(null);

  // Pick-step candidates: only agents runnable in the overlay. The overlay
  // provides no working directory, so exclude working_dir_mode=required.
  const runnableAgents = $derived(
    agentState.agents.filter((a) => a.workingDirMode !== "required"),
  );

  const filteredAgents = $derived.by(() => {
    const query = value.trim().toLowerCase();
    if (!query) return runnableAgents;
    return runnableAgents.filter((a) => a.name.toLowerCase().includes(query));
  });

  // Show pick-step content only when agents exist or loading has finished (so
  // empty/no-match states can show); while loading with none yet, the summon
  // shows just a clean input row.
  const showPickerContent = $derived(
    selectedAgent === null &&
      (runnableAgents.length > 0 || !agentState.isLoading),
  );
  const hasContent = $derived(sessionId !== null || showPickerContent);

  // answered step: input disabled; Cmd+Enter only available then
  const isAnswered = $derived(sessionId !== null);
  const canContinue = $derived(sessionId !== null);

  const placeholder = $derived(
    selectedAgent
      ? t("quickaction.messagePlaceholder", { name: selectedAgent.name })
      : t("quickaction.searchPlaceholder"),
  );

  // Reset highlight to the first item whenever the query changes (pick step)
  $effect(() => {
    void value;
    if (selectedAgent === null) highlightIndex = 0;
  });

  function focusInput(): void {
    composer?.focus();
  }

  /** Reset to a fresh blank state (each summon = one single-turn document). */
  function resetOverlay(): void {
    selectedAgent = null;
    sessionId = null;
    value = "";
    highlightIndex = 0;
    runError = null;
    sending = false;
  }

  function selectAgent(agent: Agent): void {
    selectedAgent = agent;
    value = "";
    runError = null;
    focusInput();
  }

  function deselectAgent(): void {
    selectedAgent = null;
    value = "";
    highlightIndex = 0;
    focusInput();
  }

  function moveHighlight(delta: number): void {
    const len = filteredAgents.length;
    if (len === 0) return;
    highlightIndex = (highlightIndex + delta + len) % len;
  }

  // Enter dispatches by step: pick the highlighted agent, or send the message;
  // clean no-op when answered (input already disabled).
  function handleSubmit(): void {
    if (isAnswered) return;
    if (selectedAgent === null) {
      const agent = filteredAgents[highlightIndex];
      if (agent) selectAgent(agent);
      return;
    }
    void sendMessage(selectedAgent, value);
  }

  /**
   * Instantiate a real agent session from the selected definition (snapshotting
   * its capabilities and model/provider), then drive one turn with
   * `runAgentStream` — the same engine as the main window's /agent. On failure,
   * fall back to the message step with the text restored for retry.
   */
  async function sendMessage(agent: Agent, text: string): Promise<void> {
    if (!text.trim()) return;
    if (sending) return;
    const agentId = agent.id;
    if (!agentId) return;

    sending = true;
    runError = null;
    try {
      // Sessions don't inherit a model from the AgentDefinition; the overlay
      // runs on the app-wide default model (settings > Agent), resolved against
      // the catalog and passed as a paired modelId+providerId override so an
      // unrunnable default is caught before a session is created. Load the
      // catalog first — helper windows skip the main window's preload.
      if (getAllModels().length === 0) {
        await providerActions.loadProvidersWithModels();
      }
      const resolved = resolveAgentDefaultModel(
        settingsState.settings?.agent,
        getAllModels(),
      );
      if (!resolved.available) {
        // Empty catalog / no default picked / default delisted: point the user to settings
        runError = t("quickaction.model.unavailable");
        value = text;
        focusInput();
        return;
      }

      const session = await agentSessionActions.createSessionFromDefinition(
        agentId,
        { modelId: resolved.modelId, providerId: resolved.providerId },
      );
      const id = session.id;
      if (!id) {
        runError = t("quickaction.runFailed");
        value = text;
        focusInput();
        return;
      }

      sessionId = id;
      value = "";
      await runAgentStream(id, text, [], []);
    } catch (error) {
      console.error("quick: failed to send message", error);
      runError =
        error instanceof Error ? error.message : t("quickaction.runFailed");
      // Fall back to the message step, keep the agent, restore text for retry
      sessionId = null;
      value = text;
      focusInput();
    } finally {
      sending = false;
    }
  }

  /**
   * Cmd+Enter "continue in chat": hand the persisted session to the main window.
   * The backend fronts it and broadcasts `quick-action-open-agent` (handled in
   * the (app) layout), then the overlay hides; the next summon starts blank.
   */
  async function handleContinue(): Promise<void> {
    if (sessionId === null) return;
    if (continuing) return;
    if (!isTauriEnvironment()) return;

    const id = sessionId;
    continuing = true;
    try {
      await invoke("quick_action_continue_in_agent", { sessionId: id });
      await invoke("quick_action_hide");
    } catch (error) {
      console.error("quick: failed to continue in agent", error);
    } finally {
      continuing = false;
    }
  }

  /** Hide the overlay (the command only resolves under Tauri). */
  async function hideOverlay(): Promise<void> {
    if (!isTauriEnvironment()) return;
    await invoke("quick_action_hide");
  }

  onMount(() => {
    focusInput();

    // This route lives outside the (app) group and skips the main layout's
    // initialization; load agents (pick-step data) and providers itself.
    agentActions.loadAgents().catch((error) => {
      console.error("quick: failed to load agents", error);
    });
    providerActions.loadProvidersWithModels().catch((error) => {
      console.error("quick: failed to load providers", error);
    });

    if (!isTauriEnvironment()) return;

    // The webview survives hide/show: the backend broadcasts `quick-action-shown`
    // whenever the panel becomes key (= a new summon); reset to blank and refocus.
    // onFocusChanged is unreliable across hide/show for a nonactivating panel,
    // hence the native become-key signal.
    let unlisten: UnlistenFn | null = null;
    let stale = false;
    listen("quick-action-shown", () => {
      resetOverlay();
      focusInput();
    })
      .then((fn) => {
        if (stale) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((error) => {
        console.error("quick: failed to listen for shown event", error);
      });

    return () => {
      stale = true;
      unlisten?.();
    };
  });

  /**
   * Window-level keys: Esc closes the overlay anytime; Cmd+Enter is the
   * answered-step fallback (the disabled textarea no longer receives keys).
   * Pick/message-step keys are handled by QuickInput's callbacks.
   */
  async function handleWindowKeydown(event: KeyboardEvent): Promise<void> {
    if (event.key === "Escape") {
      event.preventDefault();
      await hideOverlay();
      return;
    }
    if (
      event.key === "Enter" &&
      (event.metaKey || event.ctrlKey) &&
      canContinue
    ) {
      event.preventDefault();
      await handleContinue();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<!--
  Raycast-style panel: input row on top, content area injected via children,
  footer at the bottom. The panel hugs its content height; the rest of the
  window stays transparent.
-->
<div class="flex h-full w-full flex-col overflow-hidden text-[var(--base-content)]">
  <QuickInput
    bind:this={composer}
    bind:value
    {placeholder}
    selectedAgentName={selectedAgent?.name ?? null}
    disabled={isAnswered}
    {canContinue}
    {runError}
    {hasContent}
    onSubmit={handleSubmit}
    onContinue={handleContinue}
    onArrowDown={() => moveHighlight(1)}
    onArrowUp={() => moveHighlight(-1)}
    onDeselect={deselectAgent}
  >
    {#snippet children()}
      {#if sessionId !== null}
        <AgentTimeline {sessionId} />
      {:else if selectedAgent === null}
        {#if runnableAgents.length === 0}
          <div class="flex flex-col items-center justify-center gap-3 px-6 py-9 text-center">
            <Bot size={26} class="text-[var(--base-content)]/35" />
            <div class="flex flex-col gap-1">
              <p class="text-sm font-medium">{t("quickaction.noAgents.title")}</p>
              <p class="text-xs text-[var(--base-content)]/55">
                {t("quickaction.noAgents.description")}
              </p>
            </div>
          </div>
        {:else if filteredAgents.length === 0}
          <div class="px-4 py-6 text-center text-sm text-[var(--base-content)]/50">
            {t("quickaction.noMatch")}
          </div>
        {:else}
          <QuickAgentList
            agents={filteredAgents}
            {highlightIndex}
            onSelect={selectAgent}
            onHover={(i) => (highlightIndex = i)}
          />
        {/if}
      {/if}
    {/snippet}
  </QuickInput>
</div>

<style>
  /* Transparent window: only the card is visible, keeping the frameless rounded-overlay look */
  :global(html),
  :global(body) {
    background: transparent;
  }
</style>
