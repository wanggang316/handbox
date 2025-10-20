<script lang="ts">
  import { chatState, chatActions, currentChatModel } from "$lib/states/chat.svelte";
  import type { ModelWithProvider } from "$lib/types/provider";

  import TableGroup from "../../ui/table/TableGroup.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";
  import DefaultRow from "../../ui/table/DefaultRow.svelte";
  import LabeledSliderRow from "../../ui/table/LabeledSliderRow.svelte";
  import NumberStepperRow from "../../ui/table/NumberStepperRow.svelte";
  import SwitchRow from "../../ui/table/SwitchRow.svelte";
  import RoundButton from "../../ui/RoundButton.svelte";
  import ChatModelSelectModal from "../ChatModelSelectModal.svelte";

  type SaveStatus = "saved" | "saving" | "error";

  const currentModel = $derived<ModelWithProvider | undefined>(currentChatModel().model);

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
      ...(Array.isArray(model.support_parameters) ? model.support_parameters : []),
      ...(Array.isArray(model.supported_parameters) ? model.supported_parameters : []),
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

  function ensureNumber(value: number | null | undefined, fallback: number): number {
    return typeof value === "number" && Number.isFinite(value) ? value : fallback;
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
      topK: hasSupport("top_k") ? Math.max(getDefaultNumber("top_k", 40), 1) : 0,
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
    const baseline = Math.max(getMaxNumber("top_k", modelDefaults.topK || 100), modelDefaults.topK || 100, 1);
    return Math.max(baseline, current ?? 1);
  }

  function resolveOutputTokensMax(current: number): number {
    const baseline = Math.max(
      getMaxNumber("output_max_tokens", getMaxNumber("max_tokens", modelDefaults.maxTokens)),
      modelDefaults.maxTokens,
      1,
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
      defaults.maxTokens,
    );

    const temperature = clamp(
      ensureNumber(chat?.temperature, defaults.temperature),
      0,
      temperatureMax,
    );

    const topP = clamp(ensureNumber(chat?.topP, defaults.topP), 0, topPMax);
    const topK = hasSupport("top_k")
      ? clamp(ensureNumber(chat?.topK, defaults.topK), 1, topKMax)
      : defaults.topK;
    const maxTokens = clamp(
      ensureNumber(chat?.maxTokens, defaults.maxTokens),
      1,
      maxTokensLimit,
    );

    const streamResponse = hasSupport("streaming")
      ? (typeof chat?.stream === "boolean" ? chat?.stream : defaults.streamResponse)
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

  $effect(() => {
    // 监听模型或聊天配置变化，刷新本地缓存
    currentModel;
    chatState.currentChat;
    const next = buildInitialSettings();
    currentSettings = { ...next };
    originalSettings = { ...next };
    saveStatus = "saved";
  });

  $effect(() => {
    const hasChanges = JSON.stringify(currentSettings) !== JSON.stringify(originalSettings);
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

<div class="flex-1 p-0 space-y-6">
  <TableGroup title="模型">
    <DefaultRow
      label="当前模型"
      value={currentModel ? currentModel.name : "选择模型"}
      onclick={() => (showModelModal = true)}
    />

    <TableBaseRow label="供应商">
      <span class="text-sm text-base-content/70">
        {currentModel ? currentModel.providerName : "未选择"}
      </span>
    </TableBaseRow>

    {#if currentModel?.id}
      <TableBaseRow label="模型 ID">
        <span class="text-xs font-mono text-base-content/60 break-all">
          {currentModel.id}
        </span>
      </TableBaseRow>
    {/if}
  </TableGroup>

  <TableGroup title="模型参数">
    <LabeledSliderRow
      label="Temperature"
      bind:value={currentSettings.temperature}
      min={0}
      max={resolveTemperatureMax(currentSettings.temperature)}
      step={0.1}
      leftLabel="精确"
      rightLabel="创意"
      scaleMarks={getTemperatureScaleMarks()}
    />

    {#if hasSupport("top_p")}
      <LabeledSliderRow
        label="Top-p"
        bind:value={currentSettings.topP}
        min={0}
        max={resolveTopPMax(currentSettings.topP)}
        step={0.05}
        leftLabel="聚焦"
        rightLabel="多样"
        scaleMarks={getTopPScaleMarks()}
      />
    {/if}

    {#if hasSupport("top_k")}
      <NumberStepperRow
        label="Top-k"
        bind:value={currentSettings.topK}
        defaultValue={modelDefaults.topK}
        placeholder="{modelDefaults.topK} (默认)"
        min={1}
        max={resolveTopKMax(currentSettings.topK)}
        step={1}
      />
    {/if}

    {#if hasSupport("output_max_tokens")}
      <NumberStepperRow
        label="输出最大 Token"
        bind:value={currentSettings.maxTokens}
        defaultValue={modelDefaults.maxTokens}
        placeholder="{modelDefaults.maxTokens} (默认)"
        min={1}
        max={resolveOutputTokensMax(currentSettings.maxTokens)}
        step={100}
      />
    {/if}

    {#if hasSupport("streaming")}
      <SwitchRow label="流式输出" bind:checked={currentSettings.streamResponse} />
    {/if}
  </TableGroup>

  <div class="flex gap-3 pt-4 items-center justify-between">
    <RoundButton
      customClass="w-24"
      label="恢复默认"
      bgColor="bg-base-200"
      textColor="text-base-content/80"
      hoverColor="hover:text-base-content"
      onclick={handleDefault}
    />

    {#if saveStatus !== "saved"}
      <div class="px-6 py-2">
        <div class="flex items-center gap-2">
          <span
            class="w-2 h-2 rounded-full {saveStatus === 'saving'
              ? 'bg-warning'
              : 'bg-error'}"
          ></span>
          <span class="text-xs text-base-content/70">
            {saveStatus === "saving" ? "保存中..." : "保存失败"}
          </span>
        </div>
      </div>
    {/if}
  </div>
</div>

<ChatModelSelectModal
  bind:open={showModelModal}
  selectedModel={currentModel ?? null}
  onModelSelect={handleModelSelect}
/>
