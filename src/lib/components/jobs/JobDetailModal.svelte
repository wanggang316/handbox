<script lang="ts">
  import { onMount } from "svelte";
  import {
    ChevronRight,
    ChevronDown,
    Repeat,
    CalendarClock,
    AlertCircle,
    History,
    Hand,
    Clock,
  } from "@lucide/svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import StatusLabel from "$lib/components/ui/StatusLabel.svelte";
  import { cronToHuman } from "$lib/utils/cronReadable";
  import { formatDateTime, formatDuration } from "$lib/utils";
  import { listExecutions } from "$lib/api/job";
  import type { Job, JobExecution, ExecutionStatus, Trigger } from "$lib/types";

  interface Props {
    open: boolean;
    job: Job | null;
    onClose: () => void;
  }

  let { open, job, onClose }: Props = $props();

  // 执行历史拉取状态。每次打开（job 变化）重新加载。
  let executions = $state<JobExecution[]>([]);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  // 展开行集合：行 id -> 是否展开 stdout/stderr/error。
  let expanded = $state<Set<string>>(new Set());

  const schedule = $derived(job ? cronToHuman(job.cronExpr) : "");

  // 执行状态 -> StatusLabel 变体 + 文案。复用现有 StatusLabel 的 4 个语义变体，
  // 不新造 widget：成功→enabled、失败/超时→error、运行中→idle、未知→idle。
  const STATUS_TO_LABEL: Record<
    ExecutionStatus,
    { variant: "enabled" | "disabled" | "idle" | "error"; text: string }
  > = {
    running: { variant: "idle", text: "运行中" },
    success: { variant: "enabled", text: "成功" },
    failed: { variant: "error", text: "失败" },
    timeout: { variant: "error", text: "超时" },
  };

  const TRIGGER_TEXT: Record<Trigger, string> = {
    schedule: "定时",
    manual: "手动",
  };

  /**
   * 行耗时文案：
   * - 进行中（status running / ended_at 缺失）显示占位「运行中」，非 0；
   * - 有 duration（毫秒）走 formatDuration，亚秒显示「Nms」不取整为 0；
   * - 终态但缺 duration（异常数据）退回「—」。
   */
  function durationText(exec: JobExecution): string {
    if (exec.status === "running" || exec.endedAt == null) return "运行中";
    if (exec.duration == null) return "—";
    return formatDuration(exec.duration);
  }

  function toggleExpand(id: string): void {
    const next = new Set(expanded);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    expanded = next;
  }

  async function loadHistory(jobId: string): Promise<void> {
    loading = true;
    loadError = null;
    try {
      executions = await listExecutions(jobId);
    } catch (e) {
      console.error("Failed to load job executions:", e);
      loadError =
        e instanceof Error ? e.message : "加载执行历史失败，请重试";
    } finally {
      loading = false;
    }
  }

  // 每次打开（或目标 job 切换）重置展开态并重新拉取历史。
  $effect(() => {
    if (open && job?.id) {
      expanded = new Set();
      void loadHistory(job.id);
    } else if (!open) {
      executions = [];
      loadError = null;
    }
  });

  // 占位以避免 onMount 未使用 lint；当前无挂载副作用。
  onMount(() => {});
</script>

