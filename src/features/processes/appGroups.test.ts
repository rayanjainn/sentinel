import { describe, expect, it } from "vitest";

import type { ProcessInfo } from "../../bindings/ProcessInfo";
import { groupByApp } from "./appGroups";

const p = (pid: number, appName: string | null, memoryRss: number, cpuPercent = 0): ProcessInfo => ({
  pid,
  ppid: 1,
  name: `helper${pid}`,
  cmd: [],
  exe: null,
  user: "rayan",
  status: "running",
  cpuPercent,
  cpuPercentAvg: cpuPercent,
  memoryRss,
  memoryVirtual: 0,
  startTime: 0,
  runTimeSecs: 0,
  threadCount: null,
  fdCount: null,
  nice: null,
  summary: { headline: "", appName, role: "unknown", category: "unknown", quitSafety: "unknown", confidence: "unknown" },
});

describe("groupByApp", () => {
  it("collapses two or more helpers of the same app into one group, busiest first", () => {
    const items = groupByApp([
      p(1, "Brave Browser", 100),
      p(2, "Brave Browser", 300),
      p(3, "postgres" as unknown as null, 50), // single-member app, stays ungrouped below
    ]);
    expect(items[0]).toMatchObject({ kind: "group" });
    if (items[0]!.kind !== "group") throw new Error("expected a group");
    expect(items[0]!.group.appName).toBe("Brave Browser");
    expect(items[0]!.group.totalMemory).toBe(400);
    expect(items[0]!.group.members.map((m) => m.pid)).toEqual([2, 1]); // busiest member first
  });

  it("never groups an app with only one member — a real app is not a collapsible group of itself", () => {
    const items = groupByApp([p(1, "Blender", 500)]);
    expect(items).toEqual([{ kind: "single", process: expect.objectContaining({ pid: 1 }) }]);
  });

  it("leaves processes with no recognised app ungrouped", () => {
    const items = groupByApp([p(1, null, 10), p(2, null, 20)]);
    expect(items.every((i) => i.kind === "single")).toBe(true);
  });

  it("orders groups and singles together by total memory, busiest first", () => {
    const items = groupByApp([
      p(1, "App A", 10),
      p(2, "App A", 10), // group total 20
      p(3, null, 500), // single, much bigger
    ]);
    expect(items[0]).toMatchObject({ kind: "single" });
    expect(items[1]).toMatchObject({ kind: "group" });
  });
});
