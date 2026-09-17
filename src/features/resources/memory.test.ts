import { describe, expect, it } from "vitest";

import type { MemoryBreakdown } from "../../bindings/MemoryBreakdown";
import { availableSegments, memorySegments } from "./memory";

const GB = 1024 ** 3;

const mac: MemoryBreakdown = {
  total: 16 * GB,
  used: 11 * GB,
  available: 5 * GB,
  free: 1 * GB,
  cached: 4 * GB,
  compressed: 2 * GB,
  wired: 3 * GB,
  swapTotal: 2 * GB,
  swapUsed: 0.5 * GB,
};

const sum = (s: ReturnType<typeof memorySegments>) => s.app + s.wired + s.compressed + s.cached + s.free;

describe("memorySegments", () => {
  it("splits used memory and sums to the total", () => {
    const s = memorySegments(mac);
    expect(s).toEqual({ app: 6 * GB, wired: 3 * GB, compressed: 2 * GB, cached: 4 * GB, free: 1 * GB });
    expect(sum(s)).toBe(16 * GB);
  });

  it("handles platforms without detail", () => {
    const s = memorySegments({ ...mac, cached: null, compressed: null, wired: null });
    expect(s.app).toBe(11 * GB);
    expect(s.free).toBe(5 * GB);
    expect(sum(s)).toBe(16 * GB);
    expect(availableSegments({ ...mac, cached: null, compressed: null, wired: null })).toEqual(["app", "free"]);
  });

  it("clamps inconsistent counters instead of overflowing", () => {
    const s = memorySegments({ ...mac, used: 20 * GB, wired: 30 * GB, cached: 9 * GB });
    expect(sum(s)).toBe(16 * GB);
    expect(s.wired).toBe(16 * GB);
    expect(s.cached).toBe(0);
  });

  it("lists reported segments in stack order", () => {
    expect(availableSegments(mac)).toEqual(["app", "wired", "compressed", "cached", "free"]);
  });
});
