/**
 * Translate common 5-field cron expressions into human-readable Chinese labels.
 *
 * Covers the presets used by the jobs UI: every minute / every N minutes /
 * hourly / every N hours / daily, weekly, monthly at HH:MM. Anything else
 * falls back to the raw cron string so there is always displayable text.
 *
 * Only the standard 5-field format `minute hour day-of-month month day-of-week`
 * is supported.
 */

const WEEKDAY_NAMES = [
  "周日",
  "周一",
  "周二",
  "周三",
  "周四",
  "周五",
  "周六",
];

function pad2(n: number): string {
  return n.toString().padStart(2, "0");
}

function formatTime(minute: string, hour: string): string | null {
  const m = Number(minute);
  const h = Number(hour);
  if (!Number.isInteger(m) || !Number.isInteger(h)) return null;
  if (m < 0 || m > 59 || h < 0 || h > 23) return null;
  return `${pad2(h)}:${pad2(m)}`;
}

function parseStep(field: string): number | null {
  const match = /^\*\/(\d+)$/.exec(field);
  if (!match) return null;
  const n = Number(match[1]);
  return Number.isInteger(n) && n >= 1 ? n : null;
}

/**
 * Parse day-of-week into a sorted, deduped 0-6 list (comma multi-select).
 * Any non 0-6 numeric part yields null so the caller falls back to the raw expression.
 */
function parseWeekdays(field: string): number[] | null {
  const parts = field.split(",");
  const set = new Set<number>();
  for (const part of parts) {
    if (!/^\d+$/.test(part)) return null;
    const w = Number(part);
    if (!Number.isInteger(w) || w < 0 || w > 6) return null;
    set.add(w);
  }
  if (set.size === 0) return null;
  return [...set].sort((a, b) => a - b);
}

/** Convert a cron expression into a readable Chinese label; unrecognized expressions are returned as-is. */
export function cronToHuman(cron: string): string {
  const expr = cron.trim();
  if (!expr) return expr;

  const fields = expr.split(/\s+/);
  if (fields.length !== 5) return expr;

  const [minute, hour, dayOfMonth, month, dayOfWeek] = fields;

  // Every minute: * * * * *
  if (
    minute === "*" &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return "每分钟";
  }

  // Every N minutes: */N * * * *
  const minuteStep = parseStep(minute);
  if (
    minuteStep !== null &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return `每 ${minuteStep} 分钟`;
  }

  // Hourly (on the hour): 0 * * * *
  if (
    minute === "0" &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return "每小时";
  }

  // Every N hours (on the hour): 0 */N * * *
  const hourStep = parseStep(hour);
  if (
    minute === "0" &&
    hourStep !== null &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return `每 ${hourStep} 小时`;
  }

  // Fixed time of day (minute/hour are plain numbers)
  const time = formatTime(minute, hour);
  if (time !== null && month === "*") {
    // Daily at HH:MM: m h * * *
    if (dayOfMonth === "*" && dayOfWeek === "*") {
      return `每天 ${time}`;
    }

    // Weekly (multi-select) at HH:MM: m h * * W[,W...]
    if (dayOfMonth === "*" && dayOfWeek !== "*") {
      const weekdays = parseWeekdays(dayOfWeek);
      if (weekdays !== null) {
        const names = weekdays.map((w) => WEEKDAY_NAMES[w]).join("、");
        return `每${names} ${time}`;
      }
    }

    // Monthly at HH:MM: m h D * *
    if (dayOfMonth !== "*" && dayOfWeek === "*") {
      const d = Number(dayOfMonth);
      if (Number.isInteger(d) && d >= 1 && d <= 31) {
        return `每月 ${d} 日 ${time}`;
      }
    }
  }

  // Not a preset: fall back to the raw expression.
  return expr;
}
