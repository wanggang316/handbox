<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { ArrowLeft } from "@lucide/svelte";
  import JobEditor from "$lib/components/jobs/JobEditor.svelte";
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import { getJob } from "$lib/api/job";
  import { t } from "$lib/i18n";
  import type { Job } from "$lib/types";

  let isLoading = $state(false);
  let errorMessage = $state<string | null>(null);
  let job = $state<Job | null>(null);

  const jobId = $derived($page.params.id);

  async function loadDetail() {
    if (!jobId) {
      errorMessage = t("jobs.form.loadFailed");
      return;
    }
    try {
      isLoading = true;
      errorMessage = null;
      job = await getJob(jobId);
    } catch (error) {
      console.error("Failed to load job detail:", error);
      errorMessage = t("jobs.form.loadFailed");
    } finally {
      isLoading = false;
    }
  }

  onMount(loadDetail);
</script>

{#if isLoading}
  <div class="h-full flex items-center justify-center">
    <Spinner size={28} />
  </div>
{:else if job}
  {#key job.id}
    <JobEditor {job} />
  {/key}
{:else}
  <div class="h-full flex flex-col gap-4 px-6 pt-12">
    <div class="mx-auto w-full max-w-3xl">
      <button
        class="flex items-center gap-2 text-sm text-base-content/70 hover:text-base-content w-fit"
        onclick={() => goto("/jobs")}
      >
        <ArrowLeft size={14} />
        {t("jobs.form.backToList")}
      </button>
      <div class="mt-4 p-3 rounded-lg bg-error/10 text-error text-sm">
        {errorMessage ?? t("jobs.form.loadFailed")}
      </div>
    </div>
  </div>
{/if}
