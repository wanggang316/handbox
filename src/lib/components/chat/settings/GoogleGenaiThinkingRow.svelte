<script lang="ts">
  import {
    chatState,
    chatActions,
    getSupportedParameterSet,
    findMethodParameter,
  } from "$lib/states/chat.svelte";
  import type { ChatReasoningConfig } from "$lib/types/chat";
  import type {
    ModelWithProvider,
    ThinkingProps,
    BudgetConfig,
    BudgetOptions,
  } from "$lib/types/provider";
  import Toggle from "../../ui/Toggle.svelte";
  import Select from "../../ui/Select.svelte";
  import LabeledSlider from "../../ui/LabeledSlider.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";
  import InfoTooltip from "../../ui/InfoTooltip.svelte";
  import { t } from "$lib/i18n";

  let {
    label = undefined,
    helpText = undefined,
    model = null,
  }: {
    label?: string;
    helpText?: string;
    model?: ModelWithProvider | null;
  } = $props();

  const chat = $derived(chatState.currentChat);
  let currentReasoning = $state<ChatReasoningConfig | null>(null);

  $effect(() => {
    currentReasoning = chat?.reasoning ?? null;
  });

  function chatSupportsThinking(): boolean {
    if (model) {
      const supported = getSupportedParameterSet(model);
      return supported.has("thinking") || supported.has("reasoning");
    }
    return false;
  }

  let enabled = $state(false);
  $effect(() => {
    enabled = chatSupportsThinking();
  });

  // 获取 thinking 参数配置
  function getThinkingProps(): ThinkingProps | null {
    if (!model) return null;
    const param = findMethodParameter("reasoning", model);
    if (!param || !param.props) return null;
    return param.props as ThinkingProps;
  }

  // 根据当前模型获取对应的 budget 配置
  function getCurrentBudgetConfig(): BudgetConfig | null {
    const thinkingProps = getThinkingProps();
    if (!thinkingProps?.budget_configs) return null;

    const key = `${model?.providerType}/${model?.id}`;

    // 查找匹配当前模型的配置
    for (const config of thinkingProps.budget_configs) {
      if (config.models.includes(key)) {
        return config;
      }
    }

    return null;
  }

  // 获取当前配置的选项列表
  const budgetModeOptions = $derived(() => {
    const config = getCurrentBudgetConfig();
    if (!config) return [];

    const options: Array<{ value: string; label: string }> = [];

    if (
      config.options.dynamic !== undefined &&
      config.options.dynamic !== null
    ) {
      options.push({ value: "dynamic", label: "Dynamic" });
    }
    if (
      config.options.disable !== undefined &&
      config.options.disable !== null
    ) {
      options.push({ value: "disable", label: "Disable" });
    }
    if (config.options.range) {
      options.push({ value: "range", label: "Custom" });
    }

    return options;
  });

  // 当前选中的模式和 range 值
  let budgetMode = $state<string>("dynamic");
  let rangeValue = $state(0);

  // 将 DB 值转换为 UI 状态
  function budgetToMode(budget: number | null | undefined): string {
    const config = getCurrentBudgetConfig();
    if (!config) return "dynamic";

    if (budget === undefined || budget === null) {
      return config.default;
    } else if (budget === -1) {
      return "dynamic";
    } else if (budget === 0) {
      return "disable";
    } else {
      return "range";
    }
  }

  // 将 UI 状态转换为 DB 值
  function modeToBudget(mode: string, currentRange: number): number | null {
    const config = getCurrentBudgetConfig();
    if (!config) return null;

    if (mode === "dynamic" && config.options.dynamic !== undefined) {
      return config.options.dynamic;
    } else if (mode === "disable" && config.options.disable !== undefined) {
      return config.options.disable;
    } else if (mode === "range") {
      return currentRange;
    }
    return null;
  }

  // 从数据库同步状态到 UI（只在 DB 值真正变化时更新）
  $effect(() => {
    const config = getCurrentBudgetConfig();
    if (!config) return;

    const currentBudget = currentReasoning?.thinking?.thinkingBudget;
    const expectedMode = budgetToMode(currentBudget);

    // 只有当 mode 变化时才更新（避免干扰用户拖动滑块）
    if (expectedMode !== budgetMode) {
      budgetMode = expectedMode;

      // mode 变化时，同步更新 rangeValue
      if (expectedMode === "range") {
        if (typeof currentBudget === "number" && currentBudget > 0) {
          rangeValue = currentBudget;
        } else if (config.options.range) {
          rangeValue = config.options.range[0];
        }
      }
    }
  });

  // 监听 rangeValue 变化并应用到数据库（仅在 range 模式下）
  let rangeUpdateTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    if (budgetMode !== "range") return;

    const currentBudget = currentReasoning?.thinking?.thinkingBudget;
    // 只有当值真正变化时才保存
    if (currentBudget === rangeValue) return;

    if (rangeUpdateTimer) {
      clearTimeout(rangeUpdateTimer);
    }

    rangeUpdateTimer = setTimeout(() => {
      applyReasoning((draft) => {
        draft.thinking = draft.thinking ?? {};
        draft.thinking.thinkingBudget = rangeValue;
      });
      rangeUpdateTimer = null;
    }, 300);
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
    if (config.reasoningEffort && !config.reasoningEffort.effort) {
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
    mutator(draft);
    const next = cleanupConfig(draft);
    await chatActions.updateReasoning(next);
  }

  // 处理 budget mode 变化
  function handleBudgetModeChange(mode: string) {
    const config = getCurrentBudgetConfig();
    if (!config) return;

    // 立即更新 UI
    budgetMode = mode;

    let newBudget: number | null = null;

    if (mode === "dynamic" && config.options.dynamic !== undefined) {
      newBudget = config.options.dynamic;
    } else if (mode === "disable" && config.options.disable !== undefined) {
      newBudget = config.options.disable;
    } else if (mode === "range" && config.options.range) {
      // 初始化 rangeValue 为当前值或最小值
      const currentBudget = currentReasoning?.thinking?.thinkingBudget;
      if (!currentBudget || currentBudget <= 0) {
        rangeValue = config.options.range[0];
      }
      newBudget = rangeValue;
    }

    // 保存到数据库（effect 会通过值比较避免循环）
    applyReasoning((draft) => {
      draft.thinking = draft.thinking ?? {};
      draft.thinking.thinkingBudget = newBudget;
    });
  }