<Modal {open} title={job?.name ?? "任务详情"} showCloseButton {onClose}>
  {#if job}
    <div class="w-[44rem] max-w-[88vw] flex flex-col max-h-[80vh]">
      <!-- 任务概览：调度 + 下次运行 + 运行统计 -->
      <div class="px-6 pt-14 pb-4 border-b border-base-300 space-y-2 text-sm">
        {#if job.description}
          <p class="text-base-content/70">{job.description}</p>
        {/if}
        <div class="flex items-center gap-2 text-base-content/70">
          <Repeat size={14} class="flex-shrink-0 text-base-content/50" />
          <span class="truncate" title={job.cronExpr}>{schedule}</span>
        </div>
        <div class="flex items-center gap-2 text-base-content/70">
          <CalendarClock size={14} class="flex-shrink-0 text-base-content/50" />
          <span class="truncate">
            下次运行：{!job.enabled
              ? "已禁用"
              : job.nextRunAt == null
                ? "—"
                : formatDateTime(job.nextRunAt)}
          </span>
        </div>
        <div class="flex items-center gap-4 text-xs text-base-content/50 pt-1">
          <span>运行 {job.runCount} 次</span>
          {#if job.failureCount > 0}
            <span class="text-error/70">失败 {job.failureCount} 次</span>
          {/if}
        </div>
      </div>

      <!-- 顶部操作区：留作 run-now feature 的扩展点（本期不实现立即运行）。 -->
      <div
        class="px-6 py-3 border-b border-base-300 flex items-center justify-between"
      >
        <h4
          class="text-sm font-medium text-base-content/80 flex items-center gap-2"
        >
          <History size={15} class="text-base-content/50" />
          执行历史
        </h4>
        <!-- run-now / refresh 操作按钮将在后续 feature 接入此处 -->
      </div>

      <!-- 历史时间线：最新在上，可滚动，避免撑破 Modal -->
      <div class="flex-1 min-h-0 overflow-y-auto px-6 py-3">
        {#if loading}
          <div class="flex items-center justify-center py-10">
            <div
              class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"
            ></div>
          </div>
        {:else if loadError}
          <div
            class="flex flex-col items-center justify-center py-10 text-base-content/50"
          >
            <AlertCircle size={32} class="mb-3 opacity-40 text-error" />
            <p class="text-sm text-base-content/70 mb-3">{loadError}</p>
            <button
              class="text-primary hover:underline cursor-pointer text-sm"
              onclick={() => job?.id && loadHistory(job.id)}
            >
              重试
            </button>
          </div>
        {:else if executions.length === 0}
          <div
            class="flex flex-col items-center justify-center py-10 text-base-content/50"
          >
            <Clock size={32} class="mb-3 opacity-20" />
            <p class="text-sm">无执行记录</p>
          </div>
        {:else}
          <ul class="space-y-2">
            {#each executions as exec (exec.id)}
              {@const isOpen = expanded.has(exec.id)}
              {@const labelMeta = STATUS_TO_LABEL[exec.status]}
              <li class="bg-base-200 rounded-lg overflow-hidden">
                <!-- 行头：点击展开/收起 stdout/stderr/error -->
                <button
                  class="w-full flex items-center gap-3 px-3 py-2.5 text-left hover:bg-base-300 transition-colors"
                  onclick={() => toggleExpand(exec.id)}
                  aria-expanded={isOpen}
                >
                  {#if isOpen}
                    <ChevronDown
                      size={16}
                      class="flex-shrink-0 text-base-content/50"
                    />
                  {:else}
                    <ChevronRight
                      size={16}
                      class="flex-shrink-0 text-base-content/50"
                    />
                  {/if}
                  <StatusLabel status={labelMeta.variant} text={labelMeta.text} />
                  <span
                    class="flex items-center gap-1 text-xs text-base-content/60 flex-shrink-0"
                  >
                    {#if exec.trigger === "manual"}
                      <Hand size={12} class="text-base-content/40" />
                    {:else}
                      <Repeat size={12} class="text-base-content/40" />
                    {/if}
                    {TRIGGER_TEXT[exec.trigger]}
                  </span>
                  <span class="text-xs text-base-content/60 truncate">
                    {formatDateTime(exec.startedAt, { second: "2-digit" })}
                  </span>
                  <span
                    class="ml-auto text-xs text-base-content/50 flex-shrink-0 tabular-nums"
                  >
                    {durationText(exec)}
                  </span>
                </button>

                <!-- 展开区（行级扩展点）：stdout / stderr / error 各自分块。
                     本期仅渲染 artifact 的输出字段；prompt/agent 的结果跳转是 M3。 -->
                {#if isOpen}
                  <div class="px-3 pb-3 pt-1 space-y-3 border-t border-base-300">
                    {#if exec.error != null}
                      <div>
                        <p
                          class="text-xs font-medium text-error/80 mb-1"
                        >
                          error
                        </p>
                        <pre
                          class="text-xs bg-base-100 text-error rounded-md p-2 max-h-48 overflow-auto whitespace-pre-wrap break-words">{exec.error}</pre>
                      </div>
                    {/if}

                    <div>
                      <p class="text-xs font-medium text-base-content/60 mb-1">
                        stdout
                      </p>
                      <pre
                        class="text-xs bg-base-100 text-base-content/80 rounded-md p-2 max-h-64 overflow-auto whitespace-pre-wrap break-words">{exec.stdout ?? ""}</pre>
                    </div>

                    <div>
                      <p class="text-xs font-medium text-base-content/60 mb-1">
                        stderr
                      </p>
                      <pre
                        class="text-xs bg-base-100 text-base-content/80 rounded-md p-2 max-h-64 overflow-auto whitespace-pre-wrap break-words">{exec.stderr ?? ""}</pre>
                    </div>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  {/if}
</Modal>
