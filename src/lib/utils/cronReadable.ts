/**
 * 把常见 5 段 cron 表达式翻译为人类可读的中文调度文案。
 *
 * 覆盖 jobs UI 里常用的预设：每分钟 / 每 N 分钟 / 每小时 / 每 N 小时 /
 * 每天 HH:MM / 每周某天 HH:MM / 每月某日 HH:MM。无法匹配预设时回退到
 * 原始 cron 字符串，保证永远有可展示文案。
 *
 * 仅依赖标准 5 段格式 `minute hour day-of-month month day-of-week`。
 * schedule-editor feature 复用此函数为编辑器提供实时预览。
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

/** 把 0-23 / 0-59 数值补零成两位字符串。 */
function pad2(n: number): string {
  return n.toString().padStart(2, "0");
}

/** 把 `minute` `hour` 两字段格式化为 `HH:MM`；非纯数字返回 null。 */
function formatTime(minute: string, hour: string): string | null {
  const m = Number(minute);
  const h = Number(hour);
  if (!Number.isInteger(m) || !Number.isInteger(h)) return null;
  if (m < 0 || m > 59 || h < 0 || h > 23) return null;
  return `${pad2(h)}:${pad2(m)}`;
}

/** 解析 `*​/N` 步进字段，返回 N（要求 N >= 1 的整数），否则 null。 */
function parseStep(field: string): number | null {
  const match = /^\*\/(\d+)$/.exec(field);
  if (!match) return null;
  const n = Number(match[1]);
  return Number.isInteger(n) && n >= 1 ? n : null;
}

/**
 * 解析 day-of-week 字段为有序去重的 0-6 列表（支持逗号多选，如 `1,3,5`）。
 * 任一段非 0-6 纯数字则返回 null（交由调用方回退原始表达式）。
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

/**
 * 将 cron 表达式转为可读中文文案。无法识别的表达式原样返回。
 *
 * @param cron 标准 5 段 cron 表达式
 */
export function cronToHuman(cron: string): string {
  const expr = cron.trim();
  if (!expr) return expr;

  const fields = expr.split(/\s+/);
  if (fields.length !== 5) return expr;

  const [minute, hour, dayOfMonth, month, dayOfWeek] = fields;

  // 每分钟：* * * * *
  if (
    minute === "*" &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return "每分钟";
  }

  // 每 N 分钟：*/N * * * *
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

  // 每小时（整点）：0 * * * *
  if (
    minute === "0" &&
    hour === "*" &&
    dayOfMonth === "*" &&
    month === "*" &&
    dayOfWeek === "*"
  ) {
    return "每小时";
  }

  // 每 N 小时（整点）：0 */N * * *
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

  // 固定到分钟的时间点（minute/hour 为纯数字）
  const time = formatTime(minute, hour);
  if (time !== null && month === "*") {
    // 每天 HH:MM：m h * * *
    if (dayOfMonth === "*" && dayOfWeek === "*") {
      return `每天 ${time}`;
    }

    // 每周某天[可多选] HH:MM：m h * * W[,W...]
    if (dayOfMonth === "*" && dayOfWeek !== "*") {
      const weekdays = parseWeekdays(dayOfWeek);
      if (weekdays !== null) {
        const names = weekdays.map((w) => WEEKDAY_NAMES[w]).join("、");
        return `每${names} ${time}`;
      }
    }

    // 每月某日 HH:MM：m h D * *
    if (dayOfMonth !== "*" && dayOfWeek === "*") {
      const d = Number(dayOfMonth);
      if (Number.isInteger(d) && d >= 1 && d <= 31) {
        return `每月 ${d} 日 ${time}`;
      }
    }
  }

  // 非预设：回退到原始表达式
  return expr;
}