</script>

{#if enabled}
  {@const thinkingProps = getThinkingProps()}
  <TableBaseRow label={label ?? "Thinking"} {helpText} layout="vertical">
    <div class="flex flex-col gap-3 pt-2 pl-2">
      <!-- Include Thoughts Toggle -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1">
          <span class="text-xs text-base-content/60">{t("chat.includeThoughts")}</span>
          {#if thinkingProps?.include_thoughts_tip}
            <InfoTooltip content={thinkingProps.include_thoughts_tip} />
          {/if}
        </div>
        <Toggle
          checked={currentReasoning?.thinking?.includeThoughts ?? false}
          onChange={(value) => {
            applyReasoning((draft) => {
              draft.thinking = draft.thinking ?? {};
              draft.thinking.includeThoughts = value;
            });
          }}
        />
      </div>

      <!-- Budget Configuration -->
      {#if budgetModeOptions().length > 0}
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-1">
              <span class="text-xs text-base-content/60">{t("chat.budgetMode")}</span>
              {#if thinkingProps?.budget_tip}
                <InfoTooltip content={thinkingProps.budget_tip} />
              {/if}
            </div>
            <Select
              value={budgetMode}
              options={budgetModeOptions()}
              autoWidth={true}
              size="sm"
              onChange={handleBudgetModeChange}
            />
          </div>

          <!-- Range Slider (only show when range mode is selected) -->
          {#if budgetMode === "range"}
            {@const config = getCurrentBudgetConfig()}
            {#if config?.options.range}
              <div class="mt-1">
                <LabeledSlider
                  bind:value={rangeValue}
                  min={config.options.range[0]}
                  max={config.options.range[1]}
                  step={1}
                  showValue={true}
                  showScaleMarks={false}
                />
              </div>
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  </TableBaseRow>
{/if}
