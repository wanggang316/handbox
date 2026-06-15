<script lang="ts">
  import { onMount } from "svelte";
  import { Plus, Clock, Search, AlertCircle } from "@lucide/svelte";
  import { jobStore } from "$lib/stores/jobStore.svelte";
  import JobCard from "$lib/components/jobs/JobCard.svelte";
  import Button from "$lib/components/ui/Button.svelte";
  import type { Job } from "$lib/types";

  let searchQuery = $state("");

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

  // 新建/编辑/删除 Modal 由 job-form-modal feature 接入；此处仅占位。
  function handleCreate() {
    // TODO(job-form-modal): 打开 JobFormModal（创建模式）
  }

  function handleEdit(_job: Job) {
    // TODO(job-form-modal): 打开 JobFormModal（编辑模式，预填 _job）
  }

  function handleDelete(_job: Job) {
    // TODO(job-form-modal): 打开删除确认 Modal
  }

  onMount(() => {
    // job_list 失败时 store 会记录 error，模板渲染可见错误而非无限 spinner。
    jobStore.load().catch((e) => {
      console.error("Failed to load jobs:", e);
    });
  });
</script>

<div class="h-full flex flex-col">
  <div class="flex-shrink-0 p-4 border-b border-base-300 mt-12">
    <div class="flex items-center justify-between mb-4">
      <div class="flex items-center gap-4">
        <h1 class="text-xl font-semibold text-base-content flex items-center gap-2">
          <Clock size={24} />
          任务
        </h1>
        <span class="text-sm text-base-content/60">
          {filteredJobs.length} 个
        </span>
      </div>
      <Button
        variant="primary"
        size="sm"
        on:click={handleCreate}
        customClass="flex items-center gap-2"
      >
        <Plus size={16} />
        新建任务
      </Button>
    </div>

    <div class="relative">
      <input
        type="text"
        placeholder="搜索任务名称..."
        class="w-full h-9 pl-10 pr-4 bg-base-200 rounded-lg text-base-content placeholder:text-base-content/50 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm"
        bind:value={searchQuery}
      />
      <Search
        class="absolute left-3 top-1/2 -translate-y-1/2 text-base-content/50"
        size={16}
      />
    </div>
  </div>

  <div class="flex-1 min-h-0 overflow-y-auto p-4">
    {#if jobStore.isLoading}
      <div class="flex items-center justify-center h-full">
        <div
          class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"
        ></div>
      </div>
    {:else if jobStore.error}
      <div class="flex flex-col items-center justify-center h-full text-base-content/50">
        <AlertCircle size={48} class="mb-4 opacity-30 text-error" />
        <p class="mb-2 text-base-content/70">加载任务失败</p>
        <p class="text-sm mb-4 text-base-content/50">{jobStore.error}</p>
        <button
          class="text-primary hover:underline cursor-pointer"
          onclick={() => jobStore.load().catch(() => {})}
        >
          重试
        </button>
      </div>
    {:else if filteredJobs.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-base-content/50">
        <Clock size={48} class="mb-4 opacity-20" />
        {#if searchQuery.trim()}
          <p class="mb-2">没有找到匹配的任务</p>
          <button
            class="text-primary hover:underline cursor-pointer"
            onclick={() => (searchQuery = "")}
          >
            清除搜索
          </button>
        {:else}
          <p>还没有创建任何任务</p>
          <p class="text-sm mt-2">点击上方按钮创建您的第一个定时任务</p>
        {/if}
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {#each filteredJobs as job (job.id)}
          <JobCard
            {job}
            onToggleEnabled={(next) => handleToggleEnabled(job, next)}
            onEdit={handleEdit}
            onDelete={handleDelete}
          />
        {/each}
      </div>
    {/if}
  </div>
</div>
