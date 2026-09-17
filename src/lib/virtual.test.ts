import { describe, expect, it } from "vitest";

import { computeRange, scrollOffsetFor } from "./virtual";

describe("computeRange", () => {
  it("covers the viewport plus overscan", () => {
    expect(computeRange(0, 280, 28, 1000, 5)).toEqual({ start: 0, end: 16 });
    expect(computeRange(2800, 280, 28, 1000, 5)).toEqual({ start: 95, end: 116 });
  });

  it("clamps at the end and handles empty lists", () => {
    expect(computeRange(27_900, 280, 28, 1000, 5)).toEqual({ start: 991, end: 1000 });
    expect(computeRange(0, 280, 28, 0, 5)).toEqual({ start: 0, end: 0 });
  });

  it("stays valid when scrolled past a shrunken list", () => {
    const r = computeRange(10_000, 280, 28, 3, 2);
    expect(r.start).toBeLessThanOrEqual(r.end);
    expect(r.end).toBe(3);
  });
});

describe("scrollOffsetFor", () => {
  it("scrolls minimally", () => {
    expect(scrollOffsetFor(5, 0, 280, 28)).toBeNull();
    expect(scrollOffsetFor(20, 0, 280, 28)).toBe(21 * 28 - 280);
    expect(scrollOffsetFor(2, 200, 280, 28)).toBe(56);
  });
});
