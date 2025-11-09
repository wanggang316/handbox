<script lang="ts">
  import {
    chatState,
    chatActions,
    getSupportedParameterSet,
  } from "$lib/states/chat.svelte";
  import type { ChatReasoningConfig } from "$lib/types/chat";
  import type { ModelWithProvider } from "$lib/types/provider";
  import SwitchRow from "../../ui/table/SwitchRow.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";

  let {
    label = undefined,
    model = null,
  }: {
    label?: string;
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

  let thinkingBudgetInput = $state("");

  $effect(() => {
    if (!enabled) return;
    const budget = currentReasoning?.thinking?.thinkingBudget;
    thinkingBudgetInput =
      budget === null || budget === undefined ? "" : budget.toString();
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
  <SwitchRow
    label={`${label ?? "Thinking"} · 包含过程`}
    checked={currentReasoning?.thinking?.includeThoughts ?? false}
    onChange={(value) =>
      applyReasoning((draft) => {
        draft.thinking = draft.thinking ?? {};
        draft.thinking.includeThoughts = value;
      })}
  />
  <TableBaseRow label={`${label ?? "Thinking"} · 预算`} layout="vertical">
    <div class="flex items-center gap-2 pt-2">
      <input
        class="input input-sm w-24 text-right"
        type="number"
        min="0"
        step="1"
        value={thinkingBudgetInput}
        onchange={(event) =>
          updateThinkingBudget((event.currentTarget as HTMLInputElement).value)}
        placeholder="默认"
      />
      <button
        type="button"
        class="btn btn-ghost btn-xs"
        onclick={() => updateThinkingBudget("")}
      >
        清除
      </button>
    </div>
  </TableBaseRow>
{/if}
