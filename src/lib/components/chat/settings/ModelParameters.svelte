<script lang="ts">
  import {
    chatState,
    chatActions,
    currentChatModel,
    toNumber,
  } from "$lib/states/chat.svelte";
  import ModelSliderParameterRow from "./ModelSliderParameterRow.svelte";
  import ResponsesReasoningRow from "./ResponsesReasoningRow.svelte";
  import CompletionsReasoningRow from "./CompletionsReasoningRow.svelte";
  import GoogleGenaiThinkingRow from "./GoogleGenaiThinkingRow.svelte";
  import OpenRouterReasoningRow from "./OpenRouterReasoningRow.svelte";
  import SwitchRow from "../../ui/table/SwitchRow.svelte";
  import TableGroup from "../../ui/table/TableGroup.svelte";
  import type {
    ModelParameterResponse,
    SliderProps,
    SwitchProps,
    ResponsesReasoningProps,
    CompletionsReasoningProps,
    OpenrouterReasoningProps,
  } from "$lib/types/provider";
  import { t } from "$lib/i18n";

  type SaveStatus = "saved" | "saving" | "error";

  const currentModel = $derived(currentChatModel().model);
  const parameters = $derived(
    (currentModel?.chat_method?.parameters as ModelParameterResponse[]) || []
  );

  // 按 level 分组参数
  const baseParameters = $derived(parameters.filter((p) => p.level === "base"));
  const advanceParameters = $derived(
    parameters.filter((p) => p.level === "advance")
  );

  // 辅助函数：限制值在 min-max 范围内
  function clamp(value: number, min: number, max: number): number {
    return Math.max(min, Math.min(max, value));
  }

  // 辅助函数：类型守卫
  function isSliderProps(
    props:
      | SliderProps
      | SwitchProps
      | ResponsesReasoningProps
      | CompletionsReasoningProps
  ): props is SliderProps {
    return props != null && typeof props === "object" && "min" in props;
  }

  function isReasoningParamName(name: string): name is "reasoning" {
    return name === "reasoning";
  }

  // 辅助函数：将 snake_case 转换为 camelCase (用于数据库字段映射)
  function snakeToCamel(str: string): string {
    return str.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
  }

  // 构建初始设置
  function buildInitialSettings() {
    const chat = chatState.currentChat;
    const settings: Record<string, any> = {};

    console.log("parameters", parameters);
    parameters.forEach((param) => {
      if (
        param.component === "responses_reasoning" ||
        param.component === "completions_reasoning" ||
        param.component === "thinking"
      ) {
        return;
      }
      const paramName = param.name; // snake_case from backend
      const dbFieldName = snakeToCamel(paramName); // camelCase for database

      if (param.component === "slider" && isSliderProps(param.props)) {
        const props = param.props;
        const chatValue = (chat as any)?.[dbFieldName];
        const hasValue = chatValue !== null && chatValue !== undefined;

        const min = props.min ?? 0;
        const max = props.max ?? 100;
        const defaultValue = props.default ?? min;

        const value = hasValue
          ? clamp(toNumber(chatValue) ?? defaultValue, min, max)
          : clamp(defaultValue, min, max);

        settings[paramName] = value;
        settings[`enable${capitalize(paramName)}`] = hasValue;
      } else if (param.component === "switch") {
        const props = param.props as SwitchProps;
        const chatValue = (chat as any)?.[dbFieldName];
        const value =
          typeof chatValue === "boolean" ? chatValue : (props.default ?? true);

        settings[paramName] = value;
      }
    });

    return settings;
  }

  // 辅助函数：首字母大写
  function capitalize(str: string): string {
    return str.charAt(0).toUpperCase() + str.slice(1);
  }

  let currentSettings = $state(buildInitialSettings());
  let originalSettings = $state(buildInitialSettings());
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let saveStatus = $state<SaveStatus>("saved");

  /**
   * 保存单个字段
   */
  async function saveField(fieldName: string, value: number | boolean | null) {
    try {
      saveStatus = "saving";
      await chatActions.updateModelField(fieldName as any, value);
      saveStatus = "saved";
    } catch (error) {
      console.error(`Failed to update ${fieldName}:`, error);
      saveStatus = "error";
    }
  }

  // 重置状态到初始值
  $effect(() => {
    // 监听模型或聊天配置变化，刷新本地缓存
    currentModel;
    chatState.currentChat;
    const next = buildInitialSettings();
    currentSettings = { ...next };
    originalSettings = { ...next };
    saveStatus = "saved";

    // 取消可能存在的定时器
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
  });

  // 为每个参数创建自动保存 effect
  $effect(() => {
    parameters.forEach((param) => {
      if (
        param.component === "responses_reasoning" ||
        param.component === "completions_reasoning" ||
        param.component === "thinking"
      ) {
        return;
      }
      const paramName = param.name; // snake_case from backend
      const dbFieldName = snakeToCamel(paramName); // camelCase for database

      if (param.component === "slider") {
        const enableKey = `enable${capitalize(paramName)}`;

        // 跳过初始化
        if (!originalSettings[enableKey] && !currentSettings[enableKey]) {
          return;
        }

        const newValue = currentSettings[enableKey]
          ? currentSettings[paramName]
          : null;
        const oldValue = originalSettings[enableKey]
          ? originalSettings[paramName]
          : null;

        if (newValue === oldValue) {
          return;
        }

        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(async () => {
          await saveField(dbFieldName, newValue);
          originalSettings[paramName] = currentSettings[paramName];
          originalSettings[enableKey] = currentSettings[enableKey];
        }, 500);
      } else if (param.component === "switch") {
        if (currentSettings[paramName] === originalSettings[paramName]) {
          return;
        }

        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(async () => {
          await saveField(dbFieldName, currentSettings[paramName]);
          originalSettings[paramName] = currentSettings[paramName];
        }, 500);
      }
    });
  });

  // 渲染滑块参数
  function renderSliderParameter(param: ModelParameterResponse) {
    if (!isSliderProps(param.props)) return null;

    const props = param.props;
    const paramName = param.name;
    const enableKey = `enable${capitalize(paramName)}`;

    const min = props.min ?? 0;
    const max = props.max ?? 100;
    const step = props.step ?? 1;

    // 计算刻度标记
    const scaleMarks = [
      { value: min, position: 0 },
      { value: Number(((min + max) / 2).toFixed(2)), position: 50 },
      { value: max, position: 100 },
    ];

    return {
      label: props.name,
      min,
      max,
      step,
      scaleMarks,
      showToggle: props.show_toggle ?? false,
      paramName,
      enableKey,
      tips: props.tips,
    };
  }
