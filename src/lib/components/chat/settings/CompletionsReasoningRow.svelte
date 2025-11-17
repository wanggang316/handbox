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
    CompletionsReasoningProps,
  } from "$lib/types/provider";
  import Select from "../../ui/Select.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";
  import Toggle from "../../ui/Toggle.svelte";

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

    if (!config.responses && !config.reasoningEffort && !config.thinking) {
      return null;
    }

    return config;
  }

  async function applyReasoning(mutator: (draft: ChatReasoningConfig) => void) {
    if (!chat?.id) return;
    const draft = cloneReasoning();
    console.log("[CompletionsReasoning] Before mutator:", JSON.stringify(draft, null, 2));
    mutator(draft);
    console.log("[CompletionsReasoning] After mutator:", JSON.stringify(draft, null, 2));
    const next = cleanupConfig(draft);
    console.log("[CompletionsReasoning] After cleanup:", JSON.stringify(next, null, 2));
    await chatActions.updateReasoning(next);
    console.log("[CompletionsReasoning] Update sent to backend");
  }

  // 从模型配置中获取 reasoning 参数的 props
  function getReasoningProps(): CompletionsReasoningProps | null {
    if (!model) return null;
    const param = findMethodParameter(paramName, model);
    if (!param || !param.props) return null;
    return param.props as CompletionsReasoningProps;
  }

  // 根据 provider_type/model_id 获取对应的选项列表
  function getOptionsForModel(
    optionsConfig: Record<string, string[]> | null | undefined,
    defaultOptions: string[]
  ): string[] {
    if (!optionsConfig) return defaultOptions;

    // 尝试匹配 provider_type/model_id
    if (model) {
      const key = `${model.providerType}/${model.id}`;
      if (optionsConfig[key]) {
        return optionsConfig[key];
      }
    }

    // 回退到 common
    return optionsConfig.common || defaultOptions;
  }

  // 构建 effort 选项
  const effortOptions = $derived(() => {
    const reasoningProps = getReasoningProps();
    const configuredOptions = getOptionsForModel(
      reasoningProps?.effort_options,
      ["minimal", "low", "medium", "high"]
    );

    return [
      { value: "", label: "跟随模型" },
      ...configuredOptions.map((opt) => ({
        value: opt,
        label: opt.charAt(0).toUpperCase() + opt.slice(1),
      })),
    ];
  });

  function normalizeEffort(value: string): ReasoningEffort | undefined {
    return value ? (value as ReasoningEffort) : undefined;
  }

  // Get include_reasoning from props
  const includeReasoning = $derived(() => {
    const reasoningProps = getReasoningProps();
    return reasoningProps?.include_reasoning ?? false;
  });
</script>

{#if enabled}
  <TableBaseRow label={label ?? "Reasoning"} {helpText} layout="vertical">
    <div class="flex flex-col gap-3 pt-2 pl-2">
      <!-- Include Reasoning Toggle (if configured) -->
      {#if includeReasoning()}
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/60">包含推理</span>
          <Toggle
            checked={currentReasoning?.reasoningEffort?.includeReasoning ?? false}
            onChange={(value) => {
              applyReasoning((draft) => {
                draft.reasoningEffort = draft.reasoningEffort ?? {};
                draft.reasoningEffort.includeReasoning = value;
              });
            }}
          />
        </div>
      {/if}

      <!-- Effort Selection -->
      <div class="flex items-center justify-between">
        <span class="text-xs text-base-content/60">难度</span>
        <Select
          value={currentReasoning?.reasoningEffort?.effort ?? ""}
          options={effortOptions()}
          autoWidth={true}
          size="sm"
          onChange={(value) => {
            applyReasoning((draft) => {
              draft.reasoningEffort = draft.reasoningEffort ?? {};
              draft.reasoningEffort.effort = normalizeEffort(value) ?? null;
            });
          }}
        />
      </div>
    </div>
  </TableBaseRow>
{/if}
