<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ArrowLeft, Save } from "@lucide/svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import { TableGroup, TableBaseRow } from "$lib/components/ui/table";
  import ScheduleEditor from "$lib/components/jobs/ScheduleEditor.svelte";
  import TargetPicker from "$lib/components/jobs/TargetPicker.svelte";
  import { providerState, providerActions } from "$lib/states/provider.svelte";
  import { agentState, agentActions } from "$lib/states/agent.svelte";
  import { jobStore } from "$lib/stores/jobStore.svelte";
  import { AppError } from "$lib/api";
  import { t } from "$lib/i18n";
  import type { Agent, Job, JobTarget } from "$lib/types";
  import {
    DEFAULT_EXEC_TIMEOUT_SECS,
    DEFAULT_MAX_RETRIES,
    DEFAULT_RETRY_DELAY_SECS,
  } from "$lib/types/job";
  import type { ProviderWithModels } from "$lib/types/provider";

  interface Props {
    /** 编辑模式传入现有任务；创建模式传 null。 */
    job?: Job | null;
  }

  let { job = null }: Props = $props();

  // ──────────────────────────────────────────────────────────────────────
  // 表单状态。本地浅拷贝，离开页面丢弃，不影响外部 job。
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

  function initialForm(): FormState {
    if (job) {
      return {
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
    }
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

  // 页面级组件按 job.id `{#key}` 重挂载，挂载时初始化一次即可（无 Modal 的
  // 开合/切换目标复用问题）。
  let form = $state<FormState>(initialForm());
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let showValidation = $state(false);

  // ──────────────────────────────────────────────────────────────────────
  // Prompt / Agent 候选：加载已启用供应商（含模型）与 Agent 模板列表。
  // providersWithModels 用于把目标里存的 (providerId, modelId) 解析为展示名；
  // agents 用于 agent 目标的模板下拉。读自共享状态，TargetPicker 不直接触状态。
  // ──────────────────────────────────────────────────────────────────────
  let providersWithModels = $state<ProviderWithModels[]>([]);
  let agents = $state<Agent[]>([]);
  let agentsLoading = $state(false);

  onMount(() => {
    if (
      providerState.providersWithModelsNeedRefresh ||
      providerState.providersWithModels.length === 0
    ) {
      providerActions.loadProvidersWithModels().catch((e) => {
        console.error("Failed to load providers for job target:", e);
      });
    }

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

  // providersWithModels 只取已启用供应商 + 其已启用模型，与 chat 模型选择口径一致。
  $effect(() => {
    providersWithModels = providerState.providersWithModels
      .filter((p) => p.enabled)
      .map((p) => ({ ...p, models: p.models.filter((m) => m.enabled) }));
  });

  // ──────────────────────────────────────────────────────────────────────
  // 校验。目标按 kind 分支校验（与 TargetPicker 的高亮提示同源）：
  // - prompt：必须同时选中 provider 与 model（VAL-TARGET-013），且 prompt
  //   文本非空白（VAL-TARGET-012）
  // - agent：必须选中 agent 模板（VAL-TARGET-014）
  // 任一不满足都不落库。
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
    const target = form.target;
    switch (target.kind) {
      case "prompt":
        return (
          target.providerId.length > 0 &&
          target.modelId.length > 0 &&
          target.prompt.trim().length > 0
        );
      case "agent":
        return target.agentId.length > 0 && target.modelId.length > 0;
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

  function backToList() {
    goto("/jobs");
  }

  // ──────────────────────────────────────────────────────────────────────
  // 保存：先校验，再落库（store 自动 upsert 列表）；失败保留表单 + 显示错误
  // （不乐观更新，避免 ghost 卡片）。成功回列表页。
  // ──────────────────────────────────────────────────────────────────────
  async function handleSave(): Promise<void> {
    if (saving || !validate()) return;
    saving = true;
    saveError = null;
    const data = {
      name: form.name.trim(),
      description: form.description.trim() ? form.description : undefined,
      target: $state.snapshot(form.target) as JobTarget,
      cronExpr: form.cronExpr.trim(),
      timezone: form.timezone,
      enabled: form.enabled,
      execTimeoutSecs: parseRobustness(form.execTimeoutSecs),
      maxRetries: parseRobustness(form.maxRetries),
      retryDelaySecs: parseRobustness(form.retryDelaySecs),
    };
    try {
      if (job?.id) {
        await jobStore.update(job.id, data);
      } else {
        await jobStore.create(data);
      }
      backToList();
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
</script>

<!-- Job 编辑二级页：与 Agent 编辑页同构——居中 max-w-3xl 阅读宽度、
     TableGroup 分组卡纵排（不撑满屏幕、不用左右结构）。 -->
<div class="h-full flex flex-col">
  <!-- 顶部工具栏 -->
  <div class="flex-shrink-0 px-6 pb-4 pt-12">
    <div class="mx-auto w-full max-w-3xl">
      <button
        class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit mb-4"
        onclick={backToList}
      >
        <ArrowLeft size={14} />
        {t("jobs.form.backToList")}
      </button>

      <div class="flex items-center gap-3">
        <div class="min-w-0 flex-1">
          <input
            class="modal-title-input w-full"
            bind:value={form.name}
            placeholder={t("jobs.form.namePlaceholder")}
            aria-invalid={nameError != null}
          />
          {#if nameError}
            <span class="text-xs text-error">{nameError}</span>
          {/if}
        </div>
        <Button
          variant="primary"
          size="sm"
          onclick={handleSave}
          disabled={saving}
          customClass="flex items-center gap-2"
        >
          <Save size={14} />
          {saving
            ? t("jobs.form.saving")
            : job
              ? t("jobs.form.save")
              : t("jobs.form.createAction")}
        </Button>
      </div>

      {#if saveError}
        <div
          class="mt-3 rounded-md bg-error/10 px-3 py-2 text-sm text-error"
        >
          {saveError}
        </div>
      {/if}
    </div>
  </div>

  <!-- 表单主体：设置页式分组卡纵排 -->
  <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-6">
    <div class="mx-auto flex w-full max-w-3xl flex-col gap-y-4">
      <!-- 任务目标（要跑什么） -->
      <TableGroup title={t("jobs.form.sectionTarget")}>
        <TableBaseRow>
          <TargetPicker
            bind:target={form.target}
            {providersWithModels}
            {agents}
            {agentsLoading}
            showError={showValidation}
          />
        </TableBaseRow>
      </TableGroup>

      <!-- 调度 -->
      <TableGroup title={t("jobs.form.sectionSchedule")}>
        <TableBaseRow>
          <ScheduleEditor bind:cron={form.cronExpr} />
        </TableBaseRow>
      </TableGroup>

      <!-- 高级：超时 / 重试 -->
      <TableGroup title={t("jobs.form.sectionAdvanced")}>
        <TableBaseRow
          label={t("jobs.form.execTimeout")}
          description={t("jobs.form.execTimeoutHint")}
          error={execTimeoutError ?? undefined}
        >
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.execTimeoutSecs}
            placeholder={t("jobs.form.execTimeoutPlaceholder", {
              n: DEFAULT_EXEC_TIMEOUT_SECS,
            })}
            aria-invalid={execTimeoutError != null}
            class="field w-32 px-2.5 py-1.5 text-sm"
            class:is-error={execTimeoutError != null}
          />
        </TableBaseRow>

        <TableBaseRow
          label={t("jobs.form.maxRetries")}
          description={t("jobs.form.maxRetriesHint")}
          error={maxRetriesError ?? undefined}
        >
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.maxRetries}
            placeholder={t("jobs.form.maxRetriesPlaceholder", {
              n: DEFAULT_MAX_RETRIES,
            })}
            aria-invalid={maxRetriesError != null}
            class="field w-32 px-2.5 py-1.5 text-sm"
            class:is-error={maxRetriesError != null}
          />
        </TableBaseRow>

        <TableBaseRow
          label={t("jobs.form.retryDelay")}
          description={t("jobs.form.retryDelayHint", {
            n: DEFAULT_RETRY_DELAY_SECS,
          })}
          error={retryDelayError ?? undefined}
        >
          <input
            type="number"
            min="0"
            step="1"
            bind:value={form.retryDelaySecs}
            placeholder={t("jobs.form.retryDelayPlaceholder", {
              n: DEFAULT_RETRY_DELAY_SECS,
            })}
            aria-invalid={retryDelayError != null}
            class="field w-32 px-2.5 py-1.5 text-sm"
            class:is-error={retryDelayError != null}
          />
        </TableBaseRow>
      </TableGroup>

      <!-- 应用关闭即不运行的提醒 -->
      <p class="px-1 text-xs text-base-content/45">
        {t("jobs.form.appClosedNotice")}
      </p>
    </div>
  </div>
</div>
