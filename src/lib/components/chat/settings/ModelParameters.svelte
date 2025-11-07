<script lang="ts">
  import {
    chatState,
    chatActions,
    currentChatModel,
    hasParameterSupport,
    getModelDefaultSettings,
    getMaxNumber,
    ensureNumber,
    clamp,
  } from "$lib/states/chat.svelte";
  import LabeledSlider from "../../ui/LabeledSlider.svelte";
  import Toggle from "../../ui/Toggle.svelte";
  import NumberStepperRow from "../../ui/table/NumberStepperRow.svelte";
  import ModelSliderParameterRow from "./ModelSliderParameterRow.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";
  import TableGroup from "../../ui/table/TableGroup.svelte";

  type SaveStatus = "saved" | "saving" | "error";

  const currentModel = $derived(currentChatModel().model);

const modelDefaults = $derived(getModelDefaultSettings(currentModel));
const supportsTemperature = $derived(
  hasParameterSupport("temperature", currentModel)
);

  const hasAdvancedParameters = $derived(
    hasParameterSupport("top_p", currentModel) ||
      hasParameterSupport("top_k", currentModel) ||
      hasParameterSupport("output_max_tokens", currentModel) ||
      hasParameterSupport("streaming", currentModel)
  );

  function resolveTemperatureMax(current: number): number {
    const upper = Math.max(getMaxNumber("temperature", 2, currentModel), 0.1);
    return Math.max(upper, current ?? 0);
  }

  function getTemperatureScaleMarks() {
    const upper = Math.max(getMaxNumber("temperature", 2, currentModel), 0.1);
    return [
      { value: 0, position: 0 },
      { value: Number((upper / 2).toFixed(2)), position: 50 },
      { value: upper, position: 100 },
    ];
  }

  function resolveTopPMax(current: number): number {
    const upper = Math.max(getMaxNumber("top_p", 1, currentModel), 0.1);
    return Math.max(upper, current ?? 0);
  }

  function getTopPScaleMarks() {
    const upper = Math.max(getMaxNumber("top_p", 1, currentModel), 0.1);
    return [
      { value: 0, position: 0 },
      { value: Number((upper / 2).toFixed(2)), position: 50 },
      { value: upper, position: 100 },
    ];
  }

  function resolveTopKMax(current: number | undefined): number {
    if (!hasParameterSupport("top_k", currentModel)) {
      return current ?? 0;
    }
    const defaultTopK =
      typeof modelDefaults.topK === "number" && Number.isFinite(modelDefaults.topK)
        ? Math.max(modelDefaults.topK, 1)
        : 1;
    const baseline = Math.max(
      getMaxNumber("top_k", defaultTopK, currentModel),
      defaultTopK,
      1
    );
    const currentValue =
      typeof current === "number" && Number.isFinite(current) ? current : defaultTopK;
    return Math.max(baseline, currentValue);
  }

  function getTopKScaleMarks() {
    const upper = resolveTopKMax(currentSettings.topK);
    return [
      { value: 1, position: 0 },
      { value: Math.floor(upper / 2), position: 50 },
      { value: upper, position: 100 },
    ];
  }

  function resolveOutputTokensMax(current: number): number {
    const defaultMaxTokens =
      typeof modelDefaults.maxTokens === "number" && Number.isFinite(modelDefaults.maxTokens)
        ? Math.max(modelDefaults.maxTokens, 1)
        : 1;
    const baseline = Math.max(
      getMaxNumber(
        "output_max_tokens",
        getMaxNumber("max_tokens", defaultMaxTokens, currentModel),
        currentModel
      ),
      defaultMaxTokens,
      1
    );
    const currentValue =
      typeof current === "number" && Number.isFinite(current) ? current : baseline;
    return Math.max(baseline, currentValue);
  }

  function buildInitialSettings() {
    const defaults = modelDefaults;
    const chat = chatState.currentChat;

    const temperatureMax = Math.max(
      getMaxNumber("temperature", 2, currentModel),
      0.1
    );
    const topPMax = Math.max(getMaxNumber("top_p", 1, currentModel), 0.1);
    const defaultTopK =
      typeof defaults.topK === "number" && Number.isFinite(defaults.topK)
        ? Math.max(defaults.topK, 1)
        : 1;
    const topKMax = hasParameterSupport("top_k", currentModel)
      ? Math.max(getMaxNumber("top_k", defaultTopK, currentModel), defaultTopK, 1)
      : undefined;
    const maxTokensLimit = Math.max(
      getMaxNumber(
        "output_max_tokens",
        getMaxNumber(
          "max_tokens",
          typeof defaults.maxTokens === "number" && Number.isFinite(defaults.maxTokens)
            ? Math.max(defaults.maxTokens, 1)
            : 1,
          currentModel
        ),
        currentModel
      ),
      typeof defaults.maxTokens === "number" && Number.isFinite(defaults.maxTokens)
        ? Math.max(defaults.maxTokens, 1)
        : 1
    );

    const temperatureFallback =
      typeof defaults.temperature === "number" && Number.isFinite(defaults.temperature)
        ? defaults.temperature
        : 0;
    const temperature = clamp(
      ensureNumber(chat?.temperature, temperatureFallback),
      0,
      temperatureMax
    );

    const topPFallback =
      typeof defaults.topP === "number" && Number.isFinite(defaults.topP) ? defaults.topP : 0;
    const topP = clamp(ensureNumber(chat?.topP, topPFallback), 0, topPMax);
    const topK = hasParameterSupport("top_k", currentModel)
      ? clamp(ensureNumber(chat?.topK, defaultTopK), 1, topKMax ?? defaultTopK)
      : undefined;

    const maxTokensFallback =
      typeof defaults.maxTokens === "number" && Number.isFinite(defaults.maxTokens)
        ? Math.max(defaults.maxTokens, 1)
        : 1;
    const maxTokens = clamp(
      ensureNumber(chat?.maxTokens, maxTokensFallback),
      1,
      maxTokensLimit
    );

    const streamResponse = hasParameterSupport("streaming", currentModel)
      ? typeof chat?.stream === "boolean"
        ? chat?.stream
        : (typeof defaults.streamResponse === "boolean" ? defaults.streamResponse : true)
      : typeof defaults.streamResponse === "boolean"
        ? defaults.streamResponse
        : true;

    return {
      temperature,
      topP,
      topK,
      streamResponse,
      maxTokens,
      enableTopP: chat?.topP !== undefined && chat?.topP !== null,
      enableTopK: chat?.topK !== undefined && chat?.topK !== null,
    };
  }

  let currentSettings = $state(buildInitialSettings());
  let originalSettings = $state(buildInitialSettings());
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");

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
    supportsTemperature;
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
          temperature?: number;
          topP?: number;
          topK?: number;
          stream?: boolean;
          maxTokens?: number;
        } = {};

        if (supportsTemperature) {
          payload.temperature = currentSettings.temperature;
        }

        if (
          hasParameterSupport("top_p", currentModel) &&
          currentSettings.enableTopP
        ) {
          payload.topP = currentSettings.topP;
        }

        if (
          hasParameterSupport("top_k", currentModel) &&
          currentSettings.enableTopK
        ) {
          payload.topK = currentSettings.topK;
        }

        if (hasParameterSupport("streaming", currentModel)) {
          payload.stream = currentSettings.streamResponse;
        }

        if (hasParameterSupport("output_max_tokens", currentModel)) {
          payload.maxTokens = currentSettings.maxTokens;
        }

        if (Object.keys(payload).length > 0) {
          await chatActions.updateModelSettings(payload);
        }

        originalSettings = { ...currentSettings };
        saveStatus = "saved";
      } catch (error) {
        console.error("Failed to update model settings:", error);
        saveStatus = "error";
      }
    }, 500);
  });
