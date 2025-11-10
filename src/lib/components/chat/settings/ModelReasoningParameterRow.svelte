<script lang="ts">
  import {
    chatState,
    chatActions,
    getSupportedParameterSet,
  } from "$lib/states/chat.svelte";
  import type {
    ChatReasoningConfig,
    ReasoningEffort,
    ReasoningSummary,
  } from "$lib/types/chat";
  import type { ModelWithProvider } from "$lib/types/provider";
  import SelectRow from "../../ui/table/SelectRow.svelte";
  import TableBaseRow from "../../ui/table/TableBaseRow.svelte";

  let {
    paramName,
    label = undefined,
    model = null,
  }: {
    paramName: "reasoning" | "reasoning_effort";
    label?: string;
    model?: ModelWithProvider | null;
  } = $props();

  const chat = $derived(chatState.currentChat);
  let variant = $state<"responses" | "reasoning_effort">("responses");
  let currentReasoning = $state<ChatReasoningConfig | null>(null);

  $effect(() => {
    currentReasoning = chat?.reasoning ?? null;
    variant =
      paramName === "reasoning_effort" ? "reasoning_effort" : "responses";
  });

  function chatSupportsReasoning(): boolean {
    if (model) {
      const supported = getSupportedParameterSet(model);
      return supported.has("reasoning") || supported.has("reasoning_effort");
    }

    // 默认允许 reasoning，thinking 默认禁用
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
    { value: "", label: "跟随模型" },
    { value: "minimal", label: "Minimal" },
    { value: "low", label: "Low" },
    { value: "medium", label: "Medium" },
    { value: "high", label: "High" },
  ];

  const summaryOptions = [
    { value: "", label: "跟随模型" },
    { value: "auto", label: "Auto" },
    { value: "concise", label: "Concise" },
    { value: "detailed", label: "Detailed" },
  ];

  function normalizeEffort(value: string): ReasoningEffort | undefined {
    return value ? (value as ReasoningEffort) : undefined;
  }

  function normalizeSummary(value: string): ReasoningSummary | undefined {
    return value ? (value as ReasoningSummary) : undefined;
  }
</script>

{#if enabled}
  {#if variant === "responses"}
    <TableBaseRow label={label ?? "Reasoning"} layout="vertical">
      <div class="flex flex-col gap-2 pt-2 pl-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/60">难度</span>
          <select
            class="select select-xs w-28"
            value={currentReasoning?.responses?.effort ?? ""}
            onchange={(event) => {
              const value = (event.currentTarget as HTMLSelectElement).value;
              applyReasoning((draft) => {
                draft.responses = draft.responses ?? {};
                draft.responses.effort = normalizeEffort(value) ?? null;
              });
            }}
          >
            {#each effortOptions as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-xs text-base-content/60">总结</span>
          <select
            class="select select-xs w-28"
            value={currentReasoning?.responses?.summary ?? ""}
            onchange={(event) => {
              const value = (event.currentTarget as HTMLSelectElement).value;
              applyReasoning((draft) => {
                draft.responses = draft.responses ?? {};
                draft.responses.summary = normalizeSummary(value) ?? null;
              });
            }}
          >
            {#each summaryOptions as option}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
        </div>
      </div>
    </TableBaseRow>
  {:else}
    <SelectRow
      label={label ?? "Reasoning"}
      options={effortOptions}
      selectedValue={currentReasoning?.reasoningEffort?.effort ?? ""}
      onSelect={(value) =>
        applyReasoning((draft) => {
          draft.reasoningEffort = draft.reasoningEffort ?? {};
          draft.reasoningEffort.effort = normalizeEffort(value) ?? null;
        })}
    />
  {/if}
{/if}
