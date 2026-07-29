<script lang="ts">
  import { Bot, ChevronsUpDown } from "@lucide/svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ModelSelectModal from "$lib/components/agentsession/ModelSelectModal.svelte";
  import { t } from "$lib/i18n";
  import type { Agent, AgentTarget, JobTarget, PromptTarget } from "$lib/types";
  import type {
    ModelWithProvider,
    ProviderWithModels,
  } from "$lib/types/provider";

  interface Props {
    /**
     * Controlled output bound by the parent via `bind:target`. Both kinds share
     * this outlet; switching kind replaces the whole object with a clean target
     * of that kind, so submissions never carry stale cross-kind fields.
     */
    target: JobTarget;
    /** Enabled providers (with their enabled models), used to resolve model display names. */
    providersWithModels?: ProviderWithModels[];
    /** Agent template candidates, loaded by the parent. */
    agents?: Agent[];
    agentsLoading?: boolean;
    /** Set by the parent on submit to highlight validation failures. */
    showError?: boolean;
  }

  let {
    target = $bindable(),
    providersWithModels = [],
    agents = [],
    agentsLoading = false,
    showError = false,
  }: Props = $props();

  type TargetKind = JobTarget["kind"];

  const KIND_ITEMS: { value: TargetKind; label: string }[] = [
    { value: "prompt", label: "Prompt" },
    { value: "agent", label: "Agent" },
  ];

  // Kind-narrowed views for the template.
  const promptTarget = $derived(target.kind === "prompt" ? target : null);
  const agentTarget = $derived(target.kind === "agent" ? target : null);

  function emptyTargetOf(kind: TargetKind): JobTarget {
    switch (kind) {
      case "prompt":
        return {
          kind: "prompt",
          providerId: "",
          modelId: "",
          prompt: "",
          sessionStrategy: "new_session",
        };
      case "agent":
        return { kind: "agent", agentId: "", modelId: "", initialMessage: "" };
    }
  }

  function handleKindChange(value: string): void {
    const kind = value as TargetKind;
    if (kind === target.kind) return;
    // Field isolation: replacing the whole object drops old-kind fields, so no
    // per-field cleanup is needed.
    target = emptyTargetOf(kind);
  }

  // Prompt targets store (providerId, modelId); display names resolve from
  // providersWithModels.
  function setPromptTarget(next: PromptTarget): void {
    target = next;
  }

  // Resolves to null when the model is deleted or not yet loaded, which shows
  // the "select model" placeholder.
  const selectedPromptModel = $derived.by((): ModelWithProvider | null => {
    const t = promptTarget;
    if (!t || !t.providerId || !t.modelId) return null;
    const provider = providersWithModels.find((p) => p.id === t.providerId);
    if (!provider) return null;
    const model = provider.models.find((m) => m.id === t.modelId);
    if (!model) return null;
    return {
      ...model,
      providerName: provider.name,
      providerType: provider.provider_type,
    };
  });

  function handleModelSelect(model: ModelWithProvider): void {
    if (!promptTarget) return;
    setPromptTarget({
      ...promptTarget,
      providerId: model.provider_id,
      modelId: model.id,
    });
  }

  function handlePromptTextChange(value: string): void {
    if (!promptTarget) return;
    setPromptTarget({ ...promptTarget, prompt: value });
  }

  function setAgentTarget(next: AgentTarget): void {
    target = next;
  }

  function handleAgentChange(value: string): void {
    if (!agentTarget) return;
    setAgentTarget({ ...agentTarget, agentId: value });
  }

  // Model used to run the agent — agent definitions carry no model, so each job
  // picks its own. Only modelId is stored; the backend resolves the provider
  // from the model catalog at execution time. Unresolvable → "select model".
  const selectedAgentModel = $derived.by((): ModelWithProvider | null => {
    const tgt = agentTarget;
    if (!tgt || !tgt.modelId) return null;
    for (const provider of providersWithModels) {
      const model = provider.models.find((m) => m.id === tgt.modelId);
      if (model) {
        return {
          ...model,
          providerName: provider.name,
          providerType: provider.provider_type,
        };
      }
    }
    return null;
  });

  function handleAgentModelSelect(model: ModelWithProvider): void {
    if (!agentTarget) return;
    setAgentTarget({ ...agentTarget, modelId: model.id });
  }

  // One model-picker modal per target kind.
  let promptModelModalOpen = $state(false);
  let agentModelModalOpen = $state(false);

  function handleAgentMessageChange(value: string): void {
    if (!agentTarget) return;
    setAgentTarget({ ...agentTarget, initialMessage: value });
  }

  // Highlight-only validation, mirroring JobEditor's submit-time checks.
  const promptModelInvalid = $derived(
    showError &&
      target.kind === "prompt" &&
      (!promptTarget || !promptTarget.providerId || !promptTarget.modelId),
  );
  const promptTextInvalid = $derived(
    showError &&
      target.kind === "prompt" &&
      (!promptTarget || promptTarget.prompt.trim().length === 0),
  );
  const agentInvalid = $derived(
    showError &&
      target.kind === "agent" &&
      (!agentTarget || !agentTarget.agentId),
  );
  const agentModelInvalid = $derived(
    showError &&
      target.kind === "agent" &&
      (!agentTarget || !agentTarget.modelId),
  );

