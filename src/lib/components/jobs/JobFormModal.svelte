<script lang="ts">
  import { AlertCircle, Info } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import ScheduleEditor from "$lib/components/jobs/ScheduleEditor.svelte";
  import TargetPicker from "$lib/components/jobs/TargetPicker.svelte";
  import { artifactState } from "$lib/states/artifact.svelte";
  import {
    providerState,
    providerActions,
  } from "$lib/states/provider.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { AppError } from "$lib/api";
  import { t } from "$lib/i18n";
  import type {
    Agent,
    Artifact,
    Job,
    JobTarget,
  } from "$lib/types";
  import {
    DEFAULT_EXEC_TIMEOUT_SECS,
    DEFAULT_MAX_RETRIES,
    DEFAULT_RETRY_DELAY_SECS,
  } from "$lib/types/job";
  import type { ProviderWithModels } from "$lib/types/provider";

  /** 父组件保存所需的表单出参（与 JobCreateInput / JobUpdateInput 对齐的子集）。 */
  export interface JobFormData {
    name: string;
    description?: string;
    target: JobTarget;
    cronExpr: string;
    timezone: string;
    enabled: boolean;
    /** 每次运行超时（秒）；undefined 表示留空、由后端回填具名默认。 */
    execTimeoutSecs?: number;
    /** 最大重试次数；undefined 表示留空、由后端回填具名默认。 */
    maxRetries?: number;
    /** 重试间隔（秒）；undefined 表示留空、由后端回填具名默认。 */
    retryDelaySecs?: number;
  }

  interface Props {
    open: boolean;
    /** 编辑模式传入现有任务；创建模式传 null。 */
    job: Job | null;
    onClose: () => void;
    /**
     * 保存回调。父组件负责调用 jobStore.create/update（落库后再更新 UI，避免 ghost 卡片），
     * 失败时 throw，本组件捕获并展示错误且保持表单打开。
     */
    onSave: (data: JobFormData) => Promise<void>;
  }

  let { open, job, onClose, onSave }: Props = $props();

  // ──────────────────────────────────────────────────────────────────────
  // 表单状态。本地浅拷贝，取消 / 关闭丢弃，不影响外部 job。
  // ──────────────────────────────────────────────────────────────────────
  const DEFAULT_CRON = "0 9 * * *";
  const localTimezone = Intl.DateTimeFormat().resolvedOptions().timeZone;

  function emptyTarget(): JobTarget {
    return { kind: "artifact", artifactId: "", args: [], env: {} };
  }

  interface FormState {
    name: string;
    description: string;
    cronExpr: string;
    timezone: string;
    enabled: boolean;
    target: JobTarget;
    // 健壮性字段以字符串持有，空串表示「留空」→ 保存时映射为 undefined（用后端默认）。
    execTimeoutSecs: string;
    maxRetries: string;
    retryDelaySecs: string;
  }

  function blankForm(): FormState {
    return {
      name: "",
      description: "",
      cronExpr: DEFAULT_CRON,
      timezone: localTimezone,
      enabled: true,
      target: emptyTarget(),
      execTimeoutSecs: "",
      maxRetries: "",
      retryDelaySecs: "",
    };
  }

  let form = $state<FormState>(blankForm());
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let showValidation = $state(false);

  // 同步外部 open 到本地，用于驱动 Modal 关闭动画。
  let localOpen = $state(false);

  // 用 job.id（或 null）作为「打开了哪个表单」的标识：从关闭→打开、或切换编辑目标时
  // 才重置表单，避免编辑过程中外部 store 刷新覆盖用户输入。
  let loadedKey = $state<string | null>(null);

  $effect(() => {
    localOpen = open;
    if (!open) {
      // 关闭后清理标识，下次打开必定重新回填（不残留上次草稿）。
      loadedKey = null;
      return;
    }

    const key = job?.id ?? "__create__";
    if (key === loadedKey) return;
    loadedKey = key;
    resetForm();
  });

  function resetForm(): void {
    saveError = null;
    showValidation = false;
    saving = false;
    if (job) {
      form = {
        name: job.name,
        description: job.description ?? "",
        cronExpr: job.cronExpr,
        timezone: job.timezone || localTimezone,
        enabled: job.enabled,
        // 深拷贝目标（$state.snapshot 返回非代理深拷贝），避免修改外部 job 引用。
        target: $state.snapshot(job.target) as JobTarget,
        // 编辑模式回填已存值（包括 0，因为 0 是有意义的「不限/不重试」）。
        execTimeoutSecs: String(job.execTimeoutSecs),
        maxRetries: String(job.maxRetries),
        retryDelaySecs: String(job.retryDelaySecs),
      };
    } else {
      form = blankForm();
    }
  }

  // ──────────────────────────────────────────────────────────────────────
  // Artifact 候选：打开时加载已安装的 artifact 列表。
  // ──────────────────────────────────────────────────────────────────────
  let artifacts = $state<Artifact[]>([]);
  let artifactsLoading = $state(false);

  $effect(() => {
    if (!open) return;
    artifactsLoading = true;
    artifactState
      .loadArtifacts({ isInstalled: true })
      .then(() => {
        artifacts = artifactState.artifacts.filter((a) => a.isInstalled);
      })
      .catch((e) => {
        console.error("Failed to load artifacts for job target:", e);
        artifacts = [];
      })
      .finally(() => {
        artifactsLoading = false;
      });
  });

  // ──────────────────────────────────────────────────────────────────────
  // Prompt / Agent 候选：打开时加载已启用供应商（含模型）与 Agent 模板列表。
  // providersWithModels 用于把目标里存的 (providerId, modelId) 解析为展示名；
  // agents 用于 agent 目标的模板下拉。读自共享状态，TargetPicker 不直接触状态。
  // ──────────────────────────────────────────────────────────────────────
  let providersWithModels = $state<ProviderWithModels[]>([]);
  let agents = $state<Agent[]>([]);
  let agentsLoading = $state(false);

  $effect(() => {
    if (!open) return;
    if (
      providerState.providersWithModelsNeedRefresh ||
      providerState.providersWithModels.length === 0
    ) {
      providerActions.loadProvidersWithModels().catch((e) => {
        console.error("Failed to load providers for job target:", e);
      });
    }
  });

  // providersWithModels 只取已启用供应商 + 其已启用模型，与 chat 模型选择口径一致。
  $effect(() => {
    providersWithModels = providerState.providersWithModels
      .filter((p) => p.enabled)
      .map((p) => ({ ...p, models: p.models.filter((m) => m.enabled) }));
  });

  $effect(() => {
    if (!open) return;
    agentsLoading = true;
    agentActions
      .loadAgents()
      .then(() => {
        agents = agentState.agents;
      })
      .catch((e) => {
        console.error("Failed to load agents for job target:", e);
        agents = [];
      })
      .finally(() => {
        agentsLoading = false;
      });
  });

  // ──────────────────────────────────────────────────────────────────────
  // 校验。目标按 kind 分支校验（与 TargetPicker 的高亮提示同源）：
  // - artifact：必须选中 artifact（VAL-TARGET-011）
  // - prompt：必须同时选中 provider 与 model（VAL-TARGET-013），且 prompt
  //   文本非空白（VAL-TARGET-012）
  // - agent：必须选中 agent 模板（VAL-TARGET-014）
  // 任一不满足都不调用 onSave（即不写库）。
  // ──────────────────────────────────────────────────────────────────────
  const nameError = $derived(
    showValidation && form.name.trim().length === 0
      ? t("jobs.form.nameRequired")
      : null,
  );

  // 健壮性字段：留空合法（保存映射为 undefined，由后端回填具名默认）；
  // 非空必须是非负整数，否则即时报错（VAL-ROBUST-003 前端侧）。
  function robustnessError(raw: string, label: string): string | null {
    const trimmed = raw.trim();
    if (trimmed.length === 0) return null; // 留空 → 用默认
    const n = Number(trimmed);
    if (!Number.isInteger(n)) return t("jobs.form.mustBeInteger", { label });
    if (n < 0) return t("jobs.form.mustNotBeNegative", { label });
    return null;
  }

  const execTimeoutError = $derived(
    showValidation
      ? robustnessError(form.execTimeoutSecs, t("jobs.form.execTimeoutLabel"))
      : null,
  );
  const maxRetriesError = $derived(
    showValidation
      ? robustnessError(form.maxRetries, t("jobs.form.maxRetriesLabel"))
      : null,
  );
  const retryDelayError = $derived(
    showValidation
      ? robustnessError(form.retryDelaySecs, t("jobs.form.retryDelayLabel"))
      : null,
  );

  /** 把健壮性输入解析为保存值：空串 → undefined（用默认），否则解析为整数。 */
  function parseRobustness(raw: string): number | undefined {
    const trimmed = raw.trim();
    if (trimmed.length === 0) return undefined;
    return Number(trimmed);
  }

  const targetValid = $derived.by((): boolean => {
    const t = form.target;
    switch (t.kind) {
      case "artifact":
        return t.artifactId.length > 0;
      case "prompt":
        return (
          t.providerId.length > 0 &&
          t.modelId.length > 0 &&
          t.prompt.trim().length > 0
        );
      case "agent":
        return t.agentId.length > 0;
    }
  });

  function validate(): boolean {
    showValidation = true;
    if (form.name.trim().length === 0) return false;
    if (!targetValid) return false;
    if (robustnessError(form.execTimeoutSecs, t("jobs.form.execTimeoutLabel")))
      return false;
    if (robustnessError(form.maxRetries, t("jobs.form.maxRetriesLabel")))
      return false;
    if (robustnessError(form.retryDelaySecs, t("jobs.form.retryDelayLabel")))
      return false;
    return true;
  }

  // ──────────────────────────────────────────────────────────────────────
  // 保存：先校验，再委托父组件落库；失败保留表单 + 显示错误（不乐观更新）。
  // ──────────────────────────────────────────────────────────────────────
  async function handleSave(): Promise<void> {
    if (!validate()) return;
    saving = true;
    saveError = null;
    try {
      await onSave({
        name: form.name.trim(),
        description: form.description.trim() ? form.description : undefined,
        target: $state.snapshot(form.target) as JobTarget,
        cronExpr: form.cronExpr.trim(),
        timezone: form.timezone,
        enabled: form.enabled,
        execTimeoutSecs: parseRobustness(form.execTimeoutSecs),
        maxRetries: parseRobustness(form.maxRetries),
        retryDelaySecs: parseRobustness(form.retryDelaySecs),
      });
      // 成功：触发关闭动画。
      localOpen = false;
    } catch (e) {
      saveError =
        e instanceof AppError
          ? e.message
          : e instanceof Error
            ? e.message
            : t("jobs.form.saveFailed");
    } finally {
      saving = false;
    }
  }

  function handleClose(): void {
    if (saving) return;
    localOpen = false;
  }
