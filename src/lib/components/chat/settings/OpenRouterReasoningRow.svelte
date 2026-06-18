<script lang="ts">
  import {
    chatState,
    chatActions,
    getSupportedParameterSet,
    findMethodParameter,
  } from "$lib/states/chat.svelte";
  import type {
    ChatReasoningConfig,
    ReasoningEffort,
  } from "$lib/types/chat";
  import type {
    ModelWithProvider,
    OpenrouterReasoningProps,
  } from "$lib/types/provider";
  import Select from "../../ui/Select.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";
  import Toggle from "../../ui/Toggle.svelte";
  import LabeledSlider from "../../ui/LabeledSlider.svelte";
  import { t } from "$lib/i18n";

  let {
    paramName,
    label = undefined,
    helpText = undefined,
    model = null,
  }: {
    paramName: "reasoning";
    label?: string;
    helpText?: string;
    model?: ModelWithProvider | null;
  } = $props();

  const chat = $derived(chatState.currentChat);
  let currentReasoning = $state<ChatReasoningConfig | null>(null);

  $effect(() => {
    currentReasoning = chat?.reasoning ?? null;
  });

  function chatSupportsReasoning(): boolean {
    if (model) {
      const supported = getSupportedParameterSet(model);
      return supported.has("reasoning");
    }
    return true;
  }

  let enabled = $state(true);
  $effect(() => {
    enabled = chatSupportsReasoning();
  });

  function cloneReasoning(): ChatReasoningConfig {
    return {
      responses: currentReasoning?.responses
        ? { ...currentReasoning.responses }
        : undefined,
      reasoningEffort: currentReasoning?.reasoningEffort
        ? { ...currentReasoning.reasoningEffort }
        : undefined,
      thinking: currentReasoning?.thinking
        ? { ...currentReasoning.thinking }
        : undefined,
      openrouter: currentReasoning?.openrouter
        ? { ...currentReasoning.openrouter }
        : undefined,
    };
  }

  function cleanupConfig(
    config: ChatReasoningConfig
  ): ChatReasoningConfig | null {
    if (
      config.responses &&
      !config.responses.effort &&
      !config.responses.summary
    ) {
      delete config.responses;
    }
    if (
      config.reasoningEffort &&
      !config.reasoningEffort.effort &&
      config.reasoningEffort.includeReasoning == null
    ) {
      delete config.reasoningEffort;
    }
    if (
      config.thinking &&
      config.thinking.includeThoughts == null &&
      config.thinking.thinkingBudget == null
    ) {
      delete config.thinking;
    }
    if (
      config.openrouter &&
      !config.openrouter.effort &&
      config.openrouter.maxTokens == null &&
      config.openrouter.exclude == null
    ) {
      delete config.openrouter;
    }

    if (!config.responses && !config.reasoningEffort && !config.thinking && !config.openrouter) {
      return null;
    }

    return config;
  }

  async function applyReasoning(mutator: (draft: ChatReasoningConfig) => void) {
    if (!chat?.id) return;
    const draft = cloneReasoning();
    mutator(draft);
    const next = cleanupConfig(draft);
    await chatActions.updateReasoning(next);
  }

  // 从模型配置中获取 reasoning 参数的 props
  function getReasoningProps(): OpenrouterReasoningProps | null {
    if (!model) return null;
    const param = findMethodParameter(paramName, model);
    if (!param || !param.props) return null;
    return param.props as OpenrouterReasoningProps;
  }

  // 获取已解析的 props（backend 已经根据模型匹配好了）
  const resolvedProps = $derived(() => {
    const props = getReasoningProps();
    return props?.props || ["effect", "exclude"];
  });

  // 是否显示 effect 参数
  const showEffect = $derived(() => {
    return resolvedProps().includes("effect");
  });

  // 是否显示 max_tokens 参数
  const showMaxTokens = $derived(() => {
    return resolvedProps().includes("max_tokens");
  });

  // 是否显示 exclude 参数
  const showExclude = $derived(() => {
    return resolvedProps().includes("exclude");
  });

  // 构建 effort 选项
  const effortOptions = $derived(() => {
    const reasoningProps = getReasoningProps();
    const options = reasoningProps?.effort_options || ["low", "medium", "high"];

    return [
      { value: "", label: t("chat.followModel") },
      ...options.map((opt) => ({
        value: opt,
        label: opt.charAt(0).toUpperCase() + opt.slice(1),
      })),
    ];
  });

  // max_tokens 范围
  const maxTokensRange = $derived(() => {
    const props = getReasoningProps();
    return props?.max_tokens || [1, 99999999];
  });

  function normalizeEffort(value: string): ReasoningEffort | undefined {
    return value ? (value as ReasoningEffort) : undefined;
  }

  // 获取各个参数的提示文本
  const effectTips = $derived(() => {
    const props = getReasoningProps();
    return props?.effect_tips;
  });

  const maxTokensTips = $derived(() => {
    const props = getReasoningProps();
    return props?.max_tokens_tips;
  });

  // max_tokens 值的响应式状态
  let maxTokensValue = $state(1);

  // 同步 maxTokensValue 与 currentReasoning
  $effect(() => {
    maxTokensValue = currentReasoning?.openrouter?.maxTokens ?? maxTokensRange()[0];
  });

  // 监听 maxTokensValue 变化并应用到数据库
  let maxTokensUpdateTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    if (!showMaxTokens()) return;

    const currentMaxTokens = currentReasoning?.openrouter?.maxTokens;
    // 只有当值真正变化时才保存
    if (currentMaxTokens === maxTokensValue) return;

    if (maxTokensUpdateTimer) {
      clearTimeout(maxTokensUpdateTimer);
    }

    maxTokensUpdateTimer = setTimeout(() => {
      applyReasoning((draft) => {
        draft.openrouter = draft.openrouter ?? {};
        draft.openrouter.maxTokens = maxTokensValue;
      });
    }, 300);
  });
