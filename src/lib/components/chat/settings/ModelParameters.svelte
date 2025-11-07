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
      typeof modelDefaults.topK === "number" &&
      Number.isFinite(modelDefaults.topK)
        ? Math.max(modelDefaults.topK, 1)
        : 1;
    const baseline = Math.max(
      getMaxNumber("top_k", defaultTopK, currentModel),
      defaultTopK,
      1
    );
    const currentValue =
      typeof current === "number" && Number.isFinite(current)
        ? current
        : defaultTopK;
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
      typeof modelDefaults.maxTokens === "number" &&
      Number.isFinite(modelDefaults.maxTokens)
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
      typeof current === "number" && Number.isFinite(current)
        ? current
        : baseline;
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
      ? Math.max(
          getMaxNumber("top_k", defaultTopK, currentModel),
          defaultTopK,
          1
        )
      : undefined;
    const maxTokensLimit = Math.max(
      getMaxNumber(
        "output_max_tokens",
        getMaxNumber(
          "max_tokens",
          typeof defaults.maxTokens === "number" &&
            Number.isFinite(defaults.maxTokens)
            ? Math.max(defaults.maxTokens, 1)
            : 1,
          currentModel
        ),
        currentModel
      ),
      typeof defaults.maxTokens === "number" &&
        Number.isFinite(defaults.maxTokens)
        ? Math.max(defaults.maxTokens, 1)
        : 1
    );

    // 使用模型默认值作为 fallback，而不是 0
    const temperatureFallback =
      typeof defaults.temperature === "number" &&
      Number.isFinite(defaults.temperature)
        ? defaults.temperature
        : 1.0; // 改为 1.0（OpenAI 默认值）

    const topPFallback =
      typeof defaults.topP === "number" && Number.isFinite(defaults.topP)
        ? defaults.topP
        : 1.0; // 改为 1.0（OpenAI 默认值）

    const maxTokensFallback =
      typeof defaults.maxTokens === "number" &&
      Number.isFinite(defaults.maxTokens)
        ? Math.max(defaults.maxTokens, 1)
        : 2048; // 改为合理的默认值

    // 判断参数是否被用户设置过（值 > 0 表示已设置）
    const hasTemperature =
      typeof chat?.temperature === "number" && chat.temperature > 0;
    const hasTopP = typeof chat?.topP === "number" && chat.topP > 0;
    const hasTopK = typeof chat?.topK === "number" && chat.topK > 0;
    const hasMaxTokens =
      typeof chat?.maxTokens === "number" && chat.maxTokens > 0;

    const temperature = hasTemperature
      ? clamp(chat!.temperature!, 0, temperatureMax)
      : clamp(temperatureFallback, 0, temperatureMax);

    const topP = hasTopP
      ? clamp(chat!.topP!, 0, topPMax)
      : clamp(topPFallback, 0, topPMax);

    const topK = hasParameterSupport("top_k", currentModel)
      ? hasTopK
        ? clamp(chat!.topK!, 1, topKMax ?? defaultTopK)
        : clamp(defaultTopK, 1, topKMax ?? defaultTopK)
      : undefined;

    const maxTokens = hasMaxTokens
      ? clamp(chat!.maxTokens!, 1, maxTokensLimit)
      : clamp(maxTokensFallback, 1, maxTokensLimit);

    const streamResponse = hasParameterSupport("streaming", currentModel)
      ? typeof chat?.stream === "boolean"
        ? chat?.stream
        : typeof defaults.streamResponse === "boolean"
          ? defaults.streamResponse
          : true
      : typeof defaults.streamResponse === "boolean"
        ? defaults.streamResponse
        : true;

    return {
      temperature,
      topP,
      topK,
      streamResponse,
      maxTokens,
      // 开关状态：只有当值有效设置时才启用（值 > 0）
      enableTemperature: hasTemperature,
      enableTopP: hasTopP,
      enableTopK: hasTopK,
      enableMaxTokens: hasMaxTokens,
    };
  }

  let currentSettings = $state(buildInitialSettings());
  let originalSettings = $state(buildInitialSettings());
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");

  async function saveSettings() {
    try {
      saveStatus = "saving";
      const payload: {
        temperature?: number | null;
        topP?: number | null;
        topK?: number | null;
        stream?: boolean;
        maxTokens?: number | null;
      } = {};

      // 处理 temperature：开关启用时设置值，关闭时设置为 null
      if (supportsTemperature) {
        payload.temperature = currentSettings.enableTemperature
          ? currentSettings.temperature
          : null;
      }

      // 处理 top_p：开关启用时设置值，关闭时设置为 null
      if (hasParameterSupport("top_p", currentModel)) {
        payload.topP = currentSettings.enableTopP ? currentSettings.topP : null;
      }

      // 处理 top_k：开关启用时设置值，关闭时设置为 null
      if (hasParameterSupport("top_k", currentModel)) {
        payload.topK = currentSettings.enableTopK ? currentSettings.topK : null;
      }

      // 处理 streaming：始终保存
      if (hasParameterSupport("streaming", currentModel)) {
        payload.stream = currentSettings.streamResponse;
      }

      // 处理 max_tokens：开关启用时设置值，关闭时设置为 null
      if (hasParameterSupport("output_max_tokens", currentModel)) {
        payload.maxTokens = currentSettings.enableMaxTokens
          ? currentSettings.maxTokens
          : null;
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
  }

  // 当 Toggle 状态改变时立即保存
  function handleToggleChange() {
    console.log("[handleToggleChange] Current settings:", currentSettings);
    if (saveTimer) {
      clearTimeout(saveTimer);
    }
    saveSettings();
  }

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

    saveTimer = setTimeout(() => {
      saveSettings();
    }, 500);
  });
</script>

<div class="space-y-0">
  {#if supportsTemperature}
    <TableGroup>
      <ModelSliderParameterRow
        label="Temperature"
        bind:value={currentSettings.temperature}
        bind:enabled={currentSettings.enableTemperature}
        min={0}
        max={resolveTemperatureMax(currentSettings.temperature)}
        step={0.1}
        scaleMarks={getTemperatureScaleMarks()}
        showScaleMarks={false}
        showValue={true}
        onToggleChange={handleToggleChange}
      />
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
          onToggleChange={handleToggleChange}
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
          onToggleChange={handleToggleChange}
        />
      {/if}

      {#if hasParameterSupport("output_max_tokens", currentModel)}
        <ModelSliderParameterRow
          label="输出最大 Token"
          bind:value={currentSettings.maxTokens}
          bind:enabled={currentSettings.enableMaxTokens}
          min={1}
          max={resolveOutputTokensMax(currentSettings.maxTokens)}
          step={100}
          scaleMarks={[
            { value: 1, position: 0 },
            {
              value: Math.floor(
                resolveOutputTokensMax(currentSettings.maxTokens) / 2
              ),
              position: 50,
            },
            {
              value: resolveOutputTokensMax(currentSettings.maxTokens),
              position: 100,
            },
          ]}
          showScaleMarks={false}
          showValue={true}
          onToggleChange={handleToggleChange}
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
