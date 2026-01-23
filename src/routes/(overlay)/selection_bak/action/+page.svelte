<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";
  import {
    ChevronDown,
    Copy,
    Languages,
    MessageCircle,
    MoreVertical,
    Pin,
    RotateCcw,
    Search,
    Volume2,
    X as CloseIcon,
    Zap,
  } from "@lucide/svelte";
  import { translateWord } from "$lib/api/word";
  import { showAppError } from "$lib/utils/error";

  type PanelMode = "translate" | "ask" | "selection" | "lookup";

  let selectedText = $state("");
  let panelMode = $state<PanelMode>("translate");
  let isTranslating = $state(false);
  let translateResult = $state<{
    translation: string;
    targetLanguage: string;
    phonetic?: string | null;
    explanation?: string | null;
  } | null>(null);
  let translateError = $state<string | null>(null);
  let overlayWebview = $state<ReturnType<typeof getCurrentWebview> | null>(
    null,
  );
  let unlistenModeChange: (() => void) | null = null;

  const panelModeOptions: { value: PanelMode; label: string }[] = [
    { value: "translate", label: "翻译" },
    { value: "ask", label: "问AI" },
    { value: "selection", label: "选区" },
    { value: "lookup", label: "查询" },
  ];

  const panelModeIcons = {
    translate: Languages,
    ask: MessageCircle,
    selection: Search,
    lookup: Search,
  } as const;
  const ActivePanelIcon = $derived(panelModeIcons[panelMode]);

  onMount(() => {
    overlayWebview = getCurrentWebview();

    if (overlayWebview) {
      void overlayWebview
        .listen<{ mode: PanelMode; text: string }>("mode_change", (event) => {
          if (!event.payload) return;
          handleModeChange(event.payload.mode, event.payload.text);
        })
        .then((unlisten) => {
          unlistenModeChange = unlisten;
        })
        .catch((error) =>
          console.error("Failed to listen for mode changes:", error),
        );
    }

    return () => {
      unlistenModeChange?.();
      unlistenModeChange = null;
      overlayWebview = null;
    };
  });

  function handleModeChange(mode: PanelMode, text: string) {
    panelMode = mode;
    selectedText = text;
    resetTranslationState();

    if (mode === "translate" && text) {
      void runTranslation(text);
    }
  }

  function resetTranslationState() {
    translateResult = null;
    translateError = null;
    isTranslating = false;
  }

  async function runTranslation(term: string) {
    translateResult = null;
    translateError = null;
    isTranslating = true;

    try {
      const response = await translateWord({ term });
      translateResult = response;
    } catch (error) {
      console.error("Failed to translate selection:", error);
      const normalized = showAppError(error, { fallbackMessage: "翻译失败" });
      translateError = normalized.message;
    } finally {
      isTranslating = false;
    }
  }

  async function closePanel() {
    try {
      await invoke("selection_hide_action_panel");
    } catch (error) {
      console.error("Failed to hide action panel:", error);
    }
  }

  function handlePanelModeSelect(nextMode: PanelMode) {
    if (panelMode === nextMode) return;
    panelMode = nextMode;
    if (panelMode === "translate" && selectedText) {
      void runTranslation(selectedText);
    } else {
      resetTranslationState();
    }
  }
</script>

<svelte:head>
  <style>
    :global(html),
    :global(body) {
      background: transparent !important;
      height: auto !important;
      min-height: 100% !important;
      overflow: visible !important;
    }
  </style>
</svelte:head>

<div
  class="w-[520px] min-h-[220px] max-h-[420px] rounded-[24px] bg-white border border-slate-200 px-4 py-3 flex flex-col gap-3 overflow-hidden"
  role="dialog"
  aria-label="操作面板"
>
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-2 text-slate-700">
      <ActivePanelIcon size={18} />
      <div class="relative">
        <select
          class="appearance-none bg-transparent text-[15px] font-semibold text-slate-800 pr-5 focus:outline-none"
          value={panelMode}
          onchange={(event) =>
            handlePanelModeSelect(
              (event.currentTarget as HTMLSelectElement).value as PanelMode,
            )}
        >
          {#each panelModeOptions as option}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
        <ChevronDown
          size={14}
          class="pointer-events-none absolute right-0 top-1/2 -translate-y-1/2 text-slate-400"
        />
      </div>
    </div>
    <div class="flex items-center gap-2 text-slate-500">
      <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="快捷操作">
        <Zap size={16} />
      </button>
      <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="固定面板">
        <Pin size={16} />
      </button>
      <button
        class="p-1.5 rounded-lg hover:bg-slate-100"
        onclick={closePanel}
        aria-label="关闭面板"
      >
        <CloseIcon size={16} />
      </button>
    </div>
  </div>

  <!-- Body -->
  <div class="flex-1 min-h-0 overflow-auto pr-1">
    <div>
      {#if panelMode === "translate"}
        <div class="text-[11px] text-slate-500 truncate">
          {selectedText}
        </div>
        <div class="mt-3 text-slate-900 text-lg font-semibold">
          {#if isTranslating}
            <div class="flex items-center gap-2 text-slate-500 text-sm">
              <div
                class="w-3 h-3 border border-t-transparent rounded-full animate-spin"
              ></div>
              <span>翻译中…</span>
            </div>
          {:else if translateError}
            <div class="text-red-600 text-sm">{translateError}</div>
          {:else if translateResult}
            <div class="text-slate-900 text-2xl font-semibold leading-tight">
              {translateResult.translation}
            </div>
            {#if translateResult.phonetic}
              <div class="mt-2 text-[12px] text-slate-500">
                [{translateResult.phonetic}]
              </div>
            {/if}
            {#if translateResult.explanation}
              <div class="mt-2 text-[12px] text-slate-500">
                {translateResult.explanation}
              </div>
            {/if}
          {:else}
            <div class="text-slate-500 text-sm">暂无翻译结果</div>
          {/if}
        </div>
      {:else if panelMode === "ask"}
        <div class="text-[11px] text-slate-500 truncate">
          {selectedText}
        </div>
        <div class="mt-3 text-sm text-slate-500">聊天功能暂未接入。</div>
      {:else if panelMode === "selection"}
        <div class="mt-3 text-sm text-slate-800 whitespace-pre-wrap">
          {selectedText || "暂无内容"}
        </div>
      {:else}
        <div class="text-sm text-slate-500">查询功能暂未接入。</div>
      {/if}
    </div>
  </div>

  <!-- Footer -->
  <div
    class="flex items-center justify-between border-t border-slate-200 pt-2 text-slate-500"
  >
    <div class="flex items-center gap-1.5">
      <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="复制">
        <Copy size={14} />
      </button>
      <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="重试">
        <RotateCcw size={14} />
      </button>
      <button class="p-1.5 rounded-lg hover:bg-slate-100" aria-label="朗读">
        <Volume2 size={14} />
      </button>
    </div>
    {#if panelMode === "translate" && translateResult}
      <div class="text-[11px]">
        目标语言: {translateResult.targetLanguage}
      </div>
    {/if}
  </div>
</div>
