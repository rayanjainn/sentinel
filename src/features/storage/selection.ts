// Batch selection helpers for storage actions.
import type { DuplicateGroup } from "../../bindings/DuplicateGroup";
import { isWithin } from "./treemap";

export interface SizedPath {
  path: string;
  sizeBytes: number;
}

/**
 * Drops paths nested under another selected path so a folder and its contents are never counted
 * or trashed twice.
 */
export function collapseNested<T extends SizedPath>(items: T[]): T[] {
  const sorted = [...items].sort((a, b) => a.path.length - b.path.length);
  const kept: T[] = [];
  for (const item of sorted) {
    if (kept.some((k) => isWithin(item.path, k.path))) continue;
    kept.push(item);
  }
  return kept;
}

export function summarize(items: SizedPath[]): { count: number; bytes: number } {
  const collapsed = collapseNested(items);
  return { count: collapsed.length, bytes: collapsed.reduce((sum, i) => sum + i.sizeBytes, 0) };
}

/** Keeps the oldest copy of each group (by modification time, then shortest path) and selects the rest. */
export function defaultDuplicateSelection(groups: DuplicateGroup[]): Set<string> {
  const selected = new Set<string>();
  for (const group of groups) {
    const keep = keeperOf(group);
    for (const f of group.files) if (f.path !== keep) selected.add(f.path);
  }
  return selected;
}

export function keeperOf(group: DuplicateGroup): string | null {
  const sorted = [...group.files].sort(
    (a, b) => (a.modified ?? Number.MAX_SAFE_INTEGER) - (b.modified ?? Number.MAX_SAFE_INTEGER) || a.path.length - b.path.length,
  );
  return sorted[0]?.path ?? null;
}

/** A group is safe when at least one copy stays unselected. */
export function keepsOneCopy(group: DuplicateGroup, selected: ReadonlySet<string>): boolean {
  return group.files.some((f) => !selected.has(f.path));
}

export function toggle(set: ReadonlySet<string>, key: string, on?: boolean): Set<string> {
  const next = new Set(set);
  const shouldAdd = on ?? !next.has(key);
  if (shouldAdd) next.add(key);
  else next.delete(key);
  return next;
}
