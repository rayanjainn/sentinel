// Splits a MemoryBreakdown into non-overlapping stacked segments that sum to physical memory.
import type { MemoryBreakdown } from "../../bindings/MemoryBreakdown";

export interface MemorySegments {
  app: number;
  wired: number;
  compressed: number;
  cached: number;
  free: number;
}

export type MemorySegmentKey = keyof MemorySegments;

export function memorySegments(m: MemoryBreakdown): MemorySegments {
  const total = Math.max(0, m.total);
  const used = Math.min(total, Math.max(0, m.used));
  const wired = Math.min(used, Math.max(0, m.wired ?? 0));
  const compressed = Math.min(used - wired, Math.max(0, m.compressed ?? 0));
  const app = used - wired - compressed;
  const cached = Math.min(total - used, Math.max(0, m.cached ?? 0));
  const free = Math.max(0, total - used - cached);
  return { app, wired, compressed, cached, free };
}

/** Segments the OS actually reports, bottom to top. Free is always last. */
export function availableSegments(m: MemoryBreakdown | null | undefined): MemorySegmentKey[] {
  const keys: MemorySegmentKey[] = ["app"];
  if (m?.wired !== null && m?.wired !== undefined) keys.push("wired");
  if (m?.compressed !== null && m?.compressed !== undefined) keys.push("compressed");
  if (m?.cached !== null && m?.cached !== undefined) keys.push("cached");
  keys.push("free");
  return keys;
}