</script>

<Modal
  bind:open={localOpen}
  title={job ? t("jobs.form.editTitle") : t("jobs.form.createTitle")}
  closeOnBackdropClick={true}
  onClose={onClose}
>
  <div
    class="w-[600px] max-h-[80vh] overflow-y-auto px-6 pt-16 pb-6 flex flex-col gap-5"
  >
    <!-- 应用关闭期间不执行 提示 (JOBMGMT-027) -->
    <div
      class="flex items-start gap-2 rounded-md border border-info/30 bg-info/10 px-3 py-2 text-sm text-base-content/80"
    >
      <Info size={16} class="mt-0.5 flex-shrink-0 text-info" />
      <span>{t("jobs.form.appClosedNotice")}</span>
    </div>

    <!-- 基本信息 -->
    <TableGroup title={t("jobs.form.sectionBasic")}>
      <div class="flex flex-col gap-3 px-6 py-4">
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.form.name")}</span>
          <input
            type="text"
            bind:value={form.name}
            placeholder={t("jobs.form.namePlaceholder")}
            aria-invalid={nameError != null}
            class="w-full rounded-md border bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary {nameError
              ? 'border-error ring-1 ring-error'
              : 'border-[var(--hairline)]'}"
          />
          {#if nameError}
            <span class="text-xs text-error">{nameError}</span>
          {/if}
        </label>

        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.form.descriptionOptional")}</span>
          <Textarea bind:value={form.description} placeholder={t("jobs.form.descriptionPlaceholder")} rows={3} />
        </label>
      </div>
    </TableGroup>

    <!-- 调度 -->
    <TableGroup title={t("jobs.form.sectionSchedule")}>
      <div class="px-6 py-4">
        <ScheduleEditor bind:cron={form.cronExpr} />
      </div>
    </TableGroup>

    <!-- 目标 -->
    <TableGroup title={t("jobs.form.sectionTarget")}>
      <div class="px-6 py-4">
        <TargetPicker
          bind:target={form.target}
          {artifacts}
          {providersWithModels}
          {agents}
          loading={artifactsLoading}
          {agentsLoading}
          showError={showValidation}
        />
      </div>
    </TableGroup>

    <!-- 高级（健壮性）：超时 / 重试。留空采用具名默认。 -->
    <TableGroup title={t("jobs.form.sectionAdvanced")}>
      <div class="flex flex-col gap-3 px-6 py-4">
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.form.execTimeout")}</span>
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.execTimeoutSecs}
            placeholder={t("jobs.form.execTimeoutPlaceholder", { n: DEFAULT_EXEC_TIMEOUT_SECS })}
            aria-invalid={execTimeoutError != null}
            class="w-full rounded-md border bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary {execTimeoutError
              ? 'border-error ring-1 ring-error'
              : 'border-[var(--hairline)]'}"
          />
          {#if execTimeoutError}
            <span class="text-xs text-error">{execTimeoutError}</span>
          {:else}
            <span class="text-xs text-base-content/50">{t("jobs.form.execTimeoutHint")}</span>
          {/if}
        </label>

        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.form.maxRetries")}</span>
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.maxRetries}
            placeholder={t("jobs.form.maxRetriesPlaceholder", { n: DEFAULT_MAX_RETRIES })}
            aria-invalid={maxRetriesError != null}
            class="w-full rounded-md border bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary {maxRetriesError
              ? 'border-error ring-1 ring-error'
              : 'border-[var(--hairline)]'}"
          />
          {#if maxRetriesError}
            <span class="text-xs text-error">{maxRetriesError}</span>
          {:else}
            <span class="text-xs text-base-content/50">{t("jobs.form.maxRetriesHint")}</span>
          {/if}
        </label>

        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.form.retryDelay")}</span>
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.retryDelaySecs}
            placeholder={t("jobs.form.retryDelayPlaceholder", { n: DEFAULT_RETRY_DELAY_SECS })}
            aria-invalid={retryDelayError != null}
            class="w-full rounded-md border bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary {retryDelayError
              ? 'border-error ring-1 ring-error'
              : 'border-[var(--hairline)]'}"
          />
          {#if retryDelayError}
            <span class="text-xs text-error">{retryDelayError}</span>
          {:else}
            <span class="text-xs text-base-content/50">{t("jobs.form.retryDelayHint", { n: DEFAULT_RETRY_DELAY_SECS })}</span>
          {/if}
        </label>
      </div>
    </TableGroup>

    <!-- 保存错误 -->
    {#if saveError}
      <div class="flex items-start gap-2 text-sm text-error">
        <AlertCircle size={16} class="mt-0.5 flex-shrink-0" />
        <span>{saveError}</span>
      </div>
    {/if}

    <!-- 底部按钮 -->
    <div class="flex items-center justify-end gap-3 pt-4 border-t border-base-300">
      <Button variant="ghost" onclick={handleClose} disabled={saving}>
        {t("common.cancel")}
      </Button>
      <Button variant="primary" onclick={handleSave} disabled={saving}>
        {saving ? t("jobs.form.saving") : job ? t("jobs.form.save") : t("jobs.form.createAction")}
      </Button>
    </div>
  </div>
</Modal>
