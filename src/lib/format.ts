// Pure formatters for numbers shown in the UI. Timestamps: `*Ms` fields are epoch milliseconds,
// `startTime` / `modified` / `accessed` are epoch seconds (see crates/sentinel-core/src/model).
import type { Metric } from "../bindings/Metric";

const BINARY_UNITS = ["B", "KB", "MB", "GB", "TB", "PB"] as const;

/**
 * Human-readable byte size. Base 1024 (memory, Activity Monitor style) by default; storage views
 * pass `base: 1000` to match Finder / Explorer volume sizes.
 */
export function formatBytes(bytes: number, options: { base?: 1000 | 1024; digits?: number } = {}): string {
  const base = options.base ?? 1024;
  if (!Number.isFinite(bytes)) return "—";
  const sign = bytes < 0 ? "-" : "";
  let value = Math.abs(bytes);
  let unit = 0;
  while (value >= base && unit < BINARY_UNITS.length - 1) {
    value /= base;
    unit += 1;
  }
  if (unit === 0) return `${sign}${Math.round(value)} B`;
  const digits = options.digits ?? (value >= 100 ? 0 : value >= 10 ? 1 : 2);
  return `${sign}${value.toFixed(digits)} ${BINARY_UNITS[unit]}`;
}

/** Bytes per second, e.g. "1.2 MB/s". */
export function formatRate(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec)) return "—";
  if (bytesPerSec < 1) return "0 B/s";
  return `${formatBytes(bytesPerSec, { base: 1000 })}/s`;
}

export function formatPercent(value: number, digits = 1): string {
  if (!Number.isFinite(value)) return "—";
  const fixed = value.toFixed(digits);
  // Avoid "-0.0%" from tiny negative noise.
  return `${Number(fixed) === 0 ? (0).toFixed(digits) : fixed}%`;
}

const countFormat = new Intl.NumberFormat("en-US");
const compactFormat = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 });

export function formatCount(value: number): string {
  return Number.isFinite(value) ? countFormat.format(Math.round(value)) : "—";
}

export function formatCompact(value: number): string {
  return Number.isFinite(value) ? compactFormat.format(value) : "—";
}

/** Elapsed seconds as a compact duration: "12s", "4m 12s", "2h 05m", "3d 4h". */
export function formatDuration(totalSecs: number): string {
  if (!Number.isFinite(totalSecs) || totalSecs < 0) return "—";
  const secs = Math.floor(totalSecs);
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m ${String(s).padStart(2, "0")}s`;
  return `${s}s`;
}

/** Milliseconds as a short duration for job timings: "840 ms", "3.2 s", "4m 12s". */
export function formatElapsedMs(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "—";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  return formatDuration(ms / 1000);
}

/** Relative time from an epoch-ms timestamp: "just now", "5 min ago", "3 h ago", "47 days ago". */
export function formatRelative(tsMs: number, nowMs: number = Date.now()): string {
  if (!Number.isFinite(tsMs)) return "—";
  const delta = nowMs - tsMs;
  const future = delta < 0;
  const abs = Math.abs(delta);
  const suffix = (s: string) => (future ? `in ${s}` : `${s} ago`);
  if (abs < 45_000) return future ? "in a moment" : "just now";
  const min = Math.round(abs / 60_000);
  if (min < 60) return suffix(`${min} min`);
  const hours = Math.round(abs / 3_600_000);
  if (hours < 24) return suffix(`${hours} h`);
  const days = Math.round(abs / 86_400_000);
  if (days < 60) return suffix(days === 1 ? "1 day" : `${days} days`);
  const months = Math.round(days / 30.44);
  if (months < 24) return suffix(`${months} months`);
  return suffix(`${Math.round(days / 365.25)} years`);
}

export function formatRelativeSecs(tsSecs: number | null, nowMs: number = Date.now()): string {
  return tsSecs === null ? "—" : formatRelative(tsSecs * 1000, nowMs);
}

const dateTimeFormat = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});
const clockFormat = new Intl.DateTimeFormat(undefined, {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
});

export function formatDateTime(tsMs: number): string {
  return Number.isFinite(tsMs) ? dateTimeFormat.format(new Date(tsMs)) : "—";
}

export function formatClock(tsMs: number): string {
  return Number.isFinite(tsMs) ? clockFormat.format(new Date(tsMs)) : "—";
}

export function formatNice(nice: number | null): string {
  if (nice === null) return "—";
  return nice > 0 ? `+${nice}` : String(nice);
}

export function formatMetric(metric: Metric): string {
  switch (metric.unit) {
    case "bytes":
      return formatBytes(metric.value, { base: 1000 });
    case "percent":
      return formatPercent(metric.value);
    case "count":
      return formatCount(metric.value);
    case "nice":
      return formatNice(metric.value);
  }
}

/** Shortens a path for narrow cells while keeping the leaf: "/Users/a/…/deep/file.txt". */
export function middleTruncatePath(path: string, max = 48): string {
  if (path.length <= max) return path;
  const parts = path.split(/([/\\])/);
  const leaf = parts.slice(-1)[0] ?? "";
  if (leaf.length + 2 >= max) return `…${leaf.slice(-(max - 1))}`;
  let head = "";
  for (const part of parts) {
    if (head.length + part.length + leaf.length + 2 > max) break;
    head += part;
  }
  const sep = path.includes("\\") && !path.includes("/") ? "\\" : "/";
  return `${head.replace(/[/\\]$/, "")}${sep}…${sep}${leaf}`;
}

/** Replaces the user's home prefix with "~" for display. */
export function tildify(path: string, home: string | null): string {
  if (!home || home === "/") return path;
  if (path === home) return "~";
  return path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;
}

export function pluralize(count: number, singular: string, plural = `${singular}s`): string {
  return `${formatCount(count)} ${count === 1 ? singular : plural}`;
}
