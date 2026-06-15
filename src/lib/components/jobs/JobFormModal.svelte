<script lang="ts">
  import { AlertCircle, Info } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Textarea from "$lib/components/ui/Textarea.svelte";
  import TableGroup from "$lib/components/ui/table/TableGroup.svelte";
  import ScheduleEditor from "$lib/components/jobs/ScheduleEditor.svelte";
  import TargetPicker from "$lib/components/jobs/TargetPicker.svelte";
  import { artifactState } from "$lib/states/artifact.svelte";
  import { AppError } from "$lib/api";
  import type { Artifact, Job, JobTarget } from "$lib/types";

  /** 父组件保存所需的表单出参（与 JobCreateInput / JobUpdateInput 对齐的子集）。 */
  export interface JobFormData {
    name: string;
    description?: string;
    target: JobTarget;
    cronExpr: string;
    timezone: string;
    enabled: boolean;
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
  }

  function blankForm(): FormState {
    return {
      name: "",
      description: "",
      cronExpr: DEFAULT_CRON,
      timezone: localTimezone,
      enabled: true,
      target: emptyTarget(),
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
  // 校验
  // ──────────────────────────────────────────────────────────────────────
  const nameError = $derived(
    showValidation && form.name.trim().length === 0
      ? "请输入任务名称"
      : null,
  );

  const targetValid = $derived(
    form.target.kind === "artifact" && form.target.artifactId.length > 0,
  );

  function validate(): boolean {
    showValidation = true;
    if (form.name.trim().length === 0) return false;
    if (!targetValid) return false;
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
      });
      // 成功：触发关闭动画。
      localOpen = false;
    } catch (e) {
      saveError =
        e instanceof AppError
          ? e.message
          : e instanceof Error
            ? e.message
            : "保存失败，请重试";
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
  title={job ? "编辑任务" : "新建任务"}
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
      <span>定时任务仅在应用运行时触发；应用关闭期间不会执行。</span>
    </div>

    <!-- 基本信息 -->
    <TableGroup title="基本信息">
      <div class="flex flex-col gap-3 px-6 py-4">
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">名称</span>
          <input
            type="text"
            bind:value={form.name}
            placeholder="输入任务名称"
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
          <span class="font-medium text-base-content/80">描述（可选）</span>
          <Textarea bind:value={form.description} placeholder="输入任务描述…" rows={3} />
        </label>
      </div>
    </TableGroup>

    <!-- 调度 -->
    <TableGroup title="调度">
      <div class="px-6 py-4">
        <ScheduleEditor bind:cron={form.cronExpr} />
      </div>
    </TableGroup>

    <!-- 目标 -->
    <TableGroup title="目标">
      <div class="px-6 py-4">
        <TargetPicker
          bind:target={form.target}
          {artifacts}
          loading={artifactsLoading}
          showError={showValidation}
        />
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
      <Button variant="ghost" on:click={handleClose} disabled={saving}>
        取消
      </Button>
      <Button variant="primary" on:click={handleSave} disabled={saving}>
        {saving ? "保存中…" : job ? "保存" : "创建"}
      </Button>
    </div>
  </div>
</Modal>
