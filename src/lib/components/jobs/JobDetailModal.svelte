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
    Play,
    ExternalLink,
    MessageSquare,
    Bot,
  } from "@lucide/svelte";
  import { goto } from "$app/navigation";
  import Modal from "$lib/components/ui/Modal.svelte";
  import StatusLabel from "$lib/components/ui/StatusLabel.svelte";
  import { cronToHuman } from "$lib/utils/cronReadable";
  import { formatDateTime, formatDuration } from "$lib/utils";
  import { listExecutions, listenJobExecuted, runNow } from "$lib/api/job";
  import { getChat } from "$lib/api/chat";
  import { getAgentSession } from "$lib/api/agentSession";
  import { t } from "$lib/i18n";
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
  // 手动「立即运行」请求进行中（与后端共享的 in-flight 防重入对应：禁用按钮
  // 是第一道防线，后端 CONFLICT 是第二道）。
  let triggering = $state(false);
  let runError = $state<string | null>(null);

  const schedule = $derived(job ? cronToHuman(job.cronExpr) : "");

  // 该任务是否有执行进行中：历史里存在 running 行即视为在跑（历史已包含
  // running 行，无需事件订阅）。运行中禁用「立即运行」以避免重复触发。
  const hasRunningExecution = $derived(
    executions.some((e) => e.status === "running"),
  );
  const runDisabled = $derived(triggering || hasRunningExecution);

  // 执行状态 -> StatusLabel 变体 + 文案。复用现有 StatusLabel 的 4 个语义变体，
  // 不新造 widget：成功→enabled、失败/超时→error、运行中→idle、未知→idle。
  const STATUS_TO_LABEL: Record<
    ExecutionStatus,
    { variant: "enabled" | "disabled" | "idle" | "error"; text: string }
  > = $derived({
    running: { variant: "idle", text: t("jobs.status.running") },
    success: { variant: "enabled", text: t("jobs.status.success") },
    failed: { variant: "error", text: t("jobs.status.failed") },
    timeout: { variant: "error", text: t("jobs.status.timeout") },
  });

  const TRIGGER_TEXT: Record<Trigger, string> = $derived({
    schedule: t("jobs.trigger.schedule"),
    manual: t("jobs.trigger.manual"),
  });

  /**
   * 行耗时文案：
   * - 进行中（status running / ended_at 缺失）显示占位「运行中」，非 0；
   * - 有 duration（毫秒）走 formatDuration，亚秒显示「Nms」不取整为 0；
   * - 终态但缺 duration（异常数据）退回「—」。
   */
  function durationText(exec: JobExecution): string {
    if (exec.status === "running" || exec.endedAt == null)
      return t("jobs.detail.runningDuration");
    if (exec.duration == null) return "—";
    return formatDuration(exec.duration);
  }

  // 目标 kind 决定历史行展开后的跳转目标路由：
  // - prompt → 「跳转到结果」入口，跳到生成的 chat（/chat?id=<chatId>）
  // - agent  → 「跳转到结果」入口，跳到生成的 agent 会话（/agent?id=<sessionId>）
  // 同一 job 的所有执行共享 target.kind；执行行不单独携带 kind，故据此判定。
  const targetKind = $derived(job?.target.kind ?? "prompt");

  // result_ref 指向的会话当前是否可达。lazy 探测：行展开时对其 result_ref
  // 调一次 getChat / getAgentSession，命中错误（如已删除）→ 标记 missing，
  // 跳转入口禁用并提示「结果不可用」（VAL-TARGET-025）。以 execId 为键缓存，
  // 避免重复探测；列表重载（loadHistory）时重置。
  type ResultState = "checking" | "ok" | "missing";
  let resultStates = $state<Record<string, ResultState>>({});

  async function probeResult(exec: JobExecution): Promise<void> {
    const ref = exec.resultRef;
    if (!ref) return;
    if (resultStates[exec.id]) return; // 已探测 / 探测中
    resultStates = { ...resultStates, [exec.id]: "checking" };
    try {
      if (targetKind === "prompt") {
        await getChat(ref);
      } else if (targetKind === "agent") {
        await getAgentSession(ref);
      }
      resultStates = { ...resultStates, [exec.id]: "ok" };
    } catch (e) {
      // 会话已删除 / 不可达：标记缺失，禁用跳转。
      console.error("Result target unreachable:", e);
      resultStates = { ...resultStates, [exec.id]: "missing" };
    }
  }

  // 对已展开的 prompt/agent 行补探测：行可能在 running 态（无 result_ref）被展开，
  // 待 `job_executed` 静默刷新把它翻成终态、补上 result_ref 后，此 effect 让跳转
  // 入口自动从「结果不可用」转为可探测，无需用户重新展开。已探测的 id 由
  // probeResult 内部去重，不会重复请求。
  $effect(() => {
    for (const exec of executions) {
      if (expanded.has(exec.id) && exec.resultRef && !resultStates[exec.id]) {
        void probeResult(exec);
      }
    }
  });

  /**
   * 跳转到该次执行生成的会话：prompt → /chat?id=，agent → /agent?id=。
   * 仅在 result_ref 存在且探测为 ok 时可用；跳转后关闭详情 modal。
   */
  function jumpToResult(exec: JobExecution): void {
    const ref = exec.resultRef;
    if (!ref) return;
    if (resultStates[exec.id] === "missing") return;
    const route =
      targetKind === "agent"
        ? `/agent?id=${encodeURIComponent(ref)}`
        : `/chat?id=${encodeURIComponent(ref)}`;
    onClose();
    void goto(route);
  }

  // 展开 prompt/agent 行后，由下方 `$effect` 对其 result_ref 探测可达性
  // （effect 也覆盖 running→终态后才出现 result_ref 的补探测）。
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
        e instanceof Error ? e.message : t("jobs.detail.historyLoadError");
    } finally {
      loading = false;
    }
  }

  /**
   * 实时静默刷新：`job_executed` 事件抵达时重新拉取历史，但**不**翻转 `loading`
   * ——否则列表会被 spinner 替换，丢失滚动位置与已展开行的 DOM。
   *
   * `list` 命令是事实来源：整体重新赋值后，keyed `#each (exec.id)` 按 id diff，
   * 已存在的行 DOM 被复用（运行中行原地翻转为终态并补耗时，VAL-HISTORY-014），
   * 顺序稳定不重复（019）；`expanded` 以 id 为键，故展开态保留（017）；滚动容器
   * DOM 未重建，滚动位置保留（018）。错过的事件不致错乱——下次事件或重开
   * modal 都会以 list 重新对账（030）。失败仅记日志，保留当前时间线。
   */
  async function refreshHistoryQuietly(jobId: string): Promise<void> {
    try {
      executions = await listExecutions(jobId);
    } catch (e) {
      console.error("Failed to refresh job executions on job_executed:", e);
    }
  }

  /**
   * 手动「立即运行」：调用 `job_run_now`（trigger=manual），完成后重载历史，
   * 时间线顶部即出现新的手动行。运行进行中（triggering 或已有 running 行）按钮
   * 禁用，且 onclick 二次防御直接返回，杜绝并发触发。
   */
  async function handleRunNow(): Promise<void> {
    if (!job?.id || runDisabled) return;
    triggering = true;
    runError = null;
    try {
      await runNow(job.id);
      await loadHistory(job.id);
    } catch (e) {
      console.error("Failed to run job now:", e);
      runError = e instanceof Error ? e.message : t("jobs.detail.runNowFailed");
    } finally {
      triggering = false;
    }
  }

  // 每次打开（或目标 job 切换）重置展开态并重新拉取历史。
  $effect(() => {
    if (open && job?.id) {
      expanded = new Set();
      resultStates = {};
      void loadHistory(job.id);
    } else if (!open) {
      executions = [];
      loadError = null;
      resultStates = {};
    }
  });

  // 打开期间订阅 `job_executed`：仅对当前 job 的事件静默刷新时间线（运行中行
  // 原地翻转为终态、补耗时；展开/滚动保留）。关闭或切换 job 时取消订阅，组件
  // 卸载时由 effect cleanup 兜底——避免泄漏监听器。modal 关闭期间不订阅，错过的
  // 执行在重开时由 `loadHistory`（list 命令，事实来源）补齐终态（VAL-HISTORY-030）。
  $effect(() => {
    if (!open || !job?.id) return;
    const jobId = job.id;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenJobExecuted((payload) => {
      if (payload.jobId === jobId) {
        void refreshHistoryQuietly(jobId);
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((e) => {
        console.error("Failed to subscribe to job_executed in detail:", e);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  });

  // 占位以避免 onMount 未使用 lint；当前无挂载副作用。
  onMount(() => {});
</script>

<Modal {open} title={job?.name ?? t("jobs.detail.title")} showCloseButton {onClose}>
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
            {t("jobs.detail.nextRun")}{!job.enabled
              ? t("jobs.status.disabled")
              : job.nextRunAt == null
                ? "—"
                : formatDateTime(job.nextRunAt)}
          </span>
        </div>
        <div class="flex items-center gap-4 text-xs text-base-content/50 pt-1">
          <span>{t("jobs.detail.runCount", { n: job.runCount })}</span>
          {#if job.failureCount > 0}
            <span class="text-error/70">{t("jobs.detail.failureCount", { n: job.failureCount })}</span>
          {/if}
        </div>
      </div>

      <!-- 顶部操作区：执行历史标题 + 立即运行（trigger=manual）。 -->
      <div
        class="px-6 py-3 border-b border-base-300 flex items-center justify-between"
      >
        <h4
          class="text-sm font-medium text-base-content/80 flex items-center gap-2"
        >
          <History size={15} class="text-base-content/50" />
          {t("jobs.detail.history")}
        </h4>
        <!-- 立即运行：禁用任务也可手动运行（禁用仅停自动调度）；运行进行中
             （triggering 或已有 running 行）按钮禁用，避免重复触发。 -->
        <button
          class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-primary text-primary-content text-xs font-medium cursor-pointer hover:opacity-90 disabled:opacity-50 disabled:cursor-not-allowed"
          disabled={runDisabled}
          onclick={handleRunNow}
        >
          <Play size={13} class="flex-shrink-0" />
          {triggering ? t("jobs.detail.runningNow") : t("jobs.detail.runNow")}
        </button>
      </div>

      {#if runError}
        <div
          class="px-6 py-2 border-b border-base-300 flex items-center gap-2 text-xs text-error"
        >
          <AlertCircle size={13} class="flex-shrink-0" />
          <span>{runError}</span>
        </div>
      {/if}

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
              {t("common.retry")}
            </button>
          </div>
        {:else if executions.length === 0}
          <div
            class="flex flex-col items-center justify-center py-10 text-base-content/50"
          >
            <Clock size={32} class="mb-3 opacity-20" />
            <p class="text-sm">{t("jobs.detail.emptyHistory")}</p>
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

                <!-- 展开区（行级扩展点）：prompt/agent 的「跳转到结果」入口
                     （跳到生成的 chat / agent 会话）。失败时仍可能有 error。 -->
                {#if isOpen}
                  {@const resultState = resultStates[exec.id]}
                  {@const unavailable =
                    exec.resultRef == null || resultState === "missing"}
                  <div class="px-3 pb-3 pt-1 space-y-3 border-t border-base-300">
                      {#if exec.error != null}
                        <div>
                          <p class="text-xs font-medium text-error/80 mb-1">
                            error
                          </p>
                          <pre
                            class="text-xs bg-base-100 text-error rounded-md p-2 max-h-48 overflow-auto whitespace-pre-wrap break-words">{exec.error}</pre>
                        </div>
                      {/if}

                      <div class="pt-1">
                        {#if unavailable}
                          <div
                            class="flex items-center gap-2 rounded-md border border-[var(--hairline)] bg-base-100 px-3 py-2 text-xs text-base-content/50"
                          >
                            <AlertCircle size={14} class="flex-shrink-0" />
                            <span>{t("jobs.detail.resultUnavailable")}</span>
                          </div>
                        {:else}
                          <button
                            type="button"
                            onclick={() => jumpToResult(exec)}
                            disabled={resultState === "checking"}
                            class="inline-flex items-center gap-1.5 rounded-md bg-base-100 px-3 py-2 text-xs font-medium text-primary hover:bg-base-300 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                          >
                            {#if targetKind === "agent"}
                              <Bot size={14} class="flex-shrink-0" />
                            {:else}
                              <MessageSquare size={14} class="flex-shrink-0" />
                            {/if}
                            <span>
                              {resultState === "checking"
                                ? t("jobs.detail.checkingResult")
                                : t("jobs.detail.jumpToResult")}
                            </span>
                            <ExternalLink size={13} class="flex-shrink-0" />
                          </button>
                        {/if}
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
