<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import {
    Eye,
    Languages,
    Sparkles,
    X,
    Pin,
    PinOff,
    Copy,
    RotateCcw,
    MessageCirclePlus,
    ChevronDown,
  } from "@lucide/svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { hideContentPanel, setContentPanelPinned } from "$lib/api/selection";
  import { settingsState } from "$lib/states/settings.svelte";
  import {
    providerActions,
    getAllModels,
  } from "$lib/states/provider.svelte";
  import { resolveAgentDefaultModel } from "$lib/utils/defaultModel";
  import { t } from "$lib/i18n";
  import {
    runAgentTextTurn,
    createSessionFromDefinition,
    updateAgentSessionField,
  } from "$lib/api/agentSession";
  import {
    resolveSpec,
    looksLikeStreamingSpec,
  } from "$lib/components/genui/jsonui/resolveSpec";
  import { uiRegistry } from "$lib/components/genui/jsonui/registry";
  import { Renderer, JsonUIProvider } from "@json-render/svelte";
  import type { Spec } from "@json-render/core";

  const appWindow = getCurrentWindow();

  type TranslationResult = {
    term: string;
    translation: string;
    targetLanguage: string;
    phonetic: string | null;
    explanation: string | null;
  };

  let content = $state({
    mode: "" as "show" | "translate" | "ai" | "",
    text: "",
    app_info: { name: "", bundle_id: "", pid: 0 },
  });

  // spec and result are mutually exclusive: a final reply that is a valid
  // JSON-Render spec renders as a GenUI card (spec), otherwise fall back to
  // structured/plain-text parsing (result).
  let translation = $state({
    isLoading: false,
    result: null as TranslationResult | null,
    spec: null as Spec | null,
    error: null as string | null,
  });

  let isPinned = $state(false);

  let showModeDropdown = $state(false);

  const modeConfig = $derived({
    show: { icon: Eye, label: t("selection.modeShow"), color: "text-error" },
    translate: {
      icon: Languages,
      label: t("selection.modeTranslate"),
      color: "text-info",
    },
    ai: { icon: Sparkles, label: t("selection.modeAi"), color: "text-primary" },
  });

  onMount(() => {
    console.log("=====> [selection/content] onMount executed");

    showModeDropdown = false;

    const unlisten = listen("init-content", async (event: any) => {
      const { mode, text, x, y, app_info } = event.payload;
      content = { mode, text, app_info };
      // New content resets the pin state
      isPinned = false;
      await setContentPanelPinned(false);
      console.log("-----> content received: ", content);

      if (mode === "translate" && text) {
        await handleTranslate();
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  });

  async function handleClose() {
    content = {
      mode: "",
      text: "",
      app_info: { name: "", bundle_id: "", pid: 0 },
    };
    isPinned = false;
    await hideContentPanel();
  }

  async function togglePin() {
    isPinned = !isPinned;
    await setContentPanelPinned(isPinned);
  }

  async function handleCopy() {
    await writeText(content.text);
  }

  async function handleRegenerate() {
    // TODO: implement regeneration
    console.log("重新生成:", content.mode);
  }

  async function handleContinue() {
    // TODO: implement follow-up ask
    console.log("继续问");
  }

  async function handleModeChange(newMode: "show" | "translate" | "ai") {
    content.mode = newMode;
    showModeDropdown = false;
    // TODO: regenerate content on mode switch
    console.log("模式切换为:", newMode);

    if (newMode === "translate" && content.text) {
      await handleTranslate();
    }
  }

  function toggleDropdown() {
    showModeDropdown = !showModeDropdown;
  }

  function handleClickOutside(event: MouseEvent) {
    const target = event.target as HTMLElement;
    if (!target.closest(".mode-dropdown")) {
      showModeDropdown = false;
    }
  }

  // Constrains the model to JSON matching parseTranslationResponse's contract
  // (parse failures fall back to plain text, never hard-fail).
  const TRANSLATION_PROMPT =
    'You are a translation assistant. Translate the user\'s input between Chinese and English (auto-detect the source and translate to the other language). Reply with ONLY a JSON object and no other text: {"translation": "<translated text>", "targetLanguage": "<zh|en>", "phonetic": "<pronunciation, or null>", "explanation": "<brief usage note in Chinese, or null>"}';

  /**
   * Get the translation session: prefer the agent picked in settings
   * (quickTools.translationAgentId), falling back to builtin-chat plus the
   * hardcoded translation prompt.
   *
   * The cached sessionId is reused only while its creating agent matches the
   * current config — switching the translation agent rebuilds the session and
   * writes it back to settings (this is the only creation point). The model is
   * always the quick-action default (instantiation must pass a model
   * explicitly); returns null when no model is available.
   */
  async function getOrCreateTranslationSession(): Promise<string | null> {
    try {
      // Settings may have changed in the main window (different agent/model):
      // force-refresh this window's snapshot before deciding reuse vs rebuild.
      await settingsState.loadSettings(true);

      const configuredAgentId =
        settingsState.settings?.quickTools?.translationAgentId ?? null;
      const cached = settingsState.settings?.translation;
      if (cached?.sessionId && (cached.agentId ?? null) === configuredAgentId) {
        return cached.sessionId;
      }

      if (getAllModels().length === 0) {
        await providerActions.loadProvidersWithModels(false);
      }
      const resolved = resolveAgentDefaultModel(
        settingsState.settings?.agent,
        getAllModels(),
      );
      if (!resolved.available) {
        return null;
      }

      const session = await createSessionFromDefinition(
        configuredAgentId ?? "builtin-chat",
        {
          modelId: resolved.modelId,
          providerId: resolved.providerId,
        },
      );
      if (!configuredAgentId) {
        // Builtin fallback: the JSON output contract rides on the session's
        // system prompt; failure doesn't block (plain-text parse fallback). A
        // picked agent uses its own system prompt / GenUI config.
        try {
          await updateAgentSessionField(
            session.id,
            "systemPrompt",
            TRANSLATION_PROMPT,
          );
        } catch (error) {
          console.warn("Failed to set translation system prompt:", error);
        }
      }
      await settingsState.updateSettings({
        section: "translation",
        data: { sessionId: session.id, agentId: configuredAgentId },
      });
      return session.id;
    } catch (error) {
      console.error("Failed to get translation session:", error);
      return null;
    }
  }

  function parseTranslationResponse(
    content: string,
    term: string,
  ): TranslationResult {
    try {
      const jsonMatch = content.match(/\{[\s\S]*\}/);
      if (jsonMatch) {
        const parsed = JSON.parse(jsonMatch[0]);
        return {
          term,
          translation: parsed.translation || content,
          targetLanguage: parsed.targetLanguage || "unknown",
          phonetic: parsed.phonetic || null,
          explanation: parsed.explanation || null,
        };
      }

      return {
        term,
        translation: content,
        targetLanguage: "unknown",
        phonetic: null,
        explanation: null,
      };
    } catch (error) {
      console.error("Failed to parse translation response:", error);
      return {
        term,
        translation: content,
        targetLanguage: "unknown",
        phonetic: null,
        explanation: null,
      };
    }
  }

  async function handleTranslate() {
    if (!content.text || translation.isLoading) return;

    const sessionId = await getOrCreateTranslationSession();
    if (!sessionId) {
      translation.error = t("selection.translationConfigHint");
      return;
    }

    translation.isLoading = true;
    translation.error = null;
    translation.result = null;
    translation.spec = null;

    const term = content.text;
    try {
      // Single turn: plain-text deltas stream into the result preview; spec-
      // shaped streams keep the loading state instead of rendering raw JSON.
      // Afterwards try GenUI spec first, else structured translation parse.
      const finalContent = await runAgentTextTurn(sessionId, term, (partial) => {
        if (looksLikeStreamingSpec(partial)) return;
        translation.result = {
          term,
          translation: partial,
          targetLanguage: "unknown",
          phonetic: null,
          explanation: null,
        };
      });
      const spec = resolveSpec(finalContent);
      if (spec) {
        translation.result = null;
        translation.spec = spec;
      } else {
        translation.result = parseTranslationResponse(finalContent, term);
      }
    } catch (error) {
      console.error("Translation error:", error);
      translation.error = t("selection.translationFailed");
      // The cached session may be stale (deleted, etc.): clear the binding so
      // retranslate rebuilds it.
      try {
        await settingsState.updateSettings({
          section: "translation",
          data: { sessionId: null, agentId: null },
        });
      } catch (clearError) {
        console.warn("Failed to clear stale translation session:", clearError);
      }
    } finally {
      translation.isLoading = false;
    }
  }
</script>

<svelte:window onclick={handleClickOutside} />

<div
  class="flex flex-col w-full h-full bg-[var(--bg-card)] rounded-xl shadow-lg border border-[var(--hairline)] overflow-hidden"
>
  {#if content.mode && modeConfig[content.mode]}
    {@const config = modeConfig[content.mode]}
    <div
      class="flex items-center justify-between px-3 py-2 border-b border-base-300 cursor-move"
      data-tauri-drag-region
    >
      <div class="mode-dropdown relative">
        <button
          class="flex items-center gap-1.5 px-2 py-1.5 rounded-lg hover:bg-base-300 transition-colors {config.color}"
          onclick={toggleDropdown}
        >
          <config.icon class="size-4" />
          <span class="text-sm font-medium">{config.label}</span>
          <ChevronDown class="size-3.5 opacity-60" />
        </button>

        {#if showModeDropdown}
          <div
            class="absolute top-full left-0 mt-1 bg-[var(--bg-card)] rounded-lg shadow-lg border border-[var(--hairline)] py-1 min-w-[120px] z-50"
          >
            {#each Object.entries(modeConfig) as [key, value]}
              {@const isActive = key === content.mode}
              <button
                class="flex items-center gap-2 w-full px-3 py-2 text-sm hover:bg-base-300 transition-colors {isActive
                  ? 'bg-base-300'
                  : ''}"
                class:text-primary={isActive}
                class:text-base-content={!isActive}
                onclick={() =>
                  handleModeChange(key as "show" | "translate" | "ai")}
              >
                <value.icon class="size-4" />
                <span>{value.label}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="flex items-center gap-1">
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-base-300 transition-colors {isPinned
            ? 'text-primary'
            : 'text-base-content/50 hover:text-base-content'}"
          onclick={togglePin}
          title={isPinned ? t("selection.unpin") : t("selection.pin")}
        >
          {#if isPinned}
            <Pin class="size-3.5" />
          {:else}
            <PinOff class="size-3.5" />
          {/if}
        </button>
        <button
          class="flex items-center justify-center w-6 h-6 rounded-full hover:bg-base-300 text-base-content/50 hover:text-base-content transition-colors"
          onclick={handleClose}
        >
          <X class="size-4" />
        </button>
      </div>
    </div>
  {/if}

  <!-- Selection results are content: translation and source must be selectable. -->
  <div class="flex-1 p-3 overflow-auto min-h-0 select-text">
    {#if content.mode === "translate"}
      <!-- While streaming, plain-text deltas preview via result; spec-shaped streams keep the loading state -->
      {#if translation.isLoading && !translation.result}
        <div class="flex items-center justify-center py-8">
          <Spinner size={28} />
          <span class="ml-2 text-sm text-base-content/60">{t("selection.translating")}</span>
        </div>
      {:else if translation.error}
        <div class="p-3 rounded-lg bg-error/10 text-error text-sm">
          {translation.error}
        </div>
      {:else if translation.spec}
        <!-- GenUI output: the whole reply is a valid JSON-Render spec, rendered as a card -->
        <JsonUIProvider initialState={{}}>
          <Renderer spec={translation.spec} registry={uiRegistry} />
        </JsonUIProvider>
      {:else if translation.result}
        <div class="space-y-3">
          <div class="p-2 rounded-lg bg-base-300">
            <div class="flex items-center gap-2">
              <span
                class="text-sm text-base-content whitespace-pre-wrap break-words font-medium"
              >
                {translation.result.translation}
              </span>
              {#if translation.result.phonetic}
                <span class="text-xs text-base-content/50">
                  [{translation.result.phonetic}]
                </span>
              {/if}
            </div>

            {#if translation.result.explanation}
              <p class="text-xs text-base-content/70 mt-1">
                {translation.result.explanation}
              </p>
            {/if}
          </div>
        </div>
      {:else}
        <p class="text-sm text-base-content/40 text-center py-4">{t("selection.waitingTranslation")}</p>
      {/if}
    {:else if content.text}
      <p
        class="text-sm text-base-content whitespace-pre-wrap break-words leading-relaxed"
      >
        {content.text}
      </p>
    {:else}
      <p class="text-sm text-base-content/40 text-center py-4">{t("selection.noContent")}</p>
    {/if}
  </div>

  <div
    class="flex items-center justify-between px-3 py-1.5 border-t border-[var(--hairline)] bg-base-300/60"
  >
    <div class="flex items-center gap-1">
      <button
        class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
        onclick={handleCopy}
        title={t("common.copy")}
        disabled={!content.text}
      >
        <Copy class="size-3.5" />
      </button>
      {#if content.mode === "translate"}
        <button
          class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          onclick={handleTranslate}
          title={t("selection.retranslate")}
          disabled={!content.text || translation.isLoading}
        >
          <RotateCcw class="size-3.5" />
        </button>
      {:else}
        <button
          class="flex items-center justify-center w-7 h-7 text-base-content/60 hover:text-base-content hover:bg-base-300/50 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
          onclick={handleRegenerate}
          title={t("selection.regenerate")}
          disabled={!content.text}
        >
          <RotateCcw class="size-3.5" />
        </button>
      {/if}
    </div>

    <button
      class="flex items-center px-2 py-1 text-xs font-medium text-primary hover:bg-primary/10 rounded-lg transition-colors disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:bg-transparent"
      onclick={handleContinue}
      title={t("selection.continueAsk")}
      disabled={!content.text}
    >
      <MessageCirclePlus class="size-3.5" />
      <span>{t("selection.continueAsk")}</span>
    </button>
  </div>
</div>