</script>

{#if enabled}
  <TableBaseRow label={label ?? "Reasoning"} {helpText} layout="vertical">
    <div class="flex flex-col gap-3 pt-2 pl-2">
      <!-- Effect Selection -->
      {#if showEffect()}
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/60" title={effectTips()}>
            {t("chat.effort")}
          </span>
          <Select
            value={currentReasoning?.openrouter?.effort ?? ""}
            options={effortOptions()}
            autoWidth={true}
            size="sm"
            onChange={(value) => {
              applyReasoning((draft) => {
                draft.openrouter = draft.openrouter ?? {};
                draft.openrouter.effort = normalizeEffort(value) ?? null;
              });
            }}
          />
        </div>
      {/if}

      <!-- Max Tokens Input -->
      {#if showMaxTokens()}
        {@const range = maxTokensRange()}
        <div class="flex flex-col gap-1">
          <div class="flex items-center justify-between">
            <span class="text-xs text-base-content/60" title={maxTokensTips()}>
              Max Tokens
            </span>
          </div>
          <LabeledSlider
            bind:value={maxTokensValue}
            min={range[0]}
            max={range[1]}
            step={1}
            showValue={true}
            showScaleMarks={false}
          />
        </div>
      {/if}

      <!-- Exclude Toggle (default on, inverted logic) -->
      {#if showExclude()}
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/60">{t("chat.includeReasoning")}</span>
          <Toggle
            checked={!(currentReasoning?.openrouter?.exclude ?? false)}
            onChange={(value) => {
              applyReasoning((draft) => {
                draft.openrouter = draft.openrouter ?? {};
                // Toggle 开启时，exclude 为 false（包含推理）
                draft.openrouter.exclude = !value;
              });
            }}
          />
        </div>
      {/if}
    </div>
  </TableBaseRow>
{/if}
