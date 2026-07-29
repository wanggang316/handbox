<script lang="ts">
  import { AlertCircle, CalendarClock } from "@lucide/svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { previewSchedule } from "$lib/api/job";
  import { AppError } from "$lib/api";
  import { cronToHuman } from "$lib/utils/cronReadable";
  import { t } from "$lib/i18n";

  interface Props {
    /**
     * Controlled output: the standard 5-field cron expression, bound by the
     * parent via `bind:cron`. The saved cron and the previewed cron are always
     * this same value.
     */
    cron?: string;
    /** Change callback (equivalent to `bind:cron`; use either). */
    onChange?: (cron: string) => void;
    previewCount?: number;
  }

  let {
    cron = $bindable("0 9 * * *"),
    onChange = () => {},
    previewCount = 5,
  }: Props = $props();

  type Tab = "quick" | "advanced";
  let activeTab = $state<Tab>("quick");

  const TAB_ITEMS = $derived([
    { value: "quick", label: t("jobs.schedule.tabQuick") },
    { value: "advanced", label: t("jobs.schedule.tabAdvanced") },
  ]);

  // Quick-preset params; each interaction compiles them into the single cron outlet.
  type PresetKind = "minutes" | "hours" | "daily" | "weekly" | "monthly";

  let presetKind = $state<PresetKind>("daily");
  let minuteN = $state(15); // every N minutes
  let hourN = $state(3); // every N hours
  let timeStr = $state("09:00"); // HH:MM for daily / weekly / monthly
  let weekdays = $state<number[]>([1]); // weekly: 0=Sun .. 6=Sat, multi-select
  let monthDay = $state(15); // day of month

  const PRESET_ITEMS = $derived([
    { value: "minutes", label: t("jobs.schedule.presetMinutes") },
    { value: "hours", label: t("jobs.schedule.presetHours") },
    { value: "daily", label: t("jobs.schedule.presetDaily") },
    { value: "weekly", label: t("jobs.schedule.presetWeekly") },
    { value: "monthly", label: t("jobs.schedule.presetMonthly") },
  ]);

  const WEEKDAY_LABELS = $derived([
    t("jobs.schedule.weekday.sun"),
    t("jobs.schedule.weekday.mon"),
    t("jobs.schedule.weekday.tue"),
    t("jobs.schedule.weekday.wed"),
    t("jobs.schedule.weekday.thu"),
    t("jobs.schedule.weekday.fri"),
    t("jobs.schedule.weekday.sat"),
  ]);

  /** Split "HH:MM" into cron minute/hour fields; falls back to 0 0 when invalid. */
  function timeFields(value: string): { minute: number; hour: number } {
    const match = /^(\d{1,2}):(\d{1,2})$/.exec(value.trim());
    if (!match) return { minute: 0, hour: 0 };
    const hour = Math.min(23, Math.max(0, Number(match[1])));
    const minute = Math.min(59, Math.max(0, Number(match[2])));
    return { minute, hour };
  }

  function compileQuick(): string {
    switch (presetKind) {
      case "minutes": {
        const n = Math.min(59, Math.max(1, Math.trunc(minuteN)));
        return `*/${n} * * * *`;
      }
      case "hours": {
        const n = Math.min(23, Math.max(1, Math.trunc(hourN)));
        return `0 */${n} * * *`;
      }
      case "daily": {
        const { minute, hour } = timeFields(timeStr);
        return `${minute} ${hour} * * *`;
      }
      case "weekly": {
        const { minute, hour } = timeFields(timeStr);
        const days = [...new Set(weekdays)].sort((a, b) => a - b);
        const dow = days.length > 0 ? days.join(",") : "1";
        return `${minute} ${hour} * * ${dow}`;
      }
      case "monthly": {
        const { minute, hour } = timeFields(timeStr);
        const d = Math.min(31, Math.max(1, Math.trunc(monthDay)));
        return `${minute} ${hour} ${d} * *`;
      }
    }
  }

  function applyQuick(): void {
    setCron(compileQuick());
  }

  /** Single write path for cron: updates the bindable and fires onChange. */
  function setCron(next: string): void {
    if (next === cron) return;
    cron = next;
    onChange(next);
  }

  function handleTabChange(value: string): void {
    activeTab = value as Tab;
  }

  function handlePresetChange(value: string): void {
    presetKind = value as PresetKind;
    applyQuick();
  }

  function toggleWeekday(day: number): void {
    weekdays = weekdays.includes(day)
      ? weekdays.filter((d) => d !== day)
      : [...weekdays, day];
    applyQuick();
  }

  // Non-preset crons fall back to the raw string (guaranteed by cronToHuman).
  const humanText = $derived(cronToHuman(cron));

  // Debounced preview: on cron change, call job_preview_schedule after 300ms.
  let occurrences = $state<number[]>([]);
  let previewError = $state<string | null>(null);
  let loading = $state(false);

  const DEBOUNCE_MS = 300;

  $effect(() => {
    // Read deps synchronously so the effect subscribes to them.
    const current = cron;
    const count = previewCount;

    const trimmed = current.trim();
    // Empty cron clears the preview with a hint, without hitting the backend.
    if (!trimmed) {
      occurrences = [];
      previewError = t("jobs.schedule.cronRequired");
      loading = false;
      return;
    }

    loading = true;
    let cancelled = false;
    const timer = setTimeout(async () => {
      try {
        const result = await previewSchedule(trimmed, count);
        if (cancelled) return;
        occurrences = result;
        previewError = null;
      } catch (err) {
        if (cancelled) return;
        occurrences = [];
        previewError =
          err instanceof AppError
            ? err.message
            : err instanceof Error
              ? err.message
              : t("jobs.schedule.previewFailed");
      } finally {
        if (!cancelled) loading = false;
      }
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  });

  function formatOccurrence(ms: number): string {
    return new Date(ms).toLocaleString("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  }
</script>

<div class="grid grid-cols-2 gap-5">
  <div class="flex flex-col gap-4">
  <Tabs value={activeTab} items={TAB_ITEMS} onChange={handleTabChange} />

  {#if activeTab === "quick"}
    <div class="flex flex-col gap-3">
      <Select
        label={t("jobs.schedule.frequency")}
        value={presetKind}
        options={PRESET_ITEMS}
        onChange={handlePresetChange}
        class="w-full"
      />

      {#if presetKind === "minutes"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.everyMinutes")}</span>
          <input
            type="number"
            min="1"
            max="59"
            bind:value={minuteN}
            oninput={applyQuick}
            class="field field--soft w-full px-3 py-2 text-sm"
          />
        </label>
      {:else if presetKind === "hours"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.everyHours")}</span>
          <input
            type="number"
            min="1"
            max="23"
            bind:value={hourN}
            oninput={applyQuick}
            class="field field--soft w-full px-3 py-2 text-sm"
          />
        </label>
      {:else if presetKind === "daily"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.time")}</span>
          <input
            type="time"
            bind:value={timeStr}
            oninput={applyQuick}
            class="field field--soft w-full px-3 py-2 text-sm"
          />
        </label>
      {:else if presetKind === "weekly"}
        <div class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.weekdays")}</span>
          <div class="flex flex-wrap gap-1.5">
            {#each WEEKDAY_LABELS as label, day (day)}
              <button
                type="button"
                aria-pressed={weekdays.includes(day)}
                onclick={() => toggleWeekday(day)}
                class="rounded-md border px-2.5 py-1 text-xs transition-colors {weekdays.includes(
                  day,
                )
                  ? 'border-primary bg-primary/15 text-primary'
                  : 'border-[var(--hairline)] bg-base-300 text-base-content/70 hover:text-base-content'}"
              >
                {label}
              </button>
            {/each}
          </div>
        </div>
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.time")}</span>
          <input
            type="time"
            bind:value={timeStr}
            oninput={applyQuick}
            class="field field--soft w-full px-3 py-2 text-sm"
          />
        </label>
      {:else if presetKind === "monthly"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.dayOfMonth")}</span>
          <input
            type="number"
            min="1"
            max="31"
            bind:value={monthDay}
            oninput={applyQuick}
            class="field field--soft w-full px-3 py-2 text-sm"
          />
        </label>
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">{t("jobs.schedule.time")}</span>
          <input
            type="time"
            bind:value={timeStr}
            oninput={applyQuick}
            class="field field--soft w-full px-3 py-2 text-sm"
          />
        </label>
      {/if}
    </div>
  {:else}
    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">{t("jobs.schedule.cronLabel")}</span>
      <input
        type="text"
        value={cron}
        oninput={(e) => setCron((e.currentTarget as HTMLInputElement).value)}
        spellcheck="false"
        autocomplete="off"
        placeholder="0 9 * * *"
        class="field field--soft w-full px-3 py-2 font-mono text-sm"
      />
    </label>
  {/if}

  </div>

  <div
    class="flex flex-col rounded-md border border-[var(--hairline)] bg-base-200 p-3"
  >
    <div
      class="mb-2 flex items-center gap-2 border-b border-[var(--hairline)] pb-2 text-sm text-base-content/70"
    >
      <CalendarClock size={14} class="flex-shrink-0 text-base-content/50" />
      <span class="truncate" title={cron}>{humanText}</span>
    </div>

    <div class="mb-2 text-xs font-medium text-base-content/60">
      {t("jobs.schedule.previewTitle", { n: previewCount })}
    </div>

    {#if previewError}
      <div class="flex items-start gap-2 text-sm text-error">
        <AlertCircle size={14} class="mt-0.5 flex-shrink-0" />
        <span>{previewError}</span>
      </div>
    {:else if loading && occurrences.length === 0}
      <div class="text-sm text-base-content/50">{t("jobs.schedule.calculating")}</div>
    {:else if occurrences.length === 0}
      <div class="text-sm text-base-content/50">{t("jobs.schedule.noOccurrences")}</div>
    {:else}
      <ol class="flex flex-col gap-1.5 text-sm text-base-content/80">
        {#each occurrences as ms (ms)}
          <li class="font-mono tabular-nums">{formatOccurrence(ms)}</li>
        {/each}
      </ol>
    {/if}
  </div>
</div>