</script>

<div class="space-y-0">
  {#if supportsTemperature}
    <TableGroup>
      <TableBaseRow label="Temperature" layout="vertical">
        <LabeledSlider
          bind:value={currentSettings.temperature}
          min={0}
          max={resolveTemperatureMax(currentSettings.temperature)}
          step={0.1}
          scaleMarks={getTemperatureScaleMarks()}
          showScaleMarks={false}
          showValue={true}
        />
      </TableBaseRow>
    </TableGroup>
  {/if}

  {#if hasAdvancedParameters}
    <TableGroup title="高级" collapsible defaultCollapsed={true}>
      {#if hasParameterSupport("top_p", currentModel)}
        <ModelSliderParameterRow
          label="Top-p"
          bind:value={currentSettings.topP}
          bind:enabled={currentSettings.enableTopP}
          min={0}
          max={resolveTopPMax(currentSettings.topP)}
          step={0.05}
          scaleMarks={getTopPScaleMarks()}
          showScaleMarks={false}
          showValue={true}
        />
      {/if}

      {#if hasParameterSupport("top_k", currentModel)}
        <ModelSliderParameterRow
          label="Top-k"
          bind:value={currentSettings.topK}
          bind:enabled={currentSettings.enableTopK}
          min={1}
          max={resolveTopKMax(currentSettings.topK)}
          step={1}
          scaleMarks={getTopKScaleMarks()}
          showScaleMarks={false}
          showValue={true}
        />
      {/if}

      {#if hasParameterSupport("output_max_tokens", currentModel)}
        <NumberStepperRow
          label="输出最大 Token"
          bind:value={currentSettings.maxTokens}
          min={1}
          max={resolveOutputTokensMax(currentSettings.maxTokens)}
          step={100}
        />
      {/if}

      {#if hasParameterSupport("streaming", currentModel)}
        <TableBaseRow>
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium text-base-content">流式输出</p>
              <p class="text-xs text-base-content/60">
                实时获取模型响应，适合长文本输出。
              </p>
            </div>
            <Toggle bind:checked={currentSettings.streamResponse} />
          </div>
        </TableBaseRow>
      {/if}
    </TableGroup>
  {/if}
</div>
