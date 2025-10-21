<script lang="ts">
  import Button from "$lib/components/ui/Button.svelte";
  import {
    chatState,
    chatActions,
    currentChatModel,
  } from "$lib/states/chat.svelte";
  import { getProviderIconById } from "$lib/states/provider.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";
  import { ChevronsUpDown } from "lucide-svelte";

  import LabeledSlider from "../../ui/LabeledSlider.svelte";
  import NumberStepper from "../../ui/NumberStepper.svelte";
  import Toggle from "../../ui/Toggle.svelte";
  import ChatModelSelectModal from "../ChatModelSelectModal.svelte";

  type SaveStatus = "saved" | "saving" | "error";

  interface Props {
    variant?: "all" | "selection" | "parameters";
  }

  let { variant = "all" }: Props = $props();

  const currentModel = $derived<ModelWithProvider | undefined>(
    currentChatModel().model
  );

  const providerIcon = $derived(
    currentModel?.provider_id
      ? getProviderIconById(currentModel.provider_id)
      : undefined
  );

  const providerInitial = $derived(() => {
    const name = currentModel?.providerName?.trim();
    if (!name) return "M";
    return name[0]?.toUpperCase() ?? "M";
  });

  const showSelection = $derived(variant === "all" || variant === "selection");
  const showParameters = $derived(
    variant === "all" || variant === "parameters"
  );

  const PARAMETER_ALIASES: Record<string, string[]> = {
    temperature: ["temperature"],
    top_p: ["top_p"],
    top_k: ["top_k"],
    streaming: ["streaming", "stream"],
    output_max_tokens: ["output_max_tokens", "max_tokens"],
  };

  function getAliases(name: string): string[] {
    return PARAMETER_ALIASES[name] ?? [name];
  }

  function toNumber(value: unknown): number | null {
    if (typeof value === "number" && Number.isFinite(value)) {
      return value;
    }
    if (typeof value === "string" && value.trim().length > 0) {
      const parsed = Number(value);
      return Number.isFinite(parsed) ? parsed : null;
    }
    return null;
  }

  function toBoolean(value: unknown): boolean | null {
    if (typeof value === "boolean") {
      return value;
    }
    if (typeof value === "string") {
      const normalized = value.trim().toLowerCase();
      if (["true", "1", "yes"].includes(normalized)) return true;
      if (["false", "0", "no"].includes(normalized)) return false;
    }
    return null;
  }

  function getSupportedParameterSet(): Set<string> {
    const model = currentModel;
    const supported = new Set<string>();

    if (!model) {
      return supported;
    }

    const supportList = [
      ...(Array.isArray(model.support_parameters)
        ? model.support_parameters
        : []),
      ...(Array.isArray(model.supported_parameters)
        ? model.supported_parameters
        : []),
    ];

    for (const raw of supportList) {
      if (typeof raw === "string" && raw.trim().length > 0) {
        supported.add(raw.trim());
      }
    }

    if (supported.size === 0 && Array.isArray(model.parameters)) {
      for (const param of model.parameters) {
        const name = typeof param?.name === "string" ? param.name.trim() : "";
        if (name) {
          supported.add(name);
        }
      }
    }

    return supported;
  }

  const supportedParameters = $derived(getSupportedParameterSet());

  function hasSupport(name: string): boolean {
    if (name === "temperature") {
      return true;
    }

    const aliases = getAliases(name);

    if (supportedParameters.size > 0) {
      return aliases.some((alias) => supportedParameters.has(alias));
    }

    const model = currentModel;
    const defaults = model?.default_parameters ?? null;
    const maxes = model?.max_parameters ?? null;

    if (defaults && aliases.some((alias) => alias in defaults)) {
      return true;
    }

    if (maxes && aliases.some((alias) => alias in maxes)) {
      return true;
    }

    return false;
  }

  function getDefaultNumber(name: string, fallback: number): number {
    const model = currentModel;
    const defaults = model?.default_parameters ?? null;

    if (defaults) {
      for (const alias of getAliases(name)) {
        if (alias in defaults) {
          const parsed = toNumber(defaults[alias]);
          if (parsed !== null) {
            return parsed;
          }
        }
      }
    }

    return fallback;
  }

  function getDefaultBoolean(name: string, fallback: boolean): boolean {
    const model = currentModel;
    const defaults = model?.default_parameters ?? null;

    if (defaults) {
      for (const alias of getAliases(name)) {
        if (alias in defaults) {
          const parsed = toBoolean(defaults[alias]);
          if (parsed !== null) {
            return parsed;
          }
        }
      }
    }

    return fallback;
  }

  function getMaxNumber(name: string, fallback: number): number {
    const model = currentModel;
    const maxes = model?.max_parameters ?? null;

    if (maxes) {
      for (const alias of getAliases(name)) {
        if (alias in maxes) {
          const parsed = toNumber(maxes[alias]);
          if (parsed !== null) {
            return parsed;
          }
        }
      }
    }

    return fallback;
  }

  function ensureNumber(
    value: number | null | undefined,
    fallback: number
  ): number {
    return typeof value === "number" && Number.isFinite(value)
      ? value
      : fallback;
  }

  function clamp(value: number, min: number, max: number): number {
    if (!Number.isFinite(value)) return min;
    if (!Number.isFinite(max) || max <= min) return value < min ? min : value;
    return Math.min(Math.max(value, min), max);
  }

  function getModelDefaultSettings() {
    const outputFallback = getDefaultNumber(
      "output_max_tokens",
      getDefaultNumber("max_tokens", 4000)
    );

    return {
      temperature: getDefaultNumber("temperature", 0.7),
      topP: getDefaultNumber("top_p", 1.0),
      topK: hasSupport("top_k")
        ? Math.max(getDefaultNumber("top_k", 40), 1)
        : 0,
      streamResponse: hasSupport("streaming")
        ? getDefaultBoolean("streaming", true)
        : true,
      maxTokens: outputFallback > 0 ? outputFallback : 4000,
    };
  }

  const modelDefaults = $derived(getModelDefaultSettings());

  function resolveTemperatureMax(current: number): number {
    const upper = Math.max(getMaxNumber("temperature", 2), 0.1);
    return Math.max(upper, current ?? 0);
  }

  function getTemperatureScaleMarks() {
    const upper = Math.max(getMaxNumber("temperature", 2), 0.1);
    return [
      { value: 0, position: 0 },
      { value: Number((upper / 2).toFixed(2)), position: 50 },
      { value: upper, position: 100 },
    ];
  }

  function resolveTopPMax(current: number): number {
    const upper = Math.max(getMaxNumber("top_p", 1), 0.1);
    return Math.max(upper, current ?? 0);
  }

  function getTopPScaleMarks() {
    const upper = Math.max(getMaxNumber("top_p", 1), 0.1);
    return [
      { value: 0, position: 0 },
      { value: Number((upper / 2).toFixed(2)), position: 50 },
      { value: upper, position: 100 },
    ];
  }

  function resolveTopKMax(current: number): number {
    if (!hasSupport("top_k")) {
      return current ?? 0;
    }
    const baseline = Math.max(
      getMaxNumber("top_k", modelDefaults.topK || 100),
      modelDefaults.topK || 100,
      1
    );
    return Math.max(baseline, current ?? 1);
  }

  function resolveOutputTokensMax(current: number): number {
    const baseline = Math.max(
      getMaxNumber(
        "output_max_tokens",
        getMaxNumber("max_tokens", modelDefaults.maxTokens)
      ),
      modelDefaults.maxTokens,
      1
    );
    return Math.max(baseline, current ?? baseline);
  }

  function buildInitialSettings() {
    const defaults = modelDefaults;
    const chat = chatState.currentChat;

    const temperatureMax = Math.max(getMaxNumber("temperature", 2), 0.1);
    const topPMax = Math.max(getMaxNumber("top_p", 1), 0.1);
    const topKMax = hasSupport("top_k")
      ? Math.max(getMaxNumber("top_k", defaults.topK || 100), 1)
      : defaults.topK || 0;
    const maxTokensLimit = Math.max(
      getMaxNumber("output_max_tokens", getMaxNumber("max_tokens", 1000000)),
      defaults.maxTokens
    );

    const temperature = clamp(
      ensureNumber(chat?.temperature, defaults.temperature),
      0,
      temperatureMax
    );

    const topP = clamp(ensureNumber(chat?.topP, defaults.topP), 0, topPMax);
    const topK = hasSupport("top_k")
      ? clamp(ensureNumber(chat?.topK, defaults.topK), 1, topKMax)
      : defaults.topK;
    const maxTokens = clamp(
      ensureNumber(chat?.maxTokens, defaults.maxTokens),
      1,
      maxTokensLimit
    );

    const streamResponse = hasSupport("streaming")
      ? typeof chat?.stream === "boolean"
        ? chat?.stream
        : defaults.streamResponse
      : defaults.streamResponse;

    return {
      temperature,
      topP,
      topK,
      streamResponse,
      maxTokens,
    };
  }

  let currentSettings = $state(buildInitialSettings());
  let originalSettings = $state(buildInitialSettings());
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");
  let showModelModal = $state(false);
  let showAdvanced = $state(false);

  const hasAdvancedParameters = $derived(
    hasSupport("top_p") ||
      hasSupport("top_k") ||
      hasSupport("output_max_tokens") ||
      hasSupport("streaming")
  );

  $effect(() => {
    // 监听模型或聊天配置变化，刷新本地缓存
    currentModel;
    chatState.currentChat;
    const next = buildInitialSettings();
    currentSettings = { ...next };
    originalSettings = { ...next };
    saveStatus = "saved";
    showAdvanced = false;
  });

  $effect(() => {
    const hasChanges =
      JSON.stringify(currentSettings) !== JSON.stringify(originalSettings);
    if (!hasChanges) {
      return;
    }

    saveStatus = "saving";
    if (saveTimer) {
      clearTimeout(saveTimer);
    }

    saveTimer = setTimeout(async () => {
      try {
        const payload: {
          temperature: number;
          topP?: number;
          topK?: number;
          stream?: boolean;
          maxTokens?: number;
        } = {
          temperature: currentSettings.temperature,
        };

        if (hasSupport("top_p")) {
          payload.topP = currentSettings.topP;
        }

        if (hasSupport("top_k")) {
          payload.topK = currentSettings.topK;
        }

        if (hasSupport("streaming")) {
          payload.stream = currentSettings.streamResponse;
        }

        if (hasSupport("output_max_tokens")) {
          payload.maxTokens = currentSettings.maxTokens;
        }

        await chatActions.updateModelSettings(payload);

        originalSettings = { ...currentSettings };
        saveStatus = "saved";
      } catch (error) {
        console.error("Failed to update model settings:", error);
        saveStatus = "error";
      }
    }, 500);
  });

  function handleDefault() {
    const defaults = modelDefaults;
    currentSettings = {
      ...currentSettings,
      temperature: defaults.temperature,
      topP: defaults.topP,
      topK: defaults.topK,
      streamResponse: defaults.streamResponse,
      maxTokens: defaults.maxTokens,
    };
  }

  function handleModelSelect(model: ModelWithProvider) {
    chatActions.updateChatModel(model.id, model.provider_id);
    showModelModal = false;
  }
