<script lang="ts">
  import { Package, Plus, Trash2, Bot } from "@lucide/svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import ChatModelSelectButton from "$lib/components/chat/ChatModelSelectButton.svelte";
  import { t } from "$lib/i18n";
  import type {
    Agent,
    AgentTarget,
    Artifact,
    ArtifactTarget,
    JobTarget,
    PromptTarget,
  } from "$lib/types";
  import type {
    ModelWithProvider,
    ProviderWithModels,
  } from "$lib/types/provider";

  interface Props {
    /**
     * 受控出口：当前任务目标配置。父组件（JobFormModal）用 `bind:target` 双向绑定。
     * 三类 kind（artifact / prompt / agent）共用此出口；切换 kind 时整体替换为
     * 对应 kind 的干净目标，故提交时绝无跨 kind 残留字段（VAL-TARGET-010）。
     */
    target: JobTarget;
    /** 已安装的 artifact 候选列表（由父组件加载后传入）。 */
    artifacts: Artifact[];
    /** 已启用的供应商（含其已启用模型），用于 prompt 目标解析所选模型展示名。 */
    providersWithModels?: ProviderWithModels[];
    /** Agent 模板候选列表（由父组件加载后传入），用于 agent 目标。 */
    agents?: Agent[];
    /** artifact 列表是否仍在加载（影响占位提示）。 */
    loading?: boolean;
    /** agent 列表是否仍在加载（影响占位提示）。 */
    agentsLoading?: boolean;
    /** 校验失败标记（如未选 artifact），由父组件在提交时打开以高亮。 */
    showError?: boolean;
  }

  let {
    target = $bindable(),
    artifacts,
    providersWithModels = [],
    agents = [],
    loading = false,
    agentsLoading = false,
    showError = false,
  }: Props = $props();

  // ──────────────────────────────────────────────────────────────────────
  // kind 选择器：三类目标均可选并各有配置面板。切换 kind 时整体替换 target
  // 为目标 kind 的空目标——这是字段隔离的单一实现点（VAL-TARGET-010）：旧 kind
  // 的字段随旧对象一起被丢弃，新对象只含新 kind 的字段，无需逐字段清理。
  // ──────────────────────────────────────────────────────────────────────
  type TargetKind = JobTarget["kind"];

  const KIND_ITEMS: { value: TargetKind; label: string }[] = [
    { value: "artifact", label: "Artifact" },
    { value: "prompt", label: "Prompt" },
    { value: "agent", label: "Agent" },
  ];

  // 各 kind 类型收窄的派生视图，供模板使用。
  const artifactTarget = $derived(target.kind === "artifact" ? target : null);
  const promptTarget = $derived(target.kind === "prompt" ? target : null);
  const agentTarget = $derived(target.kind === "agent" ? target : null);

  function emptyTargetOf(kind: TargetKind): JobTarget {
    switch (kind) {
      case "artifact":
        return { kind: "artifact", artifactId: "", args: [], env: {} };
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
  // Artifact 目标
  // ──────────────────────────────────────────────────────────────────────
  function setArtifactTarget(next: ArtifactTarget): void {
    target = next;
  }

  function handleArtifactChange(value: string): void {
    if (!artifactTarget) return;
    setArtifactTarget({ ...artifactTarget, artifactId: value });
  }

  // 参数 (args)：以 string[] 存储，UI 逐行编辑。
  function addArg(): void {
    if (!artifactTarget) return;
    setArtifactTarget({
      ...artifactTarget,
      args: [...(artifactTarget.args ?? []), ""],
    });
  }

  function updateArg(index: number, value: string): void {
    if (!artifactTarget) return;
    const args = [...(artifactTarget.args ?? [])];
    args[index] = value;
    setArtifactTarget({ ...artifactTarget, args });
  }

  function removeArg(index: number): void {
    if (!artifactTarget) return;
    const args = (artifactTarget.args ?? []).filter((_, i) => i !== index);
    setArtifactTarget({ ...artifactTarget, args });
  }

  // 环境变量 (env)：以 Record<string,string> 存储，UI 以 key/value 行编辑。
  // 内部用有序数组维护行（含空 key 的草稿行），提交时由父组件读取 target.env。
  let envRows = $state<{ key: string; value: string }[]>([]);

  // 当 target 从外部（编辑回填 / 重置 / 切到 artifact）变化时，把 env 同步成行视图。
  // 用「kind + artifactId」做同步键：切换 kind 或换 artifact 都触发重建，避免
  // 编辑过程中光标跳动，也避免从别的 kind 切回时残留上一份行。
  let syncedKey = $state<string | null>(null);
  $effect(() => {
    const t = artifactTarget;
    if (!t) {
      syncedKey = null;
      return;
    }
    const key = `artifact:${t.artifactId}`;
    if (key !== syncedKey) {
      syncedKey = key;
      envRows = Object.entries(t.env ?? {}).map(([key, value]) => ({
        key,
        value,
      }));
    }
  });

  /** 从行视图重建 Record 并写回 target.env（跳过空 key）。 */
  function commitEnv(rows: { key: string; value: string }[]): void {
    if (!artifactTarget) return;
    const env: Record<string, string> = {};
    for (const row of rows) {
      const key = row.key.trim();
      if (key) env[key] = row.value;
    }
    setArtifactTarget({ ...artifactTarget, env });
  }

  function addEnvRow(): void {
    envRows = [...envRows, { key: "", value: "" }];
  }

  function updateEnvKey(index: number, key: string): void {
    envRows = envRows.map((row, i) => (i === index ? { ...row, key } : row));
    commitEnv(envRows);
  }

  function updateEnvValue(index: number, value: string): void {
    envRows = envRows.map((row, i) => (i === index ? { ...row, value } : row));
    commitEnv(envRows);
  }

  function removeEnvRow(index: number): void {
    envRows = envRows.filter((_, i) => i !== index);
    commitEnv(envRows);
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
  // - artifact：未选 artifact 无效（VAL-TARGET-011）
  // - prompt：未选模型（缺 provider/model）或 prompt 文本空白无效（012 / 013）
  // - agent：未选 agent 模板无效（014）
  // ──────────────────────────────────────────────────────────────────────
  const artifactInvalid = $derived(
    showError &&
      target.kind === "artifact" &&
      (!artifactTarget || !artifactTarget.artifactId),
  );
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

  {#if artifactTarget}
    <!-- Artifact 选择 -->
    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.target.artifactLabel")}</span>
      {#if loading}
        <div class="text-sm text-base-content/50">{t("jobs.target.artifactLoading")}</div>
      {:else if artifacts.length === 0}
        <div
          class="flex items-center gap-2 rounded-md border border-[var(--hairline)] bg-base-200 px-3 py-2 text-sm text-base-content/60"
        >
          <Package size={14} class="flex-shrink-0" />
          <span>{t("jobs.target.artifactEmpty")}</span>
        </div>
      {:else}
        <select
          aria-label={t("jobs.target.artifactLabel")}
          aria-invalid={artifactInvalid}
          value={artifactTarget.artifactId}
          onchange={(e) =>
            handleArtifactChange((e.currentTarget as HTMLSelectElement).value)}
          class="{inputClass} cursor-pointer appearance-none {artifactInvalid
            ? 'border-error ring-1 ring-error'
            : ''}"
        >
          <option value="" disabled>{t("jobs.target.artifactSelect")}</option>
          {#each artifacts as artifact (artifact.id)}
            <option value={artifact.id}>{artifact.name}</option>
          {/each}
        </select>
      {/if}
      {#if artifactInvalid}
        <span class="text-xs text-error">{t("jobs.target.artifactRequired")}</span>
      {/if}
    </label>

    <!-- 参数 args -->
    <div class="flex flex-col gap-1.5 text-sm">
      <div class="flex items-center justify-between">
        <span class="font-medium text-base-content/80">{t("jobs.target.argsLabel")}</span>
        <button
          type="button"
          onclick={addArg}
          class="flex items-center gap-1 text-xs text-primary hover:underline"
        >
          <Plus size={12} /> {t("jobs.target.argsAdd")}
        </button>
      </div>
      {#if (artifactTarget.args ?? []).length === 0}
        <p class="text-xs text-base-content/50">{t("jobs.target.argsEmpty")}</p>
      {:else}
        {#each artifactTarget.args ?? [] as arg, index (index)}
          <div class="flex items-center gap-2">
            <input
              type="text"
              aria-label={t("jobs.target.argAria", { n: index + 1 })}
              value={arg}
              oninput={(e) =>
                updateArg(index, (e.currentTarget as HTMLInputElement).value)}
              placeholder={t("jobs.target.argPlaceholder")}
              class={inputClass}
            />
            <button
              type="button"
              aria-label={t("jobs.target.argRemoveAria", { n: index + 1 })}
              onclick={() => removeArg(index)}
              class="flex-shrink-0 rounded-md p-1.5 text-base-content/50 hover:bg-error/10 hover:text-error"
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      {/if}
    </div>

    <!-- 环境变量 env -->
    <div class="flex flex-col gap-1.5 text-sm">
      <div class="flex items-center justify-between">
        <span class="font-medium text-base-content/80">{t("jobs.target.envLabel")}</span>
        <button
          type="button"
          onclick={addEnvRow}
          class="flex items-center gap-1 text-xs text-primary hover:underline"
        >
          <Plus size={12} /> {t("jobs.target.envAdd")}
        </button>
      </div>
      {#if envRows.length === 0}
        <p class="text-xs text-base-content/50">{t("jobs.target.envEmpty")}</p>
      {:else}
        {#each envRows as row, index (index)}
          <div class="flex items-center gap-2">
            <input
              type="text"
              aria-label={t("jobs.target.envKeyAria", { n: index + 1 })}
              value={row.key}
              oninput={(e) =>
                updateEnvKey(index, (e.currentTarget as HTMLInputElement).value)}
              placeholder="KEY"
              class="{inputClass} font-mono"
            />
            <input
              type="text"
              aria-label={t("jobs.target.envValueAria", { n: index + 1 })}
              value={row.value}
              oninput={(e) =>
                updateEnvValue(
                  index,
                  (e.currentTarget as HTMLInputElement).value,
                )}
              placeholder="value"
              class={inputClass}
            />
            <button
              type="button"
              aria-label={t("jobs.target.envRemoveAria", { n: index + 1 })}
              onclick={() => removeEnvRow(index)}
              class="flex-shrink-0 rounded-md p-1.5 text-base-content/50 hover:bg-error/10 hover:text-error"
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      {/if}
    </div>
  {:else if promptTarget}
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
