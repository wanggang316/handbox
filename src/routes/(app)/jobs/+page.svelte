<script lang="ts">
  import { onMount } from "svelte";
  import { Plus, Clock, Search, AlertCircle } from "@lucide/svelte";
  import PageHeader from "$lib/components/ui/PageHeader.svelte";
  import { listenJobExecuted } from "$lib/api/job";
  import { jobStore } from "$lib/stores/jobStore.svelte";
  import JobCard from "$lib/components/jobs/JobCard.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import ConfirmModal from "$lib/components/ui/ConfirmModal.svelte";
  import JobFormModal from "$lib/components/jobs/JobFormModal.svelte";
  import JobDetailModal from "$lib/components/jobs/JobDetailModal.svelte";
  import type { JobFormData } from "$lib/components/jobs/JobFormModal.svelte";
  import { t } from "$lib/i18n";
  import type { Job } from "$lib/types";

  let searchQuery = $state("");

  // Modal 状态：创建/编辑共用一个 JobFormModal（job 为 null → 创建）。
  let showFormModal = $state(false);
  let editingJob = $state<Job | null>(null);
  let showDeleteConfirm = $state(false);
  let deletingJob = $state<Job | null>(null);
  let deleting = $state(false);
  let deleteError = $state<string | null>(null);

  // 详情 Modal 状态：点卡片主体打开，展示执行历史时间线。
  let showDetailModal = $state(false);
  let detailJob = $state<Job | null>(null);

  // 搜索按名称、大小写不敏感
  const filteredJobs = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return jobStore.jobs;
    return jobStore.jobs.filter((j) => j.name.toLowerCase().includes(query));
  });

  /**
   * 启停桥接：拨动开关时尝试写后端。成功返回 true 落定开关视觉，
   * 失败返回 false 让 Toggle 回滚到原状态（onChangeBefore 语义）。
   */
  async function handleToggleEnabled(job: Job, next: boolean): Promise<boolean> {
    if (!job.id) return false;
    try {
      await jobStore.setEnabled(job.id, next);
      return true;
    } catch (e) {
      console.error("Failed to toggle job enabled:", e);
      return false;
    }
  }

  function handleCreate() {
    editingJob = null;
    showFormModal = true;
  }

  function handleEdit(job: Job) {
    editingJob = job;
    showFormModal = true;
  }

  function handleDelete(job: Job) {
    deletingJob = job;
    deleteError = null;
    showDeleteConfirm = true;
  }

  function handleView(job: Job) {
    detailJob = job;
    showDetailModal = true;
  }

  function closeDetailModal() {
    showDetailModal = false;
    detailJob = null;
  }

  function closeDeleteConfirm() {
    showDeleteConfirm = false;
    deletingJob = null;
    deleteError = null;
  }

  // ConfirmModal 以 {@html message} 渲染；后端错误文案虽非用户输入，仍转义后再注入。
  function escapeHtml(text: string): string {
    return text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  const deleteMessage = $derived(
    deleteError
      ? `<span class="text-error">${escapeHtml(deleteError)}</span>`
      : t("jobs.delete.confirmMessage"),
  );

  function closeFormModal() {
    showFormModal = false;
    editingJob = null;
  }

  /**
   * 保存桥接：落库成功后 store 自动 upsert 列表。失败时 throw 给 JobFormModal，
   * 由其捕获并展示错误且保持表单打开——不在落库前乐观更新，避免 ghost 卡片。
   */
  async function handleSave(data: JobFormData): Promise<void> {
    if (editingJob?.id) {
      await jobStore.update(editingJob.id, {
        name: data.name,
        description: data.description,
        target: data.target,
        cronExpr: data.cronExpr,
        timezone: data.timezone,
        enabled: data.enabled,
        execTimeoutSecs: data.execTimeoutSecs,
        maxRetries: data.maxRetries,
        retryDelaySecs: data.retryDelaySecs,
      });
    } else {
      await jobStore.create({
        name: data.name,
        description: data.description,
        target: data.target,
        cronExpr: data.cronExpr,
        timezone: data.timezone,
        enabled: data.enabled,
        execTimeoutSecs: data.execTimeoutSecs,
        maxRetries: data.maxRetries,
        retryDelaySecs: data.retryDelaySecs,
      });
    }
  }

  async function confirmDelete(): Promise<void> {
    if (!deletingJob?.id) return;
    deleting = true;
    deleteError = null;
    try {
      await jobStore.delete(deletingJob.id);
      closeDeleteConfirm();
    } catch (e) {
      // 删除失败：行保留（store 不移除），错误就地展示在确认框内，不关闭。
      console.error("Failed to delete job:", e);
      deleteError = jobStore.error ?? t("jobs.delete.failed");
    } finally {
      deleting = false;
    }
  }

  onMount(() => {
    // job_list 失败时 store 会记录 error，模板渲染可见错误而非无限 spinner。
    jobStore.load().catch((e) => {
      console.error("Failed to load jobs:", e);
    });

    // 订阅执行事件：某任务执行开始/完成时，原地刷新对应卡片的上次状态/运行次数
    // （VAL-HISTORY-015/016）。`refresh` 经 `job_get`（事实来源）拉最新值并按 id
    // upsert——已在列表中的任务原地替换，顺序稳定不重排（VAL-HISTORY-019）；
    // 错过的事件不致错乱，因为下次事件或重开 modal 都会重新对账（030）。
    let unlisten: (() => void) | undefined;
    listenJobExecuted(({ jobId }) => {
      jobStore.refresh(jobId).catch((e) => {
        console.error("Failed to refresh job card on job_executed:", e);
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((e) => {
        console.error("Failed to subscribe to job_executed:", e);
      });

    // 组件卸载时取消订阅，避免离开 /jobs 后泄漏监听器。
    return () => {
      unlisten?.();
    };
  });
</script>

<div class="h-full flex flex-col bg-[var(--bg-canvas)]">
  <!-- 页头随内容滚动（Codex 式） -->
  <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-6 pt-12">
    <div class="mx-auto w-full max-w-3xl">
    <div class="pb-5 pt-2">
    <PageHeader
      title={t("jobs.title")}
      meta={t("jobs.count", { n: filteredJobs.length })}
    >
      {#snippet actions()}
        <Button
          variant="primary"
          size="sm"
          onclick={handleCreate}
          customClass="flex items-center gap-2"
        >
          <Plus size={16} />
          {t("jobs.create")}
        </Button>
      {/snippet}

      <div class="relative">
        <input
          type="text"
          placeholder={t("jobs.searchPlaceholder")}
          class="field h-10 w-full pl-10 pr-4 text-sm"
          bind:value={searchQuery}
        />
        <Search
          class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/50"
          size={16}
        />
      </div>
    </PageHeader>
    </div>
    {#if jobStore.isLoading}
      <div class="flex items-center justify-center h-full">
        <Spinner size={28} />
      </div>
    {:else if jobStore.error}
      <div class="flex flex-col items-center justify-center h-full text-base-content/50">
        <AlertCircle size={48} class="mb-4 opacity-30 text-error" />
        <p class="mb-2 text-base-content/70">{t("jobs.loadError")}</p>
        <p class="text-sm mb-4 text-base-content/50">{jobStore.error}</p>
        <button
          class="text-primary hover:underline cursor-pointer"
          onclick={() => jobStore.load().catch(() => {})}
        >
          {t("common.retry")}
        </button>
      </div>
    {:else if filteredJobs.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-base-content/50">
        <Clock size={48} class="mb-4 opacity-20" />
        {#if searchQuery.trim()}
          <p class="mb-2">{t("jobs.empty.noMatch")}</p>
          <button
            class="text-primary hover:underline cursor-pointer"
            onclick={() => (searchQuery = "")}
          >
            {t("jobs.empty.clearSearch")}
          </button>
        {:else}
          <p>{t("jobs.empty.none")}</p>
          <p class="text-sm mt-2">{t("jobs.empty.hint")}</p>
        {/if}
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        {#each filteredJobs as job (job.id)}
          <JobCard
            {job}
            onToggleEnabled={(next) => handleToggleEnabled(job, next)}
            onEdit={handleEdit}
            onDelete={handleDelete}
            onView={handleView}
          />
        {/each}
      </div>
    {/if}
    </div>
  </div>
</div>

<!-- 任务表单 Modal（创建/编辑共用） -->
<JobFormModal
  open={showFormModal}
  job={editingJob}
  onClose={closeFormModal}
  onSave={handleSave}
/>

<!-- 任务详情 Modal（执行历史时间线） -->
<JobDetailModal
  open={showDetailModal}
  job={detailJob}
  onClose={closeDetailModal}
/>

<!-- 删除确认模态框 -->
<ConfirmModal
  title={t("jobs.delete.confirmTitle")}
  message={deleteMessage}
  confirmText={t("common.delete")}
  cancelText={t("common.cancel")}
  confirmButtonStyle="danger"
  isLoading={deleting}
  autoCloseOnConfirm={false}
  open={showDeleteConfirm}
  onClose={closeDeleteConfirm}
  onCancel={closeDeleteConfirm}
  onConfirm={confirmDelete}
/>
