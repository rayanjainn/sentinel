import { describe, expect, it } from "vitest";

import { lowerBound, mergeSeries, windowed } from "./series";

const s = (tsMs: number, v = tsMs) => ({ tsMs, v });

describe("mergeSeries", () => {
  it("appends newer samples", () => {
    expect(mergeSeries([s(1), s(2)], [s(3)], 100).map((x) => x.tsMs)).toEqual([1, 2, 3]);
  });

  it("backfills older history without duplicates", () => {
    const out = mergeSeries([s(5), s(6)], [s(3), s(4), s(5, 99)], 100);
    expect(out.map((x) => x.tsMs)).toEqual([3, 4, 5, 6]);
    expect(out[2]!.v).toBe(99);
  });

  it("prunes beyond max age", () => {
    expect(mergeSeries([s(0), s(50)], [s(200)], 100).map((x) => x.tsMs)).toEqual([200]);
  });

  it("returns the same array for empty input", () => {
    const arr = [s(1)];
    expect(mergeSeries(arr, [], 10)).toBe(arr);
  });

  it("sorts unsorted incoming batches", () => {
    expect(mergeSeries([], [s(3), s(1), s(2)], 100).map((x) => x.tsMs)).toEqual([1, 2, 3]);
  });
});

describe("windowing", () => {
  const items = [s(0), s(10), s(20), s(30)];
  it("binary searches", () => {
    expect(lowerBound(items, 15)).toBe(2);
    expect(lowerBound(items, -5)).toBe(0);
    expect(lowerBound(items, 99)).toBe(4);
  });

  it("keeps one leading point", () => {
    expect(windowed(items, 15, 30).map((x) => x.tsMs)).toEqual([10, 20, 30]);
  });
});
