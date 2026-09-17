// Pure helpers for rolling time series fed by events and backfilled by history commands.

export interface Timestamped {
  tsMs: number;
}

/**
 * Merges `incoming` into `existing` (both ascending by tsMs), dropping duplicates by timestamp and
 * anything older than `maxAgeMs` before the newest sample. Returns `existing` itself when nothing
 * changed so subscribers can skip work.
 */
export function mergeSeries<T extends Timestamped>(existing: T[], incoming: T[], maxAgeMs: number): T[] {
  if (incoming.length === 0) return existing;
  let merged: T[];
  const last = existing[existing.length - 1];
  const sortedIncoming = isAscending(incoming) ? incoming : [...incoming].sort((a, b) => a.tsMs - b.tsMs);
  const firstIncoming = sortedIncoming[0]!;
  if (!last || firstIncoming.tsMs > last.tsMs) {
    merged = existing.concat(dedupe(sortedIncoming));
  } else {
    const byTs = new Map<number, T>();
    for (const s of existing) byTs.set(s.tsMs, s);
    for (const s of sortedIncoming) byTs.set(s.tsMs, s);
    merged = [...byTs.values()].sort((a, b) => a.tsMs - b.tsMs);
  }
  const newest = merged[merged.length - 1]!.tsMs;
  const cutoff = newest - maxAgeMs;
  let start = 0;
  while (start < merged.length && merged[start]!.tsMs < cutoff) start += 1;
  return start > 0 ? merged.slice(start) : merged;
}

function isAscending<T extends Timestamped>(items: T[]): boolean {
  for (let i = 1; i < items.length; i += 1) if (items[i]!.tsMs < items[i - 1]!.tsMs) return false;
  return true;
}

function dedupe<T extends Timestamped>(items: T[]): T[] {
  const out: T[] = [];
  for (const item of items) {
    const prev = out[out.length - 1];
    if (prev && prev.tsMs === item.tsMs) out[out.length - 1] = item;
    else out.push(item);
  }
  return out;
}

/** Index of the first sample with tsMs >= fromMs (binary search). */
export function lowerBound<T extends Timestamped>(items: T[], fromMs: number): number {
  let lo = 0;
  let hi = items.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (items[mid]!.tsMs < fromMs) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Samples within the trailing window, including one point before it so lines enter from the edge. */
export function windowed<T extends Timestamped>(items: T[], windowMs: number, nowMs: number): T[] {
  const idx = lowerBound(items, nowMs - windowMs);
  return items.slice(Math.max(0, idx - 1));
}
