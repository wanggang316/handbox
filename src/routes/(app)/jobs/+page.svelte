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
  import JobDetailModal from "$lib/components/jobs/JobDetailModal.svelte";
  import { goto } from "$app/navigation";
  import { t } from "$lib/i18n";
  import type { Job } from "$lib/types";

  let searchQuery = $state("");

  let showDeleteConfirm = $state(false);
  let deletingJob = $state<Job | null>(null);
  let deleting = $state(false);
  let deleteError = $state<string | null>(null);

  let showDetailModal = $state(false);
  let detailJob = $state<Job | null>(null);

  const filteredJobs = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return jobStore.jobs;
    return jobStore.jobs.filter((j) => j.name.toLowerCase().includes(query));
  });

  /**
   * Toggle bridge: write the backend on switch. Return true to commit the
   * visual state, false so the Toggle rolls back (onChangeBefore semantics).
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
    goto("/jobs/new");
  }

  function handleEdit(job: Job) {
    if (!job.id) return;
    goto(`/jobs/${job.id}`);
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

  // ConfirmModal renders {@html message}; escape backend error text before injecting
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

  async function confirmDelete(): Promise<void> {
    if (!deletingJob?.id) return;
    deleting = true;
    deleteError = null;
    try {
      await jobStore.delete(deletingJob.id);
      closeDeleteConfirm();
    } catch (e) {
      // Delete failed: keep the row (store untouched) and show the error inside
      // the confirm dialog without closing it.
      console.error("Failed to delete job:", e);
      deleteError = jobStore.error ?? t("jobs.delete.failed");
    } finally {
      deleting = false;
    }
  }

  onMount(() => {
    // On job_list failure the store records the error so the template shows it
    // instead of an endless spinner.
    jobStore.load().catch((e) => {
      console.error("Failed to load jobs:", e);
    });

    // Subscribe to execution events: refresh the affected card's last status /
    // run count in place. `refresh` pulls the latest via job_get and upserts by
    // id — existing rows are replaced in place, order stays stable. Missed
    // events self-heal: the next event or reopening the modal reconciles.
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

    return () => {
      unlisten?.();
    };
  });
</script>

<div class="h-full flex flex-col bg-[var(--bg-canvas)]">
  <!-- Header scrolls with the content -->
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
    <!-- Spinner replaces content only on cold start (empty list); with cache,
         render immediately and refresh in background to avoid a spinner flash. -->
    {#if jobStore.isLoading && jobStore.jobs.length === 0}
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

<JobDetailModal
  open={showDetailModal}
  job={detailJob}
  onClose={closeDetailModal}
/>

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