</script>

<div class="flex flex-col gap-3">
  <Select
    label={t("jobs.target.kindLabel")}
    value={target.kind}
    onChange={handleKindChange}
    options={KIND_ITEMS}
    class="w-full"
  />

  {#if promptTarget}
    <div class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.target.modelLabel")}</span>
      <!-- Trigger styled to match Select (field--soft base + trailing chevrons) -->
      <button
        type="button"
        onclick={() => (promptModelModalOpen = true)}
        class="field field--soft flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm"
        class:is-error={promptModelInvalid}
      >
        {#if selectedPromptModel}
          <span class="truncate text-base-content">{selectedPromptModel.name}</span>
        {:else}
          <span class="text-base-content/50">{t("agent.input.selectModel")}</span>
        {/if}
        <ChevronsUpDown size={14} class="shrink-0 text-base-content/60" />
      </button>
      {#if promptModelInvalid}
        <span class="text-xs text-error">{t("jobs.target.modelRequired")}</span>
      {/if}
    </div>

    <ModelSelectModal
      bind:open={promptModelModalOpen}
      selectedModel={selectedPromptModel}
      onModelSelect={handleModelSelect}
    />

    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.target.promptLabel")}</span>
      <textarea
        aria-label={t("jobs.target.promptAria")}
        aria-invalid={promptTextInvalid}
        value={promptTarget.prompt}
        rows={5}
        placeholder={t("jobs.target.promptPlaceholder")}
        oninput={(e) =>
          handlePromptTextChange((e.currentTarget as HTMLTextAreaElement).value)}
        class="field w-full resize-y px-3 py-2.5 font-mono text-sm leading-relaxed"
        class:is-error={promptTextInvalid}
      ></textarea>
      {#if promptTextInvalid}
        <span class="text-xs text-error">{t("jobs.target.promptRequired")}</span>
      {/if}
    </label>
  {:else if agentTarget}
    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.target.agentLabel")}</span>
      {#if agentsLoading}
        <div class="text-sm text-base-content/50">{t("jobs.target.agentLoading")}</div>
      {:else if agents.length === 0}
        <div
          class="flex items-center gap-2 rounded-md border border-[var(--hairline)] bg-base-200 px-3 py-2 text-sm text-base-content/60"
        >
          <Bot size={14} class="flex-shrink-0" />
          <span>{t("jobs.target.agentEmpty")}</span>
        </div>
      {:else}
        <Select
          value={agentTarget.agentId}
          onChange={handleAgentChange}
          options={agents.map((a) => ({ value: a.id ?? "", label: a.name }))}
          placeholder={t("jobs.target.agentSelect")}
          invalid={agentInvalid}
          class="w-full"
        />
      {/if}
      {#if agentInvalid}
        <span class="text-xs text-error">{t("jobs.target.agentRequired")}</span>
      {/if}
    </label>

    <!-- Model to run the agent with (agent definitions carry no model; each job picks its own) -->
    <div class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80"
        >{t("jobs.target.modelLabel")}</span
      >
      <!-- Trigger styled to match Select (field--soft base + trailing chevrons) -->
      <button
        type="button"
        onclick={() => (agentModelModalOpen = true)}
        class="field field--soft flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm"
        class:is-error={agentModelInvalid}
      >
        {#if selectedAgentModel}
          <span class="truncate text-base-content">{selectedAgentModel.name}</span
          >
        {:else}
          <span class="text-base-content/50">{t("agent.input.selectModel")}</span>
        {/if}
        <ChevronsUpDown size={14} class="shrink-0 text-base-content/60" />
      </button>
      {#if agentModelInvalid}
        <span class="text-xs text-error">{t("jobs.target.modelRequired")}</span>
      {/if}
    </div>

    <ModelSelectModal
      bind:open={agentModelModalOpen}
      selectedModel={selectedAgentModel}
      onModelSelect={handleAgentModelSelect}
    />

    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.target.initialMessageLabel")}</span>
      <textarea
        aria-label={t("jobs.target.initialMessageAria")}
        value={agentTarget.initialMessage}
        rows={4}
        placeholder={t("jobs.target.initialMessagePlaceholder")}
        oninput={(e) =>
          handleAgentMessageChange(
            (e.currentTarget as HTMLTextAreaElement).value,
          )}
        class="field w-full resize-y px-3 py-2.5 font-mono text-sm leading-relaxed"
      ></textarea>
    </label>
  {/if}
</div>
