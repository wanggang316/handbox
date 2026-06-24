<script lang="ts">
  import { Bot } from "@lucide/svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { t } from "$lib/i18n";
  import type { Agent, AgentTarget, JobTarget, PromptTarget } from "$lib/types";
  import type {
    ModelWithProvider,
    ProviderWithModels,
  } from "$lib/types/provider";

  interface Props {
    /**
     * 受控出口：当前任务目标配置。父组件（JobFormModal）用 `bind:target` 双向绑定。
     * 两类 kind（prompt / agent）共用此出口；切换 kind 时整体替换为对应 kind 的
     * 干净目标，故提交时绝无跨 kind 残留字段（VAL-TARGET-010）。
     */
    target: JobTarget;
    /** 已启用的供应商（含其已启用模型），用于 prompt 目标解析所选模型展示名。 */
    providersWithModels?: ProviderWithModels[];
    /** Agent 模板候选列表（由父组件加载后传入），用于 agent 目标。 */
    agents?: Agent[];
    /** agent 列表是否仍在加载（影响占位提示）。 */
    agentsLoading?: boolean;
    /** 校验失败标记（如未选 agent），由父组件在提交时打开以高亮。 */
    showError?: boolean;
  }

  let {
    target = $bindable(),
    providersWithModels = [],
    agents = [],
    agentsLoading = false,
    showError = false,
  }: Props = $props();

  // ──────────────────────────────────────────────────────────────────────
  // kind 选择器：两类目标均可选并各有配置面板。切换 kind 时整体替换 target
  // 为目标 kind 的空目标——这是字段隔离的单一实现点（VAL-TARGET-010）：旧 kind
  // 的字段随旧对象一起被丢弃，新对象只含新 kind 的字段，无需逐字段清理。
  // ──────────────────────────────────────────────────────────────────────
  type TargetKind = JobTarget["kind"];

  const KIND_ITEMS: { value: TargetKind; label: string }[] = [
    { value: "prompt", label: "Prompt" },
    { value: "agent", label: "Agent" },
  ];

  // 各 kind 类型收窄的派生视图，供模板使用。
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
        return { kind: "agent", agentId: "", initialMessage: "" };
    }
  }

  function handleKindChange(value: string): void {
    const kind = value as TargetKind;
    if (kind === target.kind) return;
    // 字段隔离：整体替换为目标 kind 的空目标，旧 kind 字段不会残留。
    target = emptyTargetOf(kind);
  }

  // ──────────────────────────────────────────────────────────────────────
  // Prompt 目标：provider/model 级联（复用 chat 的模型选择弹窗）+ prompt 文本。
  // 内部以 (providerId, modelId) 存储；展示名从 providersWithModels 解析。
  // ──────────────────────────────────────────────────────────────────────
  function setPromptTarget(next: PromptTarget): void {
    target = next;
  }

  // 把当前 (providerId, modelId) 解析为 ChatModelSelectButton 需要的
  // ModelWithProvider；解析不到（模型已删/未加载）返回 null → 显示「选择模型」。
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

  // ──────────────────────────────────────────────────────────────────────
  // Agent 目标：选择 agent 模板 + 初始指令。
  // ──────────────────────────────────────────────────────────────────────
  function setAgentTarget(next: AgentTarget): void {
    target = next;
  }

  function handleAgentChange(value: string): void {
    if (!agentTarget) return;
    setAgentTarget({ ...agentTarget, agentId: value });
  }

  function handleAgentMessageChange(value: string): void {
    if (!agentTarget) return;
    setAgentTarget({ ...agentTarget, initialMessage: value });
  }

  // ──────────────────────────────────────────────────────────────────────
  // 校验（与父组件 JobFormModal 的提交校验同源；这里仅负责高亮提示）：
  // - prompt：未选模型（缺 provider/model）或 prompt 文本空白无效（012 / 013）
  // - agent：未选 agent 模板无效（014）
  // ──────────────────────────────────────────────────────────────────────
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

  const inputClass =
    "w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary";
</script>

<div class="flex flex-col gap-3">
  <!-- 目标类型 -->
  <Select
    label={t("jobs.target.kindLabel")}
    value={target.kind}
    onChange={handleKindChange}
    class="w-full"
  >
    {#each KIND_ITEMS as item (item.value)}
      <option value={item.value}>{item.label}</option>
    {/each}
  </Select>

  {#if promptTarget}
    <!-- Prompt：provider/model 级联（复用 chat 模型选择弹窗）+ prompt 文本 -->
    <div class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.target.modelLabel")}</span>
      <div
        class="flex items-center gap-2 rounded-md border px-1 py-1 {promptModelInvalid
          ? 'border-error ring-1 ring-error'
          : 'border-[var(--hairline)]'}"
      >
        <ChatModelSelectButton
          selectedModel={selectedPromptModel}
          onModelSelect={handleModelSelect}
          variant="ghost"
        />
        {#if selectedPromptModel}
          <span class="text-xs text-base-content/50">
            {selectedPromptModel.providerName}
          </span>
        {/if}
      </div>
      {#if promptModelInvalid}
        <span class="text-xs text-error">{t("jobs.target.modelRequired")}</span>
      {/if}
    </div>

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
        class="{inputClass} resize-none {promptTextInvalid
          ? 'border-error ring-1 ring-error'
          : ''}"
      ></textarea>
      {#if promptTextInvalid}
        <span class="text-xs text-error">{t("jobs.target.promptRequired")}</span>
      {/if}
    </label>
  {:else if agentTarget}
    <!-- Agent：选择 agent 模板 + 初始指令 -->
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
        <select
          aria-label={t("jobs.target.agentLabel")}
          aria-invalid={agentInvalid}
          value={agentTarget.agentId}
          onchange={(e) =>
            handleAgentChange((e.currentTarget as HTMLSelectElement).value)}
          class="{inputClass} cursor-pointer appearance-none {agentInvalid
            ? 'border-error ring-1 ring-error'
            : ''}"
        >
          <option value="" disabled>{t("jobs.target.agentSelect")}</option>
          {#each agents as agent (agent.id)}
            <option value={agent.id}>{agent.name}</option>
          {/each}
        </select>
      {/if}
      {#if agentInvalid}
        <span class="text-xs text-error">{t("jobs.target.agentRequired")}</span>
      {/if}
    </label>

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
        class="{inputClass} resize-none"
      ></textarea>
    </label>
  {/if}
</div>