</script>

{#if showSelection}
  <button
    class="w-full rounded-2xl bg-base-100 px-3 py-4 border border-base-200 hover:bg-base-300"
    type="button"
    onclick={() => (showModelModal = true)}
  >
    {#if currentModel}
      <div class="flex items-start justify-between gap-2">
        <div class=" flex items-center gap-2">
          <div class="flex-1 flex justify-center items-center">
            <img
              src={providerIcon}
              alt={currentModel?.providerName ?? "模型供应商"}
              class="h-8 w-8 rounded-md object-contain p-0"
            />
          </div>

          <div class="space-y-1 pb-1">
            <div class="text-md text-base-content">
              {currentModel ? currentModel.name : "未选择模型"}
            </div>
            <div
              class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-base-content/60"
            >
              {#if currentModel?.id}
                <span class="font-mono text-[11px] text-base-content/50">
                  {currentModel.id}
                </span>
              {/if}
            </div>
          </div>
        </div>
      </div>
      {#if currentModel.description}
        <p class="mt-4 text-left text-xs leading-relaxed text-base-content/60">
          {currentModel.description}
        </p>
      {/if}
    {:else}
      <div class="flex flex-row justify-between items-center px-2">
        <p class="text-left text-sm leading-relaxed text-base-content">
          选择一个模型以开始对话
        </p>
        <ChevronsUpDown size={14} />
      </div>
    {/if}
  </button>
{/if}

{#if showParameters}
  <div
    class="space-y-4 rounded-2xl border border-base-200 bg-base-100 px-5 py-5 shadow-sm"
  >
    <div class="space-y-4">
      <div class="flex items-start justify-between gap-4">
        <div class="space-y-1">
          <h3 class="text-sm font-semibold text-base-content">Temperature</h3>
        </div>
        <span
          class="rounded-full bg-base-200 px-3 py-1 font-mono text-sm text-base-content/80"
        >
          {currentSettings.temperature.toFixed(1)}
        </span>
      </div>

      <LabeledSlider
        bind:value={currentSettings.temperature}
        min={0}
        max={resolveTemperatureMax(currentSettings.temperature)}
        step={0.1}
        leftLabel="精确"
        rightLabel="创意"
        scaleMarks={getTemperatureScaleMarks()}
        showValue={false}
      />
    </div>

    {#if hasAdvancedParameters}
      <div class="flex items-center justify-end">
        <button
          type="button"
          class="flex items-center gap-1 rounded-full border border-base-300 px-3 py-1 text-xs font-medium text-base-content/70 hover:border-primary/50 hover:text-primary transition-colors"
          onclick={() => (showAdvanced = !showAdvanced)}
        >
          {showAdvanced ? "收起高级" : "高级"}
        </button>
      </div>
    {/if}

    {#if showAdvanced}
      <div
        class="space-y-4 rounded-xl border border-dashed border-base-200 bg-base-50/50 px-4 py-4"
      >
        {#if hasSupport("top_p")}
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <div>
                <p class="text-sm font-medium text-base-content">Top-p</p>
                <p class="text-xs text-base-content/60">
                  限制采样概率分布的累积和。
                </p>
              </div>
              <span
                class="rounded bg-base-200 px-2 py-1 font-mono text-xs text-base-content/70"
              >
                {currentSettings.topP.toFixed(2)}
              </span>
            </div>
            <LabeledSlider
              bind:value={currentSettings.topP}
              min={0}
              max={resolveTopPMax(currentSettings.topP)}
              step={0.05}
              leftLabel="聚焦"
              rightLabel="多样"
              scaleMarks={getTopPScaleMarks()}
              showValue={false}
            />
          </div>
        {/if}

        {#if hasSupport("top_k")}
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <p class="text-sm font-medium text-base-content">Top-k</p>
              <span
                class="rounded bg-base-200 px-2 py-1 font-mono text-xs text-base-content/70"
              >
                {currentSettings.topK}
              </span>
            </div>
            <NumberStepper
              bind:value={currentSettings.topK}
              min={1}
              max={resolveTopKMax(currentSettings.topK)}
              step={1}
              defaultValue={modelDefaults.topK}
              placeholder={`${modelDefaults.topK} (默认)`}
            />
          </div>
        {/if}

        {#if hasSupport("output_max_tokens")}
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <p class="text-sm font-medium text-base-content">
                输出最大 Token
              </p>
              <span
                class="rounded bg-base-200 px-2 py-1 font-mono text-xs text-base-content/70"
              >
                {currentSettings.maxTokens}
              </span>
            </div>
            <NumberStepper
              bind:value={currentSettings.maxTokens}
              min={1}
              max={resolveOutputTokensMax(currentSettings.maxTokens)}
              step={100}
              defaultValue={modelDefaults.maxTokens}
              placeholder={`${modelDefaults.maxTokens} (默认)`}
            />
          </div>
        {/if}

        {#if hasSupport("streaming")}
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium text-base-content">流式输出</p>
              <p class="text-xs text-base-content/60">
                实时获取模型响应，适合长文本输出。
              </p>
            </div>
            <Toggle bind:checked={currentSettings.streamResponse} />
          </div>
        {/if}
      </div>
    {/if}

    <div class="flex items-center justify-between pt-2">
      <!-- <button
        type="button"
        class="rounded-full border border-base-300 px-4 py-2 text-sm font-medium text-base-content hover:border-primary/50 hover:bg-primary/10 transition-colors"
        onclick={handleDefault}
      >
        恢复默认
      </button> -->

      {#if saveStatus !== "saved"}
        <div class="flex items-center gap-2 text-xs text-base-content/70">
          <span
            class="h-2 w-2 rounded-full {saveStatus === 'saving'
              ? 'bg-warning'
              : 'bg-error'}"
          ></span>
          <span>
            {saveStatus === "saving" ? "保存中..." : "保存失败"}
          </span>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if showSelection}
  <ChatModelSelectModal
    bind:open={showModelModal}
    selectedModel={currentModel ?? null}
    onModelSelect={handleModelSelect}
  />
{/if}
