import { describe, expect, it } from "vitest";

import {
  formatBytes,
  formatCount,
  formatDuration,
  formatElapsedMs,
  formatMetric,
  formatNice,
  formatPercent,
  formatRate,
  formatRelative,
  middleTruncatePath,
  pluralize,
  tildify,
} from "./format";

describe("formatBytes", () => {
  it("formats binary sizes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.50 KB");
    expect(formatBytes(10.5 * 1024 ** 3)).toBe("10.5 GB");
    expect(formatBytes(512 * 1024 ** 2)).toBe("512 MB");
  });

  it("supports decimal base for storage", () => {
    expect(formatBytes(2_300_000_000, { base: 1000 })).toBe("2.30 GB");
  });

  it("handles negatives and non-finite", () => {
    expect(formatBytes(-2048)).toBe("-2.00 KB");
    expect(formatBytes(Number.NaN)).toBe("—");
  });
});

describe("formatRate", () => {
  it("formats per-second rates", () => {
    expect(formatRate(0)).toBe("0 B/s");
    expect(formatRate(1_250_000)).toBe("1.25 MB/s");
  });
});

describe("formatPercent", () => {
  it("never renders negative zero", () => {
    expect(formatPercent(-0.01)).toBe("0.0%");
    expect(formatPercent(47.25)).toBe("47.3%");
    expect(formatPercent(3, 0)).toBe("3%");
  });
});

describe("formatDuration", () => {
  it("picks the two most significant units", () => {
    expect(formatDuration(12)).toBe("12s");
    expect(formatDuration(252)).toBe("4m 12s");
    expect(formatDuration(7500)).toBe("2h 05m");
    expect(formatDuration(3 * 86400 + 4 * 3600 + 5)).toBe("3d 4h");
    expect(formatDuration(-1)).toBe("—");
  });

  it("formats millisecond job timings", () => {
    expect(formatElapsedMs(840)).toBe("840 ms");
    expect(formatElapsedMs(3200)).toBe("3.2 s");
    expect(formatElapsedMs(252_000)).toBe("4m 12s");
  });
});

describe("formatRelative", () => {
  const now = 1_700_000_000_000;
  it("formats past times", () => {
    expect(formatRelative(now - 10_000, now)).toBe("just now");
    expect(formatRelative(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(formatRelative(now - 3 * 3_600_000, now)).toBe("3 h ago");
    expect(formatRelative(now - 86_400_000, now)).toBe("1 day ago");
    expect(formatRelative(now - 47 * 86_400_000, now)).toBe("47 days ago");
    expect(formatRelative(now - 400 * 86_400_000, now)).toBe("13 months ago");
  });

  it("formats future times", () => {
    expect(formatRelative(now + 4 * 60_000, now)).toBe("in 4 min");
  });
});

describe("misc", () => {
  it("formats counts and nice values", () => {
    expect(formatCount(1284)).toBe("1,284");
    expect(formatNice(5)).toBe("+5");
    expect(formatNice(-10)).toBe("-10");
    expect(formatNice(null)).toBe("—");
    expect(pluralize(1, "file")).toBe("1 file");
    expect(pluralize(3, "copy", "copies")).toBe("3 copies");
  });

  it("formats metrics by unit", () => {
    expect(formatMetric({ key: "k", label: "l", value: 62.04, unit: "percent" })).toBe("62.0%");
    expect(formatMetric({ key: "k", label: "l", value: 8_700_000_000, unit: "bytes" })).toBe("8.70 GB");
    expect(formatMetric({ key: "k", label: "l", value: 14, unit: "count" })).toBe("14");
  });

  it("truncates paths keeping the leaf", () => {
    const p = "/Users/rayan/Library/Application Support/Some App/Caches/deep/file.bin";
    const out = middleTruncatePath(p, 40);
    expect(out.endsWith("/file.bin")).toBe(true);
    expect(out.length).toBeLessThanOrEqual(40);
    expect(middleTruncatePath("/short", 40)).toBe("/short");
  });

  it("tildifies home paths", () => {
    expect(tildify("/Users/rayan/Downloads", "/Users/rayan")).toBe("~/Downloads");
    expect(tildify("/Users/rayanx", "/Users/rayan")).toBe("/Users/rayanx");
    expect(tildify("/Users/rayan", "/Users/rayan")).toBe("~");
  });
});
