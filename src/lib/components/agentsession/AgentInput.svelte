<script lang="ts">
  import {
    ArrowUp,
    Square,
    Paperclip,
    Plus,
    X,
    Ban,
    ChevronDown,
    ChevronRight,
    ChevronsUpDown,
    Check,
    Folder,
    Zap,
    SignalLow,
    SignalMedium,
    SignalHigh,
  } from "@lucide/svelte";
  import { onDestroy, tick } from "svelte";
  import { fly } from "svelte/transition";
  import { goto } from "$app/navigation";
  import Button from "$lib/components/ui/Button.svelte";
  import McpIcon from "$lib/components/ui/McpIcon.svelte";
  import ModelSelectModal from "./ModelSelectModal.svelte";
  import SkillSlashPopover from "./SkillSlashPopover.svelte";
  import { t } from "$lib/i18n";
  import { resolveAgentIcon } from "$lib/utils/agentIcons";
  import { agentSessionActions } from "$lib/states/agentSession.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { mcpState, mcpActions } from "$lib/states/mcp.svelte";
  import { agentRunStore } from "$lib/states/agentRun.svelte";
  import { agentApprovalStore } from "$lib/states/agentApproval.svelte";
  import { agentQuestionStore } from "$lib/states/agentQuestion.svelte";
  import { agentQuoteStore } from "$lib/states/agentQuote.svelte";
  import { withQuote } from "./quote";
  import AgentQuestionPanel from "./AgentQuestionPanel.svelte";
  import { getAllModels, getProviderIconById } from "$lib/states/provider.svelte";
  import { runAgentStream, steerAgentRun } from "$lib/api/agentSession";
  import { listSkills } from "$lib/api/skill";
  import type {
    Agent,
    AgentSession,
    AgentRunAttachment,
    AgentQuestionRequest,
    AgentQuestionResponse,
    McpServer,
    SkillInfo,
  } from "$lib/types";
  import type { ModelWithProvider } from "$lib/types/provider";

  interface Props {
    session: AgentSession;
  }

  let { session }: Props = $props();

  // Thinking-effort levels (thinkingLevel is a free-text backend field).
  // $derived so labels re-render on language switch.
  const thinkingLevelOptions = $derived([
    {
      value: "off",
      label: t("agent.thinking.off"),
      desc: t("agent.thinking.offDesc"),
      icon: Ban,
    },
    {
      value: "low",
      label: t("agent.thinking.low"),
      desc: t("agent.thinking.lowDesc"),
      icon: SignalLow,
    },
    {
      value: "medium",
      label: t("agent.thinking.medium"),
      desc: t("agent.thinking.mediumDesc"),
      icon: SignalMedium,
    },
    {
      value: "high",
      label: t("agent.thinking.high"),
      desc: t("agent.thinking.highDesc"),
      icon: SignalHigh,
    },
  ]);

  // Soft per-image limit (10 MiB). Oversized images are skipped with a hint
  // instead of pushing huge byte arrays through IPC, which can hang the UI.
  const MAX_IMAGE_BYTES = 10 * 1024 * 1024;

  let input = $state("");
  let textareaRef: HTMLTextAreaElement;
  let modelPrompt = $state<string | null>(null);

  // Image attachments (image/* only). `previewUrl` is an object URL for the
  // thumbnail; revoked on remove/send/unmount to avoid leaks.
  type AttachmentWithPreview = {
    id: string;
    name: string;
    mimeType: string;
    data: Uint8Array;
    previewUrl: string;
  };
  let attachments = $state<AttachmentWithPreview[]>([]);
  let fileInputRef: HTMLInputElement | null = null;

  // The session stores modelId/providerId; the picker needs ModelWithProvider,
  // so look it up in the catalog.
  const selectedModel = $derived<ModelWithProvider | null>(
    session.modelId && session.providerId
      ? (getAllModels().find(
          (m) =>
            m.id === session.modelId && m.provider_id === session.providerId,
        ) ?? null)
      : null,
  );
  const selectedModelIcon = $derived(
    selectedModel ? getProviderIconById(selectedModel.provider_id) : undefined,
  );
  let modelModalOpen = $state(false);

  const thinkingLevel = $derived(session.thinkingLevel ?? "off");
  const thinkingLevelLabel = $derived(
    thinkingLevelOptions.find((o) => o.value === thinkingLevel)?.label ??
      thinkingLevelOptions[0].label,
  );

  // Thinking-effort menu: the hovered item drives the footer description,
  // falling back to the selected item's when nothing is hovered.
  let thinkingMenuOpen = $state(false);
  let thinkingMenuHover = $state<string | null>(null);
  const thinkingMenuDesc = $derived(
    (
      thinkingLevelOptions.find(
        (o) => o.value === (thinkingMenuHover ?? thinkingLevel),
      ) ?? thinkingLevelOptions[0]
    ).desc,
  );

  // Close on outside click; clicks inside the menu stopPropagation and never reach window.
  $effect(() => {
    if (!thinkingMenuOpen) return;
    const handler = () => (thinkingMenuOpen = false);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  });

  function toggleThinkingMenu(event: MouseEvent) {
    event.stopPropagation();
    // stopPropagation defeats the other popovers' outside-click close, so close them explicitly.
    agentMenuOpen = false;
    closeAddMenu();
    thinkingMenuHover = null;
    thinkingMenuOpen = !thinkingMenuOpen;
  }

  function selectThinkingLevel(value: string) {
    thinkingMenuOpen = false;
    if (value === thinkingLevel) return;
    handleThinkingChange(value);
  }

  // Agent picker: selecting another definition instantiates a new session from
  // it and navigates there; re-selecting the current one is a no-op.
  let agentMenuOpen = $state(false);

  // Source Agent of this session; null when unloaded/deleted/absent, in which
  // case the trigger falls back to the "select agent" placeholder.
  const currentAgent = $derived<Agent | null>(
    session.agentDefinitionId
      ? (agentState.agents.find((a) => a.id === session.agentDefinitionId) ??
          null)
      : null,
  );
  const currentAgentLabel = $derived(
    currentAgent?.name ?? t("agent.input.selectAgent"),
  );
  const CurrentAgentIcon = $derived(resolveAgentIcon(currentAgent?.icon));

  // Show the working-dir picker unless workingDirMode is "none" (required /
  // optional / NULL definitions all take a working directory).
  const showWorkingDir = $derived(
    !!currentAgent && currentAgent.workingDirMode !== "none",
  );
  // Working-dir basename fits the compact button; null when unset.
  const workingDirName = $derived.by(() => {
    const dir = session.workingDir;
    if (!dir) return null;
    const parts = dir.split("/").filter(Boolean);
    return parts.length > 0 ? parts[parts.length - 1] : dir;
  });

  // Lazy-load agents (the /agent route doesn't). Builtin agents are always
  // seeded, so length === 0 reliably means "not loaded yet". Also load when the
  // session has a source agent, to resolve workingDirMode for the dir picker.
  $effect(() => {
    if (
      (agentMenuOpen || session.agentDefinitionId) &&
      agentState.agents.length === 0
    ) {
      agentActions
        .loadAgents()
        .catch((error) => console.error("Failed to load agents:", error));
    }
  });

  // Close on outside click; clicks inside the menu stopPropagation and never reach window.
  $effect(() => {
    if (!agentMenuOpen) return;
    const handler = () => (agentMenuOpen = false);
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  });

  function toggleAgentMenu(event: MouseEvent) {
    event.stopPropagation();
    // stopPropagation defeats the other popovers' outside-click close, so close them explicitly.
    thinkingMenuOpen = false;
    closeAddMenu();
    agentMenuOpen = !agentMenuOpen;
  }

  async function selectAgent(agent: Agent) {
    agentMenuOpen = false;
    if (!agent.id) return;
    // Already the session's source agent: no-op.
    if (agent.id === session.agentDefinitionId) return;
    try {
      // Untouched session (no messages, no active run): repoint it in place,
      // keeping id/URL; otherwise instantiate a new session and navigate.
      // No overrides: the new definition's model/working-dir policy wins.
      if (session.messageCount === 0 && !agentRunStore.isRunning(session.id)) {
        await agentSessionActions.reinstantiateFromDefinition(
          session.id,
          agent.id,
        );
        return;
      }
      const created =
        await agentSessionActions.createSessionFromDefinition(agent.id);
      await goto(`/agent?id=${created.id}`);
    } catch (error) {
      console.error("Failed to switch agent:", error);
      modelPrompt = t("agent.input.switchAgentFailed");
    }
  }

  // Pick the session working dir via the system dialog; the backend validates
  // it as an existing absolute directory. Cancel (non-string result) is a no-op.
  async function pickWorkingDir() {
    try {
      const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
      const picked = await openDialog({ directory: true });
      if (typeof picked !== "string") return;
      modelPrompt = null;
      await agentSessionActions.updateField(session.id, "workingDir", picked);
    } catch (error) {
      console.error("Failed to set working directory:", error);
      modelPrompt =
        error instanceof Error
          ? error.message
          : t("agent.input.workingDirFailed");
    }
  }

  // Transcript text the reader quoted from the timeline; rendered as a card
  // above the textarea and prepended to the message as a blockquote on send.
  const quote = $derived(agentQuoteStore.quoteFor(session.id));

  function removeQuote() {
    agentQuoteStore.clear(session.id);
  }

  // Quoting is a request to write about that text, so the keyboard follows it
  // into the composer.
  $effect(() => {
    if (quote === null) return;
    textareaRef?.focus();
  });

  // Active run for this session drives the Send <-> Stop toggle.
  const running = $derived(agentRunStore.isRunning(session.id));

  // A pending approval for a dangerous tool call (write/edit/bash) pauses the
  // conversation: input disabled, send blocked, until the user decides in the
  // page-level AgentApprovalModal.
  const awaitingApproval = $derived(agentApprovalStore.hasPending(session.id));

  // A pending `ask_question` call pauses the conversation the same way, except
  // the answering surface is the inline panel above this composer rather than a
  // modal — the user must answer or skip before typing again.
  const pendingQuestion = $derived(agentQuestionStore.pendingFor(session.id));

  // Either pause disables the composer; they never overlap in practice (the
  // agent loop awaits one parked call at a time) but the guard is symmetric.
  const paused = $derived(awaitingApproval || pendingQuestion !== null);

  // Slash skill autocomplete: typing `/` into an empty textarea (not pasted,
  // not during IME composition) opens a popover of enabled skills filtered
  // case-insensitively by the text after `/`. Selecting writes `/<name> ` back
  // into the input; the text is the single source of truth — on send the
  // leading `/<name>` is parsed as the forced skill name and the backend
  // injects the skill body into the system prompt.

  let slashOpen = $state(false);
  let slashQuery = $state("");
  let slashHighlight = $state(0);
  let availableSkills = $state<SkillInfo[]>([]);
  // During IME composition the popover neither opens, selects, nor sends.
  let composing = $state(false);

  const slashCandidates = $derived.by(() => {
    const q = slashQuery.trim().toLowerCase();
    const enabled = availableSkills.filter((s) => !s.disabled);
    if (!q) return enabled;
    return enabled.filter((s) => s.name.toLowerCase().includes(q));
  });

  // Clamp the highlight when filtering shrinks the list; -1 when empty.
  const effectiveHighlight = $derived(
    slashCandidates.length === 0
      ? -1
      : Math.min(slashHighlight, slashCandidates.length - 1),
  );

  async function loadAvailableSkills() {
    try {
      availableSkills = await listSkills(session.workingDir ?? undefined);
    } catch (error) {
      console.error("Failed to list skills for slash popover:", error);
      availableSkills = [];
    }
  }

  function openSlashPopover() {
    slashOpen = true;
    slashHighlight = 0;
    slashQuery = "";
    void loadAvailableSkills();
  }

  function closeSlashPopover() {
    slashOpen = false;
    slashQuery = "";
    slashHighlight = 0;
  }

  // Clear the typed /query from the textarea.
  function clearSlashQuery() {
    input = "";
    adjustTextareaHeight();
  }

  // Return focus to the textarea with the caret at the end.
  async function focusInputEnd() {
    await tick();
    if (!textareaRef) return;
    textareaRef.focus();
    const end = textareaRef.value.length;
    textareaRef.setSelectionRange(end, end);
    adjustTextareaHeight();
  }

  async function selectSkill(skill: SkillInfo) {
    // Replace the typed /query with `/<name> `, then return focus with the
    // caret at the end. The forced skill name is re-parsed from the leading
    // `/<name>` on send (see leadingForcedSkillNames).
    input = `/${skill.name} `;
    closeSlashPopover();
    await focusInputEnd();
  }

  // Picking a skill from the add menu keeps whatever is already typed: only an
  // existing leading `/<skill>` token is swapped, since the forced skill is
  // parsed from that token on send.
  async function selectSkillFromMenu(skill: SkillInfo) {
    closeAddMenu();
    closeSlashPopover();
    const text = input.trimStart();
    const leading = leadingForcedSkillNames(text)[0];
    const rest = leading ? text.slice(leading.length + 1).trimStart() : text;
    input = rest ? `/${skill.name} ${rest}` : `/${skill.name} `;
    await focusInputEnd();
  }

  // Leading `/<name>` → forced skill name; only known enabled skills match,
  // anything else is plain text. The slash trigger only fires on a lone `/`,
  // so there is at most one leading forced skill.
  function leadingForcedSkillNames(text: string): string[] {
    const m = text.trimStart().match(/^\/(\S+)/);
    if (!m) return [];
    const name = m[1];
    return availableSkills.some((s) => !s.disabled && s.name === name)
      ? [name]
      : [];
  }

  function adjustTextareaHeight() {
    if (textareaRef) {
      textareaRef.style.height = "auto";
      const scrollHeight = textareaRef.scrollHeight;
      const maxHeight = 200;
      textareaRef.style.height = Math.min(scrollHeight, maxHeight) + "px";
    }
  }

  // Enter sends; Shift+Enter inserts a newline. While the popover is open the
  // keyboard is consumed first: arrows move the highlight, Enter selects
  // (never sends), Escape closes. IME composition swallows every key.
  function handleKeydown(event: KeyboardEvent) {
    // IME composing: leave all keys to the IME (belt and braces: flag + isComposing).
    if (composing || event.isComposing) return;

    if (slashOpen) {
      // ArrowDown/Ctrl|Cmd+N and ArrowUp/Ctrl|Cmd+P move the highlight
      // (clamped); preventDefault keeps the text caret still.
      const mod = event.metaKey || event.ctrlKey;
      const navDown =
        event.key === "ArrowDown" || (mod && event.key.toLowerCase() === "n");
      const navUp =
        event.key === "ArrowUp" || (mod && event.key.toLowerCase() === "p");
      if (navDown) {
        event.preventDefault();
        if (slashCandidates.length > 0) {
          slashHighlight = Math.min(
            effectiveHighlight + 1,
            slashCandidates.length - 1,
          );
        }
        return;
      }
      if (navUp) {
        event.preventDefault();
        if (slashCandidates.length > 0) {
          slashHighlight = Math.max(effectiveHighlight - 1, 0);
        }
        return;
      }
      if (event.key === "Enter" && !event.shiftKey) {
        // Enter selects instead of sending; no highlight → no-op.
        event.preventDefault();
        const target = slashCandidates[effectiveHighlight];
        if (target) selectSkill(target);
        return;
      }
      if (event.key === "Escape") {
        // Close and consume the /query.
        event.preventDefault();
        clearSlashQuery();
        closeSlashPopover();
        return;
      }
    }

    // Escape drops the quote once the popover is out of the way, so the reader
    // can undo a mis-quote without reaching for the card's button.
    if (event.key === "Escape" && quote !== null) {
      event.preventDefault();
      removeQuote();
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      sendAgentRun();
    }
  }

  function handleCompositionStart() {
    composing = true;
  }

  function handleCompositionEnd() {
    composing = false;
    // After composition commits, the text participates in trigger/filter as usual.
    syncSlashState(false);
  }

  // Sync popover trigger/query with textarea input; a pasted `/` must not trigger.
  function syncSlashState(fromPaste: boolean) {
    // Not during composition (text not committed yet).
    if (composing) return;

    // Trigger: input is exactly "/" and not pasted.
    if (!slashOpen) {
      if (!fromPaste && input === "/") openSlashPopover();
      return;
    }

    // Open: query is the text after `/`; close once the leading `/` is gone.
    if (!input.startsWith("/")) {
      closeSlashPopover();
      return;
    }
    slashQuery = input.slice(1);
    slashHighlight = 0;
  }

  function handleInput(event: Event) {
    adjustTextareaHeight();
    const inputType = (event as InputEvent).inputType;
    const fromPaste =
      inputType === "insertFromPaste" || inputType === "insertFromDrop";
    syncSlashState(fromPaste);
  }

  // "+" menu: attachments plus Skills and MCP submenus. Skills come from the
  // same source as the slash popover, so both stay in sync.
  let addMenuOpen = $state(false);
  // At most one submenu is open at a time.
  let openSubmenu = $state<"skills" | "mcp" | null>(null);
  const menuSkills = $derived(availableSkills.filter((s) => !s.disabled));

  // Only ready servers exposing tools can contribute anything to a run, so the
  // submenu lists those (same filter as the Agent editor's MCP picker).
  const availableMcpServers = $derived(
    mcpState.servers.filter(
      (s) => s.enabled && s.status === "ready" && s.enabledTools.length > 0,
    ),
  );
  const sessionMcpServers = $derived(session.mcpServers ?? []);

  // Close on outside click; clicks inside the menu stopPropagation and never reach window.
  $effect(() => {
    if (!addMenuOpen) return;
    const handler = () => closeAddMenu();
    window.addEventListener("click", handler);
    return () => window.removeEventListener("click", handler);
  });

  function closeAddMenu() {
    addMenuOpen = false;
    openSubmenu = null;
  }

  function toggleAddMenu(event: MouseEvent) {
    event.stopPropagation();
    // stopPropagation defeats the other popovers' outside-click close, so close them explicitly.
    thinkingMenuOpen = false;
    agentMenuOpen = false;
    openSubmenu = null;
    addMenuOpen = !addMenuOpen;
    // The submenus need their lists before they are hovered.
    if (addMenuOpen) {
      void loadAvailableSkills();
      mcpActions
        .loadServers(mcpState.needsRefresh)
        .catch((error) => console.error("Failed to load MCP servers:", error));
    }
  }

  // Bind / unbind a server for this session. The binding carries the server's
  // own tool selection (an empty list would expose no tools) and defaults to
  // auto execution; the menu stays open so several can be toggled in a row.
  async function toggleMcpServer(server: McpServer) {
    const bound = sessionMcpServers.some((c) => c.serverId === server.id);
    const next = bound
      ? sessionMcpServers.filter((c) => c.serverId !== server.id)
      : [
          ...sessionMcpServers,
          {
            serverId: server.id,
            executionMode: "auto" as const,
            enabledTools: server.enabledTools,
          },
        ];
    try {
      modelPrompt = null;
      await agentSessionActions.updateField(session.id, "mcpServers", next);
    } catch (error) {
      console.error("Failed to update session MCP servers:", error);
      modelPrompt = t("agent.input.mcpUpdateFailed");
    }
  }

  function handleAddAttachment(event?: MouseEvent) {
    event?.stopPropagation();
    closeAddMenu();
    fileInputRef?.click();
  }

  // Accept image/* only; oversized images are silently skipped. Raw bytes are
  // kept for sending, an object URL for the thumbnail preview.
  async function handleAttachmentChange(event: Event) {
    const target = event.currentTarget as HTMLInputElement;
    const files = target.files;
    if (!files || files.length === 0) return;

    const additions: AttachmentWithPreview[] = [];
    let skippedOversize = false;
    for (const file of Array.from(files)) {
      if (!file.type.startsWith("image/")) continue;
      if (file.size > MAX_IMAGE_BYTES) {
        skippedOversize = true;
        continue;
      }
      const buffer = await file.arrayBuffer();
      additions.push({
        id: crypto.randomUUID(),
        name: file.name,
        mimeType: file.type || "image/png",
        data: new Uint8Array(buffer),
        previewUrl: URL.createObjectURL(file),
      });
    }

    if (additions.length) {
      attachments = [...attachments, ...additions];
    }
    if (skippedOversize) {
      modelPrompt = t("agent.input.oversizeSkipped");
    }

    // Reset so re-picking the same file fires change again.
    if (fileInputRef) {
      fileInputRef.value = "";
    }
  }

  function removeAttachment(id: string) {
    const item = attachments.find((a) => a.id === id);
    if (item?.previewUrl.startsWith("blob:")) {
      URL.revokeObjectURL(item.previewUrl);
    }
    attachments = attachments.filter((a) => a.id !== id);
  }

  function resetAttachments() {
    attachments.forEach((a) => {
      if (a.previewUrl.startsWith("blob:")) {
        URL.revokeObjectURL(a.previewUrl);
      }
    });
    attachments = [];
    if (fileInputRef) {
      fileInputRef.value = "";
    }
  }

  onDestroy(() => {
    resetAttachments();
  });

  async function sendAgentRun() {
    // Paused on an approval or a question: neither start a run nor enqueue
    // steering until the user decides. No-op, input preserved.
    if (paused) return;

    // Run in progress: route the message into the active run's steering queue
    // (drained at turn boundaries) instead of starting a second run. Steering
    // is text-only: attachments are dropped and a leading `/<name>` is not
    // parsed as a forced skill. An active run always has a model, so this
    // branch safely precedes the model guard.
    if (running) {
      // Whitespace-only input with nothing quoted: no-op.
      if (!input.trim() && quote === null) return;
      modelPrompt = null;
      const text = withQuote(input, quote);
      resetAttachments();
      removeQuote();
      input = "";
      adjustTextareaHeight();
      try {
        await steerAgentRun(session.id, text);
      } catch (error) {
        // On steer failure just surface the error; don't restore the cleared input.
        console.error("Failed to steer agent run:", error);
        modelPrompt =
          error instanceof Error
            ? error.message
            : t("agent.input.steerFailed");
      }
      return;
    }

    // Empty input with no attachments and nothing quoted: no run, no bubble.
    if (!input.trim() && attachments.length === 0 && quote === null) return;

    // No model: prompt and block (defensive; created sessions normally have one).
    if (!session.modelId || !session.providerId) {
      modelPrompt = t("agent.input.selectModelFirst");
      return;
    }

    modelPrompt = null;
    // The typed text and the quote are kept apart from the composed message:
    // a failed start restores exactly what the user had, card included.
    const typed = input;
    const quoted = quote;
    const text = withQuote(typed, quoted);
    // Snapshot attachments for sending (Uint8Array -> number[] to match the
    // backend Vec<u8> IPC shape), then clear input. The user bubble comes from
    // the backend's user message_end event; no optimistic insert to avoid dupes.
    const payloadAttachments: AgentRunAttachment[] = attachments.map((a) => ({
      name: a.name,
      mimeType: a.mimeType,
      data: Array.from(a.data),
    }));
    const sentAttachments = attachments;
    // Forced skill names come from the leading `/<name>` of what the user typed
    // (a quote sits in front of it in the sent text): the input is the single
    // source of truth, so restoring it on failure also restores them.
    // `/<name>` is sent to the model verbatim; the backend injects skill bodies.
    const forcedSkillNames = leadingForcedSkillNames(typed);
    input = "";
    attachments = [];
    removeQuote();
    adjustTextareaHeight();
    try {
      await runAgentStream(
        session.id,
        text,
        payloadAttachments,
        forcedSkillNames,
      );
      // Revoke preview URLs only after a successful send (thumbnails left the DOM).
      sentAttachments.forEach((a) => {
        if (a.previewUrl.startsWith("blob:")) {
          URL.revokeObjectURL(a.previewUrl);
        }
      });
    } catch (error) {
      // Start failed: restore input, attachments and quote (the leading
      // `/<name>` restores the forced skill) and surface the error for retry.
      input = typed;
      attachments = sentAttachments;
      if (quoted !== null) {
        agentQuoteStore.set(session.id, quoted);
      }
      adjustTextareaHeight();
      modelPrompt =
        error instanceof Error ? error.message : t("agent.input.runFailed");
    }
  }

  async function handleStop() {
    try {
      await agentRunStore.abort(session.id);
    } catch (error) {
      console.error("Failed to abort agent run:", error);
    }
  }

  function handleModelSelect(model: ModelWithProvider) {
    modelPrompt = null;
    agentSessionActions
      .updateField(session.id, "modelId", model.id)
      .then(() =>
        agentSessionActions.updateField(
          session.id,
          "providerId",
          model.provider_id,
        ),
      )
      .catch((error) => {
        console.error("Failed to update agent session model:", error);
      });
  }

  // Answers (or a dismissal) flow straight back to the parked tool call. Pass
  // the request the panel is showing so the store answers that exact requestId.
  function handleQuestionRespond(
    request: AgentQuestionRequest,
    response: AgentQuestionResponse,
  ) {
    void agentQuestionStore.respondTo(request, response);
  }

  function handleThinkingChange(value: string) {
    agentSessionActions
      .updateField(session.id, "thinkingLevel", value)
      .catch((error) => {
        console.error("Failed to update agent session thinking level:", error);
      });
  }
