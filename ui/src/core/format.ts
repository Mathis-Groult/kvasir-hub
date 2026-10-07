// Pure formatting helpers. Every function takes its inputs explicitly (no clock, no locale).

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/** 1_500_000 -> "1.5 MB" (SI units, as Seafile reports quotas). */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1000 && unit < BYTE_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : value >= 10 ? 1 : 2;
  // Drop a fractional part that is only zeros: "2.00" -> "2", "1.50" -> "1.5".
  const text = String(Number(value.toFixed(digits)));
  return `${text} ${BYTE_UNITS[unit]}`;
}

/** 193 -> "3:13"; 3725 -> "1:02:05". */
export function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

/** "12 min", "3 h", "2 d" ago, or an ISO date after 30 days. */
export function formatAge(thenSeconds: number, nowSeconds: number): string {
  const delta = nowSeconds - thenSeconds;
  if (delta < 45) return 'just now';
  if (delta < 3600) return `${Math.round(delta / 60)} min ago`;
  if (delta < 86_400) return `${Math.round(delta / 3600)} h ago`;
  if (delta < 30 * 86_400) return `${Math.round(delta / 86_400)} d ago`;
  return new Date(thenSeconds * 1000).toISOString().slice(0, 10);
}

/** `plural(1, 'song')` -> "1 song"; `plural(2, 'song')` -> "2 songs"; `plural(2, 'library', 'libraries')`. */
export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

/** Whole percent of `used` in `total`, clamped to 0..100. */
export function percent(used: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((used / total) * 100)));
}
