import { describe, expect, it } from "vitest";

import type { ProcessInfo } from "../../bindings/ProcessInfo";
import { comparator, queryMatcher, statusMatches } from "./columns";

const p = (pid: number, over: Partial<ProcessInfo> = {}): ProcessInfo => ({
  pid,
  ppid: 1,
  name: `proc${pid}`,
  cmd: [],
  exe: null,
  user: "rayan",
  status: "running",
  cpuPercent: 0,
  cpuPercentAvg: 0,
  memoryRss: 0,
  memoryVirtual: 0,
  startTime: 0,
  runTimeSecs: 0,
  threadCount: null,
  fdCount: null,
  nice: null,
  ...over,
});

describe("comparator", () => {
  it("sorts numbers in both directions with PID tiebreak", () => {
    const rows = [p(3, { cpuPercent: 5 }), p(1, { cpuPercent: 50 }), p(2, { cpuPercent: 5 })];
    expect([...rows].sort(comparator("cpu", "desc")).map((r) => r.pid)).toEqual([1, 2, 3]);
    expect([...rows].sort(comparator("cpu", "asc")).map((r) => r.pid)).toEqual([2, 3, 1]);
  });

  it("keeps unknown values last in either direction", () => {
    const rows = [p(1, { threadCount: null }), p(2, { threadCount: 4 }), p(3, { threadCount: 9 })];
    expect([...rows].sort(comparator("threads", "desc")).map((r) => r.pid)).toEqual([3, 2, 1]);
    expect([...rows].sort(comparator("threads", "asc")).map((r) => r.pid)).toEqual([2, 3, 1]);
  });

  it("sorts names case-insensitively", () => {
    const rows = [p(1, { name: "zsh" }), p(2, { name: "Finder" })];
    expect([...rows].sort(comparator("name", "asc")).map((r) => r.pid)).toEqual([2, 1]);
  });
});

describe("filters", () => {
  it("matches all terms across fields", () => {
    const m = queryMatcher("node 4821")!;
    expect(m(p(4821, { name: "node" }))).toBe(true);
    expect(m(p(4821, { name: "python" }))).toBe(false);
    expect(queryMatcher("   ")).toBeNull();
    expect(queryMatcher("server.js")!(p(1, { cmd: ["node", "server.js"] }))).toBe(true);
  });

  it("groups statuses", () => {
    expect(statusMatches("sleeping", "idle")).toBe(true);
    expect(statusMatches("running", "sleeping")).toBe(false);
    expect(statusMatches("all", "zombie")).toBe(true);
  });
});
