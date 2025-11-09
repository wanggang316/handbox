<script lang="ts">
  import { chatState, chatActions, getSupportedParameterSet } from '$lib/states/chat.svelte';
  import type {
    ChatReasoningConfig,
    ReasoningEffort,
    ReasoningSummary,
  } from '$lib/types/chat';
  import type { ModelWithProvider } from '$lib/types/provider';
  import SelectRow from '../../ui/table/SelectRow.svelte';
  import SwitchRow from '../../ui/table/SwitchRow.svelte';
  import TableBaseRow from '../../ui/table/TableBaseRow.svelte';

  let {
    paramName,
    label = undefined,
    model = null,
  }: {
    paramName: 'reasoning' | 'reasoning_effort' | 'thinking';
    label?: string;
    model?: ModelWithProvider | null;
  } = $props();

  const chat = $derived(chatState.currentChat);
  let variant = $state<'responses' | 'reasoning_effort' | 'thinking'>('responses');
  let currentReasoning = $state<ChatReasoningConfig | null>(null);

  $effect(() => {
    currentReasoning = chat?.reasoning ?? null;
    variant =
      paramName === 'thinking'
        ? 'thinking'
        : paramName === 'reasoning_effort'
          ? 'reasoning_effort'
          : 'responses';
  });

  function chatSupports(key: 'reasoning' | 'thinking'): boolean {
    const chatParams = chat?.supportedParameters;
    if (Array.isArray(chatParams) && chatParams.length > 0) {
      return chatParams.includes(key);
    }

    if (model) {
      const supported = getSupportedParameterSet(model);
      if (key === 'thinking') {
        return supported.has('thinking');
      }
      return supported.has('reasoning') || supported.has('reasoning_effort');
    }

    // 默认允许 reasoning，thinking 默认禁用
    return key === 'reasoning';
  }

  let enabled = $state(true);
  $effect(() => {
    enabled =
      variant === 'thinking' ? chatSupports('thinking') : chatSupports('reasoning');
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

  function cleanupConfig(config: ChatReasoningConfig): ChatReasoningConfig | null {
    if (config.responses && !config.responses.effort && !config.responses.summary) {
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

  const effortOptions = [
    { value: '', label: '跟随模型' },
    { value: 'minimal', label: 'Minimal' },
    { value: 'low', label: 'Low' },
    { value: 'medium', label: 'Medium' },
    { value: 'high', label: 'High' },
  ];

  const summaryOptions = [
    { value: '', label: '跟随模型' },
    { value: 'auto', label: 'Auto' },
    { value: 'concise', label: 'Concise' },
    { value: 'detailed', label: 'Detailed' },
  ];

  function normalizeEffort(value: string): ReasoningEffort | undefined {
    return value ? (value as ReasoningEffort) : undefined;
  }

  function normalizeSummary(value: string): ReasoningSummary | undefined {
    return value ? (value as ReasoningSummary) : undefined;
  }

  let thinkingBudgetInput = $state('');

  $effect(() => {
    if (!enabled || variant !== 'thinking') return;
    const budget = currentReasoning?.thinking?.thinkingBudget;
    thinkingBudgetInput =
      budget === null || budget === undefined ? '' : budget.toString();
  });

  function updateThinkingBudget(raw: string) {
    const text = raw.trim();
    thinkingBudgetInput = text;
    if (!text) {
      applyReasoning((draft) => {
        draft.thinking = draft.thinking ?? {};
        draft.thinking.thinkingBudget = null;
      });
      return;
    }

    const parsed = Number(text);
    if (!Number.isFinite(parsed) || parsed < 0) {
      return;
    }
    applyReasoning((draft) => {
      draft.thinking = draft.thinking ?? {};
      draft.thinking.thinkingBudget = Math.floor(parsed);
    });
  }
</script>

{#if enabled}
  {#if variant === 'responses'}
    <SelectRow
      label={`${label ?? 'Reasoning'} · 难度`}
      options={effortOptions}
      selectedValue={currentReasoning?.responses?.effort ?? ''}
      onSelect={(value) =>
        applyReasoning((draft) => {
          draft.responses = draft.responses ?? {};
          draft.responses.effort = normalizeEffort(value) ?? null;
        })
      }
    />
    <SelectRow
      label={`${label ?? 'Reasoning'} · 总结`}
      options={summaryOptions}
      selectedValue={currentReasoning?.responses?.summary ?? ''}
      onSelect={(value) =>
        applyReasoning((draft) => {
          draft.responses = draft.responses ?? {};
          draft.responses.summary = normalizeSummary(value) ?? null;
        })
      }
    />
  {:else if variant === 'reasoning_effort'}
    <SelectRow
      label={label ?? 'Reasoning'}
      options={effortOptions}
      selectedValue={currentReasoning?.reasoningEffort?.effort ?? ''}
      onSelect={(value) =>
        applyReasoning((draft) => {
          draft.reasoningEffort = draft.reasoningEffort ?? {};
          draft.reasoningEffort.effort = normalizeEffort(value) ?? null;
        })
      }
    />
  {:else}
    <SwitchRow
      label={`${label ?? 'Thinking'} · 包含过程`}
      checked={currentReasoning?.thinking?.includeThoughts ?? false}
      onChange={(value) =>
        applyReasoning((draft) => {
          draft.thinking = draft.thinking ?? {};
          draft.thinking.includeThoughts = value;
        })
      }
    />
    <TableBaseRow label={`${label ?? 'Thinking'} · 预算`} layout="vertical">
      <div class="flex items-center gap-2 pt-2">
        <input
          class="input input-sm w-24 text-right"
          type="number"
          min="0"
          step="1"
          value={thinkingBudgetInput}
          onchange={(event) =>
            updateThinkingBudget((event.currentTarget as HTMLInputElement).value)
          }
          placeholder="默认"
        />
        <button
          type="button"
          class="btn btn-ghost btn-xs"
          onclick={() => updateThinkingBudget('')}
        >
          清除
        </button>
      </div>
    </TableBaseRow>
  {/if}
{/if}
