<script lang="ts">
  import { AlertCircle, CalendarClock } from "@lucide/svelte";
  import Tabs from "$lib/components/ui/Tabs.svelte";
  import Select from "$lib/components/ui/Select.svelte";
  import { previewSchedule } from "$lib/api/job";
  import { AppError } from "$lib/api";
  import { cronToHuman } from "$lib/utils/cronReadable";

  interface Props {
    /**
     * 受控出口：编辑器对外输出的标准 5 段 cron 表达式。父组件（job-form-modal）
     * 用 `bind:cron` 双向绑定；保存所用 cron 即此值，与预览所用 cron 一致。
     */
    cron?: string;
    /** cron 变更回调（与 `bind:cron` 等价，二选一即可）。 */
    onChange?: (cron: string) => void;
    /** 预览返回的未来执行点条数，默认 5。 */
    previewCount?: number;
  }

  let {
    cron = $bindable("0 9 * * *"),
    onChange = () => {},
    previewCount = 5,
  }: Props = $props();

  // ──────────────────────────────────────────────────────────────────────
  // Tab 状态
  // ──────────────────────────────────────────────────────────────────────
  type Tab = "quick" | "advanced";
  let activeTab = $state<Tab>("quick");

  const TAB_ITEMS = [
    { value: "quick", label: "快捷" },
    { value: "advanced", label: "高级 Cron" },
  ];

  // ──────────────────────────────────────────────────────────────────────
  // 快捷预设参数（仅在快捷面板内编辑，交互时编译写回单一 cron 出口）
  // ──────────────────────────────────────────────────────────────────────
  type PresetKind = "minutes" | "hours" | "daily" | "weekly" | "monthly";

  let presetKind = $state<PresetKind>("daily");
  let minuteN = $state(15); // 每 N 分钟
  let hourN = $state(3); // 每 N 小时
  let timeStr = $state("09:00"); // 每天 / 每周 / 每月 的 HH:MM
  let weekdays = $state<number[]>([1]); // 每周：0=周日 .. 6=周六，可多选
  let monthDay = $state(15); // 每月第几日

  const PRESET_ITEMS = [
    { value: "minutes", label: "每 N 分钟" },
    { value: "hours", label: "每 N 小时" },
    { value: "daily", label: "每天" },
    { value: "weekly", label: "每周" },
    { value: "monthly", label: "每月" },
  ];

  const WEEKDAY_LABELS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

  /** 把 `HH:MM` 拆成 cron 的 minute / hour 字段；非法时回退 0 0。 */
  function timeFields(value: string): { minute: number; hour: number } {
    const match = /^(\d{1,2}):(\d{1,2})$/.exec(value.trim());
    if (!match) return { minute: 0, hour: 0 };
    const hour = Math.min(23, Math.max(0, Number(match[1])));
    const minute = Math.min(59, Math.max(0, Number(match[2])));
    return { minute, hour };
  }

  /** 从当前快捷参数编译标准 5 段 cron。 */
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

  /** 用户在快捷面板交互后：编译 cron 并通过单一出口写回。 */
  function applyQuick(): void {
    setCron(compileQuick());
  }

  /** 单一 cron 出口：写回 bindable 并触发 onChange。 */
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

  // ──────────────────────────────────────────────────────────────────────
  // 可读化文案（非预设回退原始字符串，由 cronToHuman 保证）
  // ──────────────────────────────────────────────────────────────────────
  const humanText = $derived(cronToHuman(cron));

  // ──────────────────────────────────────────────────────────────────────
  // 防抖预览：cron 变化 → 300ms 后调 job_preview_schedule
  // ──────────────────────────────────────────────────────────────────────
  let occurrences = $state<number[]>([]);
  let previewError = $state<string | null>(null);
  let loading = $state(false);

  const DEBOUNCE_MS = 300;

  $effect(() => {
    // 显式订阅依赖：cron 与条数。
    const current = cron;
    const count = previewCount;

    const trimmed = current.trim();
    // 空 cron 直接清空预览并提示，不打后端。
    if (!trimmed) {
      occurrences = [];
      previewError = "请填写 cron 表达式";
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
              : "预览失败";
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

<div class="flex flex-col gap-4">
  <Tabs value={activeTab} items={TAB_ITEMS} onChange={handleTabChange} />

  {#if activeTab === "quick"}
    <div class="flex flex-col gap-3">
      <Select
        label="频率"
        value={presetKind}
        options={PRESET_ITEMS}
        onChange={handlePresetChange}
        class="w-full"
      />

      {#if presetKind === "minutes"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">每隔多少分钟</span>
          <input
            type="number"
            min="1"
            max="59"
            bind:value={minuteN}
            oninput={applyQuick}
            class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </label>
      {:else if presetKind === "hours"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">每隔多少小时</span>
          <input
            type="number"
            min="1"
            max="23"
            bind:value={hourN}
            oninput={applyQuick}
            class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </label>
      {:else if presetKind === "daily"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">时间</span>
          <input
            type="time"
            bind:value={timeStr}
            oninput={applyQuick}
            class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </label>
      {:else if presetKind === "weekly"}
        <div class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">星期（可多选）</span>
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
          <span class="font-medium text-base-content/80">时间</span>
          <input
            type="time"
            bind:value={timeStr}
            oninput={applyQuick}
            class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </label>
      {:else if presetKind === "monthly"}
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">每月第几日</span>
          <input
            type="number"
            min="1"
            max="31"
            bind:value={monthDay}
            oninput={applyQuick}
            class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </label>
        <label class="flex flex-col gap-1 text-sm">
          <span class="font-medium text-base-content/80">时间</span>
          <input
            type="time"
            bind:value={timeStr}
            oninput={applyQuick}
            class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </label>
      {/if}
    </div>
  {:else}
    <label class="flex flex-col gap-1 text-sm">
      <span class="font-medium text-base-content/80">Cron 表达式（标准 5 段：分 时 日 月 周）</span>
      <input
        type="text"
        value={cron}
        oninput={(e) => setCron((e.currentTarget as HTMLInputElement).value)}
        spellcheck="false"
        autocomplete="off"
        placeholder="0 9 * * *"
        class="w-full rounded-md border border-[var(--hairline)] bg-base-300 px-3 py-2 font-mono text-sm text-base-content focus:outline-none focus:ring-2 focus:ring-primary"
      />
    </label>
  {/if}

  <!-- 可读化文案 -->
  <div class="flex items-center gap-2 text-sm text-base-content/70">
    <CalendarClock size={14} class="flex-shrink-0 text-base-content/50" />
    <span class="truncate" title={cron}>{humanText}</span>
  </div>

  <!-- 预览区 -->
  <div class="rounded-md border border-[var(--hairline)] bg-base-200 p-3">
    <div class="mb-2 text-xs font-medium text-base-content/60">
      接下来 {previewCount} 次执行（本地时间）
    </div>

    {#if previewError}
      <div class="flex items-start gap-2 text-sm text-error">
        <AlertCircle size={14} class="mt-0.5 flex-shrink-0" />
        <span>{previewError}</span>
      </div>
    {:else if loading && occurrences.length === 0}
      <div class="text-sm text-base-content/50">计算中…</div>
    {:else if occurrences.length === 0}
      <div class="text-sm text-base-content/50">近期无可执行时间</div>
    {:else}
      <ol class="flex flex-col gap-1 text-sm text-base-content/80">
        {#each occurrences as ms (ms)}
          <li class="font-mono tabular-nums">{formatOccurrence(ms)}</li>
        {/each}
      </ol>
    {/if}
  </div>
</div>
