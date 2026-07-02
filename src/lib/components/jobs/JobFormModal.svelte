<script lang="ts">
  import FormModal from "$lib/components/ui/FormModal.svelte";
  import ScheduleEditor from "$lib/components/jobs/ScheduleEditor.svelte";
  import TargetPicker from "$lib/components/jobs/TargetPicker.svelte";
  import {
    providerState,
    providerActions,
  } from "$lib/states/provider.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { AppError } from "$lib/api";
  import { t } from "$lib/i18n";
  import type { Agent, Job, JobTarget } from "$lib/types";
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
    return {
      kind: "prompt",
      providerId: "",
      modelId: "",
      prompt: "",
      sessionStrategy: "new_session",
    };
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
      case "prompt":
        return (
          t.providerId.length > 0 &&
          t.modelId.length > 0 &&
          t.prompt.trim().length > 0
        );
      case "agent":
        return t.agentId.length > 0 && t.modelId.length > 0;
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
    onClose();
  }
</script>

<FormModal
  bind:open={localOpen}
  size="lg"
  title={job ? t("jobs.form.editTitle") : t("jobs.form.createTitle")}
  onClose={handleClose}
  {saving}
  hint={t("jobs.form.appClosedNotice")}
  error={saveError}
  submitLabel={saving
    ? t("jobs.form.saving")
    : job
      ? t("jobs.form.save")
      : t("jobs.form.createAction")}
  onSubmit={handleSave}
>
  <!-- 主区：大标题式名称 + 描述 + 任务目标（要跑什么） -->
  <div class="flex flex-col gap-1">
    <input
      class="modal-title-input"
      bind:value={form.name}
      placeholder={t("jobs.form.namePlaceholder")}
      aria-invalid={nameError != null}
    />
    {#if nameError}
      <span class="text-xs text-error">{nameError}</span>
    {/if}
    <input
      class="w-full bg-transparent text-sm text-base-content/80 outline-none placeholder:text-base-content/35"
      bind:value={form.description}
      placeholder={t("jobs.form.descriptionPlaceholder")}
    />
  </div>

  <div class="mt-6 flex flex-col gap-2.5">
    <span class="form-section-label">{t("jobs.form.sectionTarget")}</span>
    <TargetPicker
      bind:target={form.target}
      {providersWithModels}
      {agents}
      {agentsLoading}
      showError={showValidation}
    />
  </div>

  {#snippet aside()}
    <!-- 配置栏：调度 + 高级（超时/重试），紧凑纵排 -->
    <div class="flex flex-col gap-6 pt-1">
      <div class="flex flex-col gap-2.5">
        <span class="form-section-label">{t("jobs.form.sectionSchedule")}</span>
        <ScheduleEditor bind:cron={form.cronExpr} />
      </div>

      <div class="flex flex-col gap-3">
        <span class="form-section-label">{t("jobs.form.sectionAdvanced")}</span>

        <label class="flex flex-col gap-1 text-sm">
          <span class="text-xs text-base-content/70">{t("jobs.form.execTimeout")}</span>
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.execTimeoutSecs}
            placeholder={t("jobs.form.execTimeoutPlaceholder", { n: DEFAULT_EXEC_TIMEOUT_SECS })}
            aria-invalid={execTimeoutError != null}
            class="field w-full px-2.5 py-1.5 text-sm"
            class:is-error={execTimeoutError != null}
          />
          {#if execTimeoutError}
            <span class="text-xs text-error">{execTimeoutError}</span>
          {:else}
            <span class="text-xs text-base-content/50">{t("jobs.form.execTimeoutHint")}</span>
          {/if}
        </label>

        <label class="flex flex-col gap-1 text-sm">
          <span class="text-xs text-base-content/70">{t("jobs.form.maxRetries")}</span>
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.maxRetries}
            placeholder={t("jobs.form.maxRetriesPlaceholder", { n: DEFAULT_MAX_RETRIES })}
            aria-invalid={maxRetriesError != null}
            class="field w-full px-2.5 py-1.5 text-sm"
            class:is-error={maxRetriesError != null}
          />
          {#if maxRetriesError}
            <span class="text-xs text-error">{maxRetriesError}</span>
          {:else}
            <span class="text-xs text-base-content/50">{t("jobs.form.maxRetriesHint")}</span>
          {/if}
        </label>

        <label class="flex flex-col gap-1 text-sm">
          <span class="text-xs text-base-content/70">{t("jobs.form.retryDelay")}</span>
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.retryDelaySecs}
            placeholder={t("jobs.form.retryDelayPlaceholder", { n: DEFAULT_RETRY_DELAY_SECS })}
            aria-invalid={retryDelayError != null}
            class="field w-full px-2.5 py-1.5 text-sm"
            class:is-error={retryDelayError != null}
          />
          {#if retryDelayError}
            <span class="text-xs text-error">{retryDelayError}</span>
          {:else}
            <span class="text-xs text-base-content/50">{t("jobs.form.retryDelayHint", { n: DEFAULT_RETRY_DELAY_SECS })}</span>
          {/if}
        </label>
      </div>
    </div>
  {/snippet}
</FormModal>
