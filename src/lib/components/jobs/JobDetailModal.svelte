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
  import Spinner from "$lib/components/ui/Spinner.svelte";
  import StatusLabel from "$lib/components/ui/StatusLabel.svelte";
  import { cronToHuman } from "$lib/utils/cronReadable";
  import { formatDateTime, formatDuration } from "$lib/utils";
  import { listExecutions, listenJobExecuted, runNow } from "$lib/api/job";
  import { getAgentSession } from "$lib/api/agentSession";
  import { t } from "$lib/i18n";
  import type { Job, JobExecution, ExecutionStatus, Trigger } from "$lib/types";

  interface Props {
    open: boolean;
    job: Job | null;
    onClose: () => void;
  }

  let { open, job, onClose }: Props = $props();

  let executions = $state<JobExecution[]>([]);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let expanded = $state<Set<string>>(new Set());
  // Manual "run now" in flight. Disabling the button is the first guard;
  // the backend's in-flight CONFLICT check is the second.
  let triggering = $state(false);
  let runError = $state<string | null>(null);

  const schedule = $derived(job ? cronToHuman(job.cronExpr) : "");

  // A running row in history means an execution is in progress (no event
  // subscription needed); "run now" is disabled then to avoid duplicate triggers.
  const hasRunningExecution = $derived(
    executions.some((e) => e.status === "running"),
  );
  const runDisabled = $derived(triggering || hasRunningExecution);

  // Map execution status onto the existing StatusLabel semantic variants.
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
   * Running rows (or missing ended_at) show a "running" placeholder; sub-second
   * durations render as "Nms" rather than rounding to 0; terminal rows missing
   * duration (bad data) fall back to "—".
   */
  function durationText(exec: JobExecution): string {
    if (exec.status === "running" || exec.endedAt == null)
      return t("jobs.detail.runningDuration");
    if (exec.duration == null) return "—";
    return formatDuration(exec.duration);
  }

  // Target kind decides where "jump to result" goes (prompt → /chat?id=,
  // agent → /agent?id=). All executions of a job share target.kind; execution
  // rows don't carry their own kind.
  const targetKind = $derived(job?.target.kind ?? "prompt");

  // Whether the session behind result_ref is still reachable. Probed lazily when
  // a row is expanded; errors (e.g. deleted session) mark it missing so the jump
  // entry is disabled. Cached per execId to avoid re-probing; reset on reload.
  type ResultState = "checking" | "ok" | "missing";
  let resultStates = $state<Record<string, ResultState>>({});

  async function probeResult(exec: JobExecution): Promise<void> {
    const ref = exec.resultRef;
    if (!ref) return;
    if (resultStates[exec.id]) return; // already probed or probing
    resultStates = { ...resultStates, [exec.id]: "checking" };
    try {
      if (targetKind === "prompt") {
        // Chat sessions no longer exist; prompt result refs are unreachable
        resultStates = { ...resultStates, [exec.id]: "missing" };
      } else if (targetKind === "agent") {
        await getAgentSession(ref);
        resultStates = { ...resultStates, [exec.id]: "ok" };
      }
    } catch (e) {
      // Session deleted or unreachable: mark missing to disable the jump entry.
      console.error("Result target unreachable:", e);
      resultStates = { ...resultStates, [exec.id]: "missing" };
    }
  }

  // Re-probe expanded rows: a row may be expanded while running (no result_ref);
  // once the silent refresh flips it to a terminal state with a result_ref, this
  // probes it so the jump entry activates without re-expanding. probeResult
  // dedupes by id, so no duplicate requests.
  $effect(() => {
    for (const exec of executions) {
      if (expanded.has(exec.id) && exec.resultRef && !resultStates[exec.id]) {
        void probeResult(exec);
      }
    }
  });

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

  // Reachability probing for newly expanded rows is handled by the $effect above.
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
   * Silent refresh on `job_executed`: re-fetch without flipping `loading`, or the
   * spinner would replace the list and drop scroll position and expanded-row DOM.
   * The keyed `#each (exec.id)` diffs by id, so running rows flip to terminal
   * state in place and `expanded` survives. Failures only log, keeping the
   * current timeline.
   */
  async function refreshHistoryQuietly(jobId: string): Promise<void> {
    try {
      executions = await listExecutions(jobId);
    } catch (e) {
      console.error("Failed to refresh job executions on job_executed:", e);
    }
  }

  /**
   * Manual run via `job_run_now`; reload history afterwards so the new manual row
   * appears. The in-body runDisabled check guards against concurrent triggers.
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

  // Reset expansion state and reload history on each open (or job switch).
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

  // While open, subscribe to `job_executed` and silently refresh the timeline
  // for this job only. Cleanup on close/job switch/unmount avoids leaked
  // listeners. Events missed while closed are reconciled on reopen by
  // `loadHistory` (the list command is the source of truth).
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

  // Placeholder so the onMount import isn't flagged unused; no mount side effects.
  onMount(() => {});
</script>

<Modal {open} title={job?.name ?? t("jobs.detail.title")} showCloseButton {onClose}>
  {#if job}
    <div class="w-[44rem] max-w-[88vw] flex flex-col max-h-[80vh]">
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

      <div
        class="px-6 py-3 border-b border-base-300 flex items-center justify-between"
      >
        <h4
          class="text-sm font-medium text-base-content/80 flex items-center gap-2"
        >
          <History size={15} class="text-base-content/50" />
          {t("jobs.detail.history")}
        </h4>
        <!-- Disabled jobs can still be run manually (disable only stops auto
             scheduling); button disabled while a run is in flight. -->
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

      <div class="flex-1 min-h-0 overflow-y-auto px-6 py-3">
        {#if loading}
          <div class="flex items-center justify-center py-10">
            <Spinner size={28} />
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

                <!-- Jump-to-result entry; failed runs may still carry an error to show -->
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