</script>

<input
  type="file"
  accept="image/*"
  multiple
  class="hidden"
  bind:this={fileInputRef}
  onchange={handleAttachmentChange}
/>

<!-- Model select modal; selection writes modelId+providerId as a pair. -->
<ModelSelectModal
  bind:open={modelModalOpen}
  {selectedModel}
  onModelSelect={handleModelSelect}
/>

<!-- Working-dir picker above the composer; shown unless workingDirMode is "none". -->
{#if showWorkingDir}
  <div class="flex w-full pb-1">
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-md px-1.5 py-1 text-xs text-base-content/55 hover:bg-base-300/50 hover:text-base-content transition-colors"
      aria-label={t("agent.input.selectWorkingDir")}
      title={session.workingDir ?? t("agent.input.selectWorkingDir")}
      onclick={pickWorkingDir}
    >
      <Folder size={13} class="shrink-0" />
      {#if workingDirName}
        <span class="max-w-[220px] truncate">{workingDirName}</span>
      {:else}
        <span class="max-w-[220px] truncate text-warning"
          >{t("agent.input.selectWorkingDir")}</span
        >
      {/if}
    </button>
  </div>
{/if}

<!-- ask_question panel, docked directly above the composer so it reads as
     sliding up out of the input. `{#key requestId}` remounts it per request, so
     a new question set never inherits the previous one's selections. -->
{#if pendingQuestion}
  {#key pendingQuestion.requestId}
    <AgentQuestionPanel
      request={pendingQuestion}
      onRespond={handleQuestionRespond}
    />
  {/key}
{/if}

<!-- Composer status line. Deliberately ABOVE the input box, not inside it:
     these are notices about the conversation's state, not part of what the user
     is composing, and rendering them between the textarea and the button row
     made the box look like it had grown an extra field. Every status message
     the composer shows belongs here. -->
{#if awaitingApproval || pendingQuestion || modelPrompt}
  <div class="flex w-full flex-col gap-1 pb-1.5">
    {#if awaitingApproval}
      <!-- Paused on a dangerous tool call: a gate, so it reads as a warning. -->
      <div class="flex items-center gap-2 px-1 text-xs text-warning">
        <span
          class="h-2 w-2 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
        ></span>
        <span>{t("agent.input.awaitingApprovalHint")}</span>
      </div>
    {:else if pendingQuestion}
      <!-- Paused on the question panel above — not a gate, so primary accent. -->
      <div class="flex items-center gap-2 px-1 text-xs text-primary">
        <span
          class="h-2 w-2 rounded-full bg-current animate-[pulse-scale_1.5s_ease-in-out_infinite]"
        ></span>
        <span>{t("agent.input.awaitingQuestionHint")}</span>
      </div>
    {/if}
    {#if modelPrompt}
      <div class="px-1 text-xs text-warning">{modelPrompt}</div>
    {/if}
  </div>
{/if}

<div
  class="flex flex-col bg-[var(--bg-page)] rounded-lg border border-[var(--hairline)] w-full"
>
  <!-- Quoted transcript text, inside the box and above the textarea: it is part
       of the message being composed, not a notice about the conversation.
       Clamped to a few lines — it is a reference to a passage, not the passage. -->
  {#if quote !== null}
    <div class="px-4 pt-3">
      <div
        class="relative rounded-md border border-[var(--hairline)] bg-base-200/40 py-2 pl-2.5 pr-8"
        transition:fly={{ y: -4, duration: 130 }}
      >
        <p
          class="line-clamp-3 border-l-2 border-base-content/25 pl-2 text-xs leading-[1.55] whitespace-pre-wrap break-words text-base-content/65"
        >
          {quote}
        </p>
        <button
          type="button"
          class="absolute right-1 top-1 flex h-5 w-5 items-center justify-center rounded-md text-base-content/50 transition-colors hover:bg-base-300 hover:text-base-content"
          aria-label={t("agent.input.removeQuote")}
          title={t("agent.input.removeQuote")}
          onclick={removeQuote}
        >
          <X size={12} />
        </button>
      </div>
    </div>
  {/if}

  <!-- Relative container anchors the popover, which opens upward (bottom-full) to stay on-screen. -->
  <div class="relative">
    {#if slashOpen}
      <div
        class="absolute bottom-full left-3 z-30 mb-1"
        transition:fly={{ y: -4, duration: 130 }}
      >
        <SkillSlashPopover
          items={slashCandidates}
          highlightedIndex={effectiveHighlight}
          onSelect={selectSkill}
          onHover={(index) => (slashHighlight = index)}
        />
      </div>
    {/if}
    <textarea
      bind:this={textareaRef}
      bind:value={input}
      placeholder={awaitingApproval
        ? t("agent.input.awaitingApprovalPlaceholder")
        : pendingQuestion
          ? t("agent.input.awaitingQuestionPlaceholder")
          : t("agent.input.placeholder")}
      onkeydown={handleKeydown}
      oninput={handleInput}
      oncompositionstart={handleCompositionStart}
      oncompositionend={handleCompositionEnd}
      rows="1"
      disabled={paused}
      class="composer-input bg-transparent text-[14px] text-base-content/80 p-4 outline-none resize-none w-full min-h-[48px] max-h-[200px] overflow-y-auto disabled:cursor-not-allowed disabled:opacity-60"
    ></textarea>
  </div>

  {#if attachments.length}
    <div class="px-4 pb-2 flex flex-wrap gap-3">
      {#each attachments as attachment (attachment.id)}
        <div
          class="relative w-20 h-20 rounded-lg overflow-hidden border border-base-300 bg-base-100"
        >
          <img
            src={attachment.previewUrl}
            alt={attachment.name}
            class="w-full h-full object-cover"
          />
          <button
            class="absolute top-1 right-1 p-1 bg-base-200/80 hover:bg-base-200 rounded-full text-base-content transition-colors"
            type="button"
            title={t("agent.input.removeImage")}
            onclick={() => removeAttachment(attachment.id)}
          >
            <X size={12} />
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="flex flex-row items-center justify-between gap-3 px-4 pt-0 pb-2">
    <div class="flex flex-row flex-wrap items-center gap-2">
      <!-- "+" menu: attachments and the Skills submenu. -->
      <div class="relative">
        <button
          type="button"
          class={`flex h-7 w-7 items-center justify-center rounded-md transition-colors ${
            addMenuOpen
              ? "bg-base-300/60 text-base-content"
              : "text-base-content hover:bg-base-300/60"
          }`}
          aria-label={t("agent.input.addMenu")}
          aria-haspopup="menu"
          aria-expanded={addMenuOpen}
          title={t("agent.input.addMenu")}
          onclick={toggleAddMenu}
        >
          <Plus size={16} />
        </button>

        {#if addMenuOpen}
          <!-- Opens upward (bottom-full) to stay on-screen; stopPropagation
               keeps inside clicks from triggering the outside-click close. -->
          <div
            transition:fly={{ y: -4, duration: 130 }}
            class="absolute bottom-full left-0 z-40 mb-2 w-52 rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
            role="menu"
            tabindex="-1"
            onclick={(event) => event.stopPropagation()}
            onkeydown={() => {}}
          >
            <button
              type="button"
              role="menuitem"
              class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300"
              onclick={handleAddAttachment}
            >
              <Paperclip size={16} class="shrink-0 text-base-content/70" />
              <span class="min-w-0 flex-1 truncate text-sm text-base-content">
                {t("agent.input.addImage")}
              </span>
            </button>

            <!-- Skills row: hovering the row (and the gap the submenu's padding
                 covers) keeps the submenu open; clicking toggles it for pointers
                 that don't hover. -->
            <div
              class="relative"
              role="none"
              onmouseenter={() => (openSubmenu = "skills")}
              onmouseleave={() => (openSubmenu = null)}
            >
              <button
                type="button"
                role="menuitem"
                aria-haspopup="menu"
                aria-expanded={openSubmenu === "skills"}
                class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
                  openSubmenu === "skills" ? "bg-base-300" : ""
                }`}
                onclick={() =>
                  (openSubmenu = openSubmenu === "skills" ? null : "skills")}
              >
                <Zap size={16} class="shrink-0 text-base-content/70" />
                <span class="min-w-0 flex-1 truncate text-sm text-base-content">
                  {t("agent.input.skills")}
                </span>
                <ChevronRight size={14} class="shrink-0 opacity-60" />
              </button>

              {#if openSubmenu === "skills"}
                <!-- pl-1 (not ml-1) so the gap to the parent menu stays inside
                     the hover area and crossing it doesn't close the submenu. -->
                <div class="absolute bottom-0 left-full z-50 pl-1">
                  <div
                    class="max-h-72 w-56 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
                    role="menu"
                    tabindex="-1"
                  >
                    {#if menuSkills.length === 0}
                      <div class="px-2 py-1.5 text-xs text-base-content/50">
                        {t("agent.input.noSkills")}
                      </div>
                    {:else}
                      {#each menuSkills as skill (skill.name)}
                        <button
                          type="button"
                          role="menuitem"
                          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300"
                          onclick={() => selectSkillFromMenu(skill)}
                        >
                          <Zap size={16} class="shrink-0 text-base-content/70" />
                          <span
                            class="min-w-0 flex-1 truncate text-sm text-base-content"
                          >
                            {skill.name}
                          </span>
                        </button>
                      {/each}
                    {/if}
                  </div>
                </div>
              {/if}
            </div>

            <!-- MCP row: the submenu is a checklist of this session's server
                 bindings, so it stays open while several are toggled. -->
            <div
              class="relative"
              role="none"
              onmouseenter={() => (openSubmenu = "mcp")}
              onmouseleave={() => (openSubmenu = null)}
            >
              <button
                type="button"
                role="menuitem"
                aria-haspopup="menu"
                aria-expanded={openSubmenu === "mcp"}
                class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
                  openSubmenu === "mcp" ? "bg-base-300" : ""
                }`}
                onclick={() =>
                  (openSubmenu = openSubmenu === "mcp" ? null : "mcp")}
              >
                <McpIcon size={16} class="shrink-0 text-base-content/70" />
                <span class="min-w-0 flex-1 truncate text-sm text-base-content">
                  {t("agent.input.mcp")}
                </span>
                {#if sessionMcpServers.length}
                  <span class="shrink-0 text-xs text-base-content/45">
                    {sessionMcpServers.length}
                  </span>
                {/if}
                <ChevronRight size={14} class="shrink-0 opacity-60" />
              </button>

              {#if openSubmenu === "mcp"}
                <div class="absolute bottom-0 left-full z-50 pl-1">
                  <div
                    class="max-h-72 w-56 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
                    role="menu"
                    tabindex="-1"
                  >
                    {#if availableMcpServers.length === 0}
                      <div class="px-2 py-1.5 text-xs text-base-content/50">
                        {t("agent.input.noAvailableMcpServers")}
                      </div>
                    {:else}
                      {#each availableMcpServers as server (server.id)}
                        {@const bound = sessionMcpServers.some(
                          (c) => c.serverId === server.id,
                        )}
                        <button
                          type="button"
                          role="menuitemcheckbox"
                          aria-checked={bound}
                          class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300"
                          onclick={() => toggleMcpServer(server)}
                        >
                          <McpIcon
                            size={16}
                            class="shrink-0 text-base-content/70"
                          />
                          <span
                            class="min-w-0 flex-1 truncate text-sm text-base-content"
                          >
                            {server.displayName || server.name}
                          </span>
                          {#if bound}
                            <Check size={14} class="shrink-0 text-primary" />
                          {/if}
                        </button>
                      {/each}
                    {/if}
                  </div>
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>

      <!-- Agent picker: selecting another definition instantiates a new session and navigates there. -->
      <div class="relative">
        <button
          type="button"
          class={`flex h-7 items-center gap-1.5 rounded-md pl-1.5 pr-2 transition-colors ${
            agentMenuOpen
              ? "bg-base-300/60 text-base-content"
              : "text-base-content hover:bg-base-300/60"
          }`}
          aria-label={t("agent.input.selectAgent")}
          aria-haspopup="menu"
          aria-expanded={agentMenuOpen}
          title={t("agent.input.selectAgent")}
          onclick={toggleAgentMenu}
        >
          <CurrentAgentIcon size={16} class="shrink-0" />
          <span class="max-w-[140px] truncate text-sm">{currentAgentLabel}</span>
          <ChevronDown size={14} class="shrink-0 opacity-60" />
        </button>

        {#if agentMenuOpen}
          <!-- Opens upward (bottom-full) to stay on-screen; stopPropagation
               keeps inside clicks from triggering the outside-click close. -->
          <div
            transition:fly={{ y: -4, duration: 130 }}
            class="absolute bottom-full left-0 z-40 mb-2 max-h-72 w-52 overflow-y-auto rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
            role="menu"
            tabindex="-1"
            onclick={(event) => event.stopPropagation()}
            onkeydown={() => {}}
          >
            {#if agentState.agents.length === 0}
              <div class="px-2 py-1.5 text-xs text-base-content/50">
                {t("common.loading")}
              </div>
            {:else}
              {#each agentState.agents as agent (agent.id)}
                {@const active = agent.id === session.agentDefinitionId}
                {@const ItemIcon = resolveAgentIcon(agent.icon)}
                <button
                  type="button"
                  role="menuitem"
                  class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
                    active ? "bg-base-300/60" : ""
                  }`}
                  onclick={() => selectAgent(agent)}
                >
                  <ItemIcon size={16} class="shrink-0 text-base-content/70" />
                  <span
                    class="min-w-0 flex-1 truncate text-sm text-base-content"
                  >
                    {agent.name}
                  </span>
                  {#if active}
                    <Check size={14} class="shrink-0 text-primary" />
                  {/if}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    </div>
    <div class="flex flex-row items-center gap-3">
      <!-- Session model trigger: opens the model modal; an unresolved model
           shows the "select model" placeholder. -->
      <button
        type="button"
        class="flex h-7 items-center gap-1.5 rounded-md px-2 py-1 text-sm text-base-content/80 hover:bg-base-300/60 transition-colors"
        aria-label={t("agent.input.selectModel")}
        title={selectedModel?.name ?? t("agent.input.selectModel")}
        onclick={() => (modelModalOpen = true)}
      >
        {#if selectedModel}
          {#if selectedModelIcon}
            <img
              src={selectedModelIcon}
              alt={selectedModel.providerName}
              class="h-4 w-4 shrink-0 rounded object-contain"
            />
          {/if}
          <span class="max-w-[240px] truncate">{selectedModel.name}</span>
        {:else}
          <span class="max-w-[240px] truncate text-warning"
            >{t("agent.input.selectModel")}</span
          >
        {/if}
        <ChevronsUpDown size={13} class="shrink-0 opacity-60" />
      </button>

      <!-- Thinking-effort menu: custom popover because a native select cannot
           style options or show a description footer. -->
      <div class="relative">
        <button
          type="button"
          class={`flex h-7 items-center gap-1 rounded-md px-2 text-sm transition-colors ${
            thinkingMenuOpen
              ? "bg-base-300/60 text-base-content"
              : "text-base-content/80 hover:bg-base-300/60"
          }`}
          aria-label={t("agent.thinking.label")}
          aria-haspopup="listbox"
          aria-expanded={thinkingMenuOpen}
          title={t("agent.thinking.label")}
          onclick={toggleThinkingMenu}
        >
          <span>{thinkingLevelLabel}</span>
          <ChevronsUpDown size={13} class="shrink-0 opacity-60" />
        </button>

        {#if thinkingMenuOpen}
          <!-- Opens upward, right-aligned so the popover stays inside the window;
               stopPropagation keeps inside clicks from closing it. -->
          <div
            transition:fly={{ y: -4, duration: 130 }}
            class="absolute bottom-full right-0 z-40 mb-2 w-52 rounded-lg border border-[var(--hairline)] bg-base-100 p-1 shadow-lg"
            role="listbox"
            tabindex="-1"
            onclick={(event) => event.stopPropagation()}
            onkeydown={() => {}}
          >
            {#each thinkingLevelOptions as opt (opt.value)}
              {@const active = opt.value === thinkingLevel}
              {@const Icon = opt.icon}
              <button
                type="button"
                role="option"
                aria-selected={active}
                class={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-base-300 ${
                  active ? "bg-base-300/60" : ""
                }`}
                onmouseenter={() => (thinkingMenuHover = opt.value)}
                onmouseleave={() => (thinkingMenuHover = null)}
                onclick={() => selectThinkingLevel(opt.value)}
              >
                <Icon size={15} class="shrink-0 text-base-content/70" />
                <span class="min-w-0 flex-1 truncate text-sm text-base-content">
                  {opt.label}
                </span>
                {#if active}
                  <Check size={14} class="shrink-0 text-primary" />
                {/if}
              </button>
            {/each}
            <div
              class="mt-1 border-t border-[var(--hairline)] px-2 pb-1 pt-1.5 text-xs text-base-content/55"
            >
              {thinkingMenuDesc}
            </div>
          </div>
        {/if}
      </div>
      {#if running}
        <Button
          variant="neutral"
          size="icon"
          shape="pill"
          class="enabled:hover:opacity-90"
          ariaLabel={t("agent.input.stop")}
          onclick={handleStop}
        >
          <Square size={16} />
        </Button>
      {:else}
        <Button
          variant="neutral"
          size="icon"
          shape="pill"
          class="enabled:hover:opacity-90"
          ariaLabel={t("agent.input.send")}
          disabled={paused}
          onclick={sendAgentRun}
        >
          <ArrowUp size={18} />
        </Button>
      {/if}
    </div>
  </div>
</div>
