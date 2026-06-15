<script lang="ts">
  import { Package, Plus, Trash2 } from "@lucide/svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import type { Artifact, ArtifactTarget, JobTarget } from "$lib/types";

  interface Props {
    /**
     * 受控出口：当前任务目标配置。父组件（JobFormModal）用 `bind:target` 双向绑定。
     * 本期仅支持 artifact，故内部恒为 `ArtifactTarget`；保存所用目标即此值。
     */
    target: JobTarget;
    /** 已安装的 artifact 候选列表（由父组件加载后传入）。 */
    artifacts: Artifact[];
    /** artifact 列表是否仍在加载（影响占位提示）。 */
    loading?: boolean;
    /** 校验失败标记（如未选 artifact），由父组件在提交时打开以高亮。 */
    showError?: boolean;
  }

  let {
    target = $bindable(),
    artifacts,
    loading = false,
    showError = false,
  }: Props = $props();

  // ──────────────────────────────────────────────────────────────────────
  // kind 选择器：本期只实现 artifact 配置。prompt/agent 占位呈现「即将支持」并禁用，
  // 其配置 / 字段隔离留给 M3（target-picker-llm）。切换框架在此预留，扩展时只需为
  // 各 kind 补一个配置面板并放开 disabled。
  // ──────────────────────────────────────────────────────────────────────
  type TargetKind = JobTarget["kind"];

  const KIND_ITEMS: { value: TargetKind; label: string; enabled: boolean }[] = [
    { value: "artifact", label: "Artifact", enabled: true },
    { value: "agent", label: "Agent（即将支持）", enabled: false },
    { value: "prompt", label: "Prompt（即将支持）", enabled: false },
  ];

  // 本期内 target 始终是 artifact；提供类型收窄的派生视图供模板使用。
  const artifactTarget = $derived(
    target.kind === "artifact" ? target : null,
  );

  /** 单一出口：写回 bindable artifact target。 */
  function setArtifactTarget(next: ArtifactTarget): void {
    target = next;
  }

  function handleKindChange(value: string): void {
    // 本期仅 artifact 可选；其余 kind 在 Select 中已 disabled，这里防御性忽略。
    if (value !== "artifact") return;
    if (target.kind === "artifact") return;
    setArtifactTarget({ kind: "artifact", artifactId: "", args: [], env: {} });
  }

  function handleArtifactChange(value: string): void {
    if (!artifactTarget) return;
    setArtifactTarget({ ...artifactTarget, artifactId: value });
  }

  // ──────────────────────────────────────────────────────────────────────
  // 参数 (args)：以 string[] 存储，UI 逐行编辑。
  // ──────────────────────────────────────────────────────────────────────
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

  // ──────────────────────────────────────────────────────────────────────
  // 环境变量 (env)：以 Record<string,string> 存储，UI 以 key/value 行编辑。
  // 内部用有序数组维护行（含空 key 的草稿行），提交时由父组件读取 target.env。
  // ──────────────────────────────────────────────────────────────────────
  let envRows = $state<{ key: string; value: string }[]>([]);

  // 当 target 从外部（编辑回填 / 重置）变化时，把 env 同步成行视图。
  // 仅在 artifactId 改变或行数与 env 不一致时重建，避免编辑过程中光标跳动。
  let syncedArtifactId = $state<string | null>(null);
  $effect(() => {
    const t = artifactTarget;
    if (!t) return;
    if (t.artifactId !== syncedArtifactId) {
      syncedArtifactId = t.artifactId;
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

  // 校验：未选 artifact → 无效（VAL-TARGET-011）。
  const artifactInvalid = $derived(
    showError && (!artifactTarget || !artifactTarget.artifactId),
  );

  const inputClass =
    "w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary";
</script>

<div class="flex flex-col gap-3">
  <!-- 目标类型 -->
  <Select
    label="目标类型"
    value={target.kind}
    onChange={handleKindChange}
    class="w-full"
  >
    {#each KIND_ITEMS as item (item.value)}
      <option value={item.value} disabled={!item.enabled}>{item.label}</option>
    {/each}
  </Select>

  {#if artifactTarget}
    <!-- Artifact 选择 -->
    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">运行的 Artifact</span>
      {#if loading}
        <div class="text-sm text-base-content/50">加载 Artifact 列表…</div>
      {:else if artifacts.length === 0}
        <div
          class="flex items-center gap-2 rounded-md border border-[var(--hairline)] bg-base-200 px-3 py-2 text-sm text-base-content/60"
        >
          <Package size={14} class="flex-shrink-0" />
          <span>暂无已安装的 Artifact，请先在 Artifact 页安装。</span>
        </div>
      {:else}
        <select
          aria-label="运行的 Artifact"
          aria-invalid={artifactInvalid}
          value={artifactTarget.artifactId}
          onchange={(e) =>
            handleArtifactChange((e.currentTarget as HTMLSelectElement).value)}
          class="{inputClass} cursor-pointer appearance-none {artifactInvalid
            ? 'border-error ring-1 ring-error'
            : ''}"
        >
          <option value="" disabled>请选择 Artifact</option>
          {#each artifacts as artifact (artifact.id)}
            <option value={artifact.id}>{artifact.name}</option>
          {/each}
        </select>
      {/if}
      {#if artifactInvalid}
        <span class="text-xs text-error">请选择一个 Artifact</span>
      {/if}
    </label>

    <!-- 参数 args -->
    <div class="flex flex-col gap-1.5 text-sm">
      <div class="flex items-center justify-between">
        <span class="font-medium text-base-content/80">命令行参数</span>
        <button
          type="button"
          onclick={addArg}
          class="flex items-center gap-1 text-xs text-primary hover:underline"
        >
          <Plus size={12} /> 添加参数
        </button>
      </div>
      {#if (artifactTarget.args ?? []).length === 0}
        <p class="text-xs text-base-content/50">无额外参数</p>
      {:else}
        {#each artifactTarget.args ?? [] as arg, index (index)}
          <div class="flex items-center gap-2">
            <input
              type="text"
              aria-label={`参数 ${index + 1}`}
              value={arg}
              oninput={(e) =>
                updateArg(index, (e.currentTarget as HTMLInputElement).value)}
              placeholder="--flag 或 value"
              class={inputClass}
            />
            <button
              type="button"
              aria-label={`删除参数 ${index + 1}`}
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
        <span class="font-medium text-base-content/80">环境变量</span>
        <button
          type="button"
          onclick={addEnvRow}
          class="flex items-center gap-1 text-xs text-primary hover:underline"
        >
          <Plus size={12} /> 添加变量
        </button>
      </div>
      {#if envRows.length === 0}
        <p class="text-xs text-base-content/50">无环境变量</p>
      {:else}
        {#each envRows as row, index (index)}
          <div class="flex items-center gap-2">
            <input
              type="text"
              aria-label={`环境变量名 ${index + 1}`}
              value={row.key}
              oninput={(e) =>
                updateEnvKey(index, (e.currentTarget as HTMLInputElement).value)}
              placeholder="KEY"
              class="{inputClass} font-mono"
            />
            <input
              type="text"
              aria-label={`环境变量值 ${index + 1}`}
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
              aria-label={`删除环境变量 ${index + 1}`}
              onclick={() => removeEnvRow(index)}
              class="flex-shrink-0 rounded-md p-1.5 text-base-content/50 hover:bg-error/10 hover:text-error"
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>
