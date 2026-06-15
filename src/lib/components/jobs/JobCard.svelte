<script lang="ts">
  import { Pencil, Trash2, Clock, Repeat, CalendarClock } from "@lucide/svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import { cronToHuman } from "$lib/utils/cronReadable";
  import type { Job, ExecutionStatus } from "$lib/types";

  interface Props {
    job: Job;
    /**
     * 启用/禁用前置回调：返回 true 提交切换、false 回滚开关视觉。
     * 由父级桥接到 jobStore.setEnabled，写失败时返回 false 触发回滚。
     */
    onToggleEnabled: (next: boolean) => boolean | Promise<boolean>;
    onEdit: (job: Job) => void;
    onDelete: (job: Job) => void;
  }

  let { job, onToggleEnabled, onEdit, onDelete }: Props = $props();

  // 目标类型 -> 展示标签 + 语义配色 chip 类（artifact→primary / agent→info / prompt→success）。
  // 类名写成完整字面量，确保 Tailwind 4 JIT 能静态扫描到（动态拼接 `bg-{x}` 会被 purge）。
  const TARGET_META: Record<Job["target"]["kind"], { label: string; chip: string }> = {
    artifact: { label: "Artifact", chip: "bg-primary/20 text-primary" },
    agent: { label: "Agent", chip: "bg-info/20 text-info" },
    prompt: { label: "Prompt", chip: "bg-success/20 text-success" },
  };

  const targetMeta = $derived(TARGET_META[job.target.kind]);

  const schedule = $derived(cronToHuman(job.cronExpr));

  // 下次运行：禁用任务显示「已禁用」语义而非误导性时间
  const nextRunText = $derived.by(() => {
    if (!job.enabled) return "已禁用";
    if (job.nextRunAt == null) return "—";
    return new Date(job.nextRunAt).toLocaleString("zh-CN");
  });

  // 上次状态：未运行（无 lastStatus）显示「从未运行」。chip 类同样写成完整字面量。
  const STATUS_META: Record<ExecutionStatus, { label: string; chip: string }> = {
    running: { label: "运行中", chip: "bg-info/20 text-info" },
    success: { label: "成功", chip: "bg-success/20 text-success" },
    failed: { label: "失败", chip: "bg-error/20 text-error" },
    timeout: { label: "超时", chip: "bg-warning/20 text-warning" },
  };

  const lastStatusMeta = $derived(job.lastStatus ? STATUS_META[job.lastStatus] : null);
</script>

<div class="bg-base-200 rounded-lg p-4 hover:bg-base-300 transition-colors flex flex-col">
  <div class="flex items-start justify-between mb-3 gap-2">
    <div class="min-w-0">
      <h3 class="font-medium text-base-content truncate">{job.name}</h3>
      <span class="mt-1 inline-block px-2 py-0.5 text-xs rounded-full {targetMeta.chip}">
        {targetMeta.label}
      </span>
    </div>
    <div class="flex items-center gap-1 flex-shrink-0">
      <Toggle
        checked={job.enabled}
        onChangeBefore={(next) => onToggleEnabled(next)}
        id={`job-toggle-${job.id ?? "new"}`}
      />
      <button
        class="p-1.5 rounded-lg hover:bg-base-100 text-base-content/60 hover:text-base-content transition-colors"
        onclick={() => onEdit(job)}
        title="编辑"
      >
        <Pencil size={14} />
      </button>
      <button
        class="p-1.5 rounded-lg hover:bg-error/10 text-base-content/60 hover:text-error transition-colors"
        onclick={() => onDelete(job)}
        title="删除"
      >
        <Trash2 size={14} />
      </button>
    </div>
  </div>

  <div class="space-y-2 text-sm">
    <div class="flex items-center gap-2 text-base-content/70">
      <Repeat size={14} class="flex-shrink-0 text-base-content/50" />
      <span class="truncate" title={job.cronExpr}>{schedule}</span>
    </div>
    <div class="flex items-center gap-2 text-base-content/70">
      <CalendarClock size={14} class="flex-shrink-0 text-base-content/50" />
      <span class="truncate">
        下次运行：<span class:text-base-content={!job.enabled}>{nextRunText}</span>
      </span>
    </div>
    <div class="flex items-center gap-2 text-base-content/70">
      <Clock size={14} class="flex-shrink-0 text-base-content/50" />
      <span>上次状态：</span>
      {#if lastStatusMeta}
        <span class="px-2 py-0.5 text-xs rounded-full {lastStatusMeta.chip}">
          {lastStatusMeta.label}
        </span>
      {:else}
        <span class="text-base-content/50">从未运行</span>
      {/if}
    </div>
  </div>

  <div
    class="mt-3 pt-3 border-t border-base-300 text-xs text-base-content/50 flex items-center justify-between"
  >
    <span>运行次数：{job.runCount}</span>
    {#if job.failureCount > 0}
      <span class="text-error/70">失败 {job.failureCount} 次</span>
    {/if}
  </div>
</div>