</script>

<div class="space-y-0">
  {#if baseParameters.length > 0}
    <TableGroup>
      {#each baseParameters as param}
        {#if param.component === "slider"}
          {@const config = renderSliderParameter(param)}
          {#if config}
            <ModelSliderParameterRow
              label={config.label}
              bind:value={currentSettings[config.paramName]}
              bind:enabled={currentSettings[config.enableKey]}
              min={config.min}
              max={config.max}
              step={config.step}
              scaleMarks={config.scaleMarks}
              showScaleMarks={false}
              showValue={true}
              showToggle={config.showToggle}
              helpText={config.tips ?? undefined}
            />
          {/if}
        {:else if param.component === "switch"}
          {@const props = param.props as SwitchProps}
          <SwitchRow
            label={props.name}
            bind:checked={currentSettings[param.name]}
            helpText={props.tips ?? undefined}
          />
        {:else if param.component === "responses_reasoning" && isReasoningParamName(param.name)}
          <ResponsesReasoningRow
            paramName={param.name}
            label={(param.props as ResponsesReasoningProps)?.name ?? param.name}
            helpText={(param.props as ResponsesReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {:else if param.component === "completions_reasoning" && isReasoningParamName(param.name)}
          <CompletionsReasoningRow
            paramName={param.name}
            label={(param.props as CompletionsReasoningProps)?.name ??
              param.name}
            helpText={(param.props as CompletionsReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {:else if param.component === "thinking"}
          <GoogleGenaiThinkingRow
            label={(param.props as ResponsesReasoningProps)?.name ?? param.name}
            helpText={(param.props as ResponsesReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {:else if param.component === "openrouter_reasoning" && isReasoningParamName(param.name)}
          <OpenRouterReasoningRow
            paramName={param.name}
            label={(param.props as OpenrouterReasoningProps)?.name ?? param.name}
            helpText={(param.props as OpenrouterReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {/if}
      {/each}
    </TableGroup>
  {/if}

  {#if advanceParameters.length > 0}
    <TableGroup title={t("chat.advanced")} collapsible defaultCollapsed={true}>
      {#each advanceParameters as param}
        {#if param.component === "slider"}
          {@const config = renderSliderParameter(param)}
          {#if config}
            <ModelSliderParameterRow
              label={config.label}
              bind:value={currentSettings[config.paramName]}
              bind:enabled={currentSettings[config.enableKey]}
              min={config.min}
              max={config.max}
              step={config.step}
              scaleMarks={config.scaleMarks}
              showScaleMarks={false}
              showValue={true}
              showToggle={config.showToggle}
              helpText={config.tips ?? undefined}
            />
          {/if}
        {:else if param.component === "switch"}
          {@const props = param.props as SwitchProps}
          <SwitchRow
            label={props.name}
            bind:checked={currentSettings[param.name]}
            helpText={props.tips ?? undefined}
          />
        {:else if param.component === "responses_reasoning" && isReasoningParamName(param.name)}
          <ResponsesReasoningRow
            paramName={param.name}
            label={(param.props as ResponsesReasoningProps)?.name ?? param.name}
            helpText={(param.props as ResponsesReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {:else if param.component === "completions_reasoning" && isReasoningParamName(param.name)}
          <CompletionsReasoningRow
            paramName={param.name}
            label={(param.props as CompletionsReasoningProps)?.name ??
              param.name}
            helpText={(param.props as CompletionsReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {:else if param.component === "thinking"}
          <GoogleGenaiThinkingRow
            label={(param.props as ResponsesReasoningProps)?.name ?? param.name}
            helpText={(param.props as ResponsesReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {:else if param.component === "openrouter_reasoning" && isReasoningParamName(param.name)}
          <OpenRouterReasoningRow
            paramName={param.name}
            label={(param.props as OpenrouterReasoningProps)?.name ?? param.name}
            helpText={(param.props as OpenrouterReasoningProps)?.tips ??
              undefined}
            model={currentModel ?? null}
          />
        {/if}
      {/each}
    </TableGroup>
  {/if}
</div>
