import { describe, expect, it } from "vitest";

import type { ProcessInfo } from "../../bindings/ProcessInfo";
import { buildForest, flattenForest, parentKeys, processKey } from "./tree";

function proc(pid: number, ppid: number | null, name = `p${pid}`, cpu = 0): ProcessInfo {
  return {
    pid,
    ppid,
    name,
    cmd: [name],
    exe: null,
    user: "u",
    status: "running",
    cpuPercent: cpu,
    cpuPercentAvg: cpu,
    memoryRss: 0,
    memoryVirtual: 0,
    startTime: 100 + pid,
    runTimeSecs: 1,
    threadCount: 1,
    fdCount: 1,
    nice: 0,
    summary: { headline: "", appName: null, role: "unknown", category: "unknown", quitSafety: "unknown", confidence: "unknown" },
  };
}

const byPid = (a: ProcessInfo, b: ProcessInfo) => a.pid - b.pid;

const sample = [proc(1, null), proc(2, 1), proc(3, 1), proc(4, 2), proc(5, 99), proc(6, 6)];

describe("buildForest", () => {
  it("treats missing, self and null parents as roots", () => {
    const f = buildForest(sample);
    expect(f.roots.sort()).toEqual([1, 5, 6]);
    expect(f.children.get(1)).toEqual([2, 3]);
  });

  it("promotes a cycle so no process disappears", () => {
    const f = buildForest([proc(10, 11), proc(11, 10)]);
    const rows = flattenForest(f, { collapsed: new Set(), compare: byPid });
    expect(rows.map((r) => r.process.pid).sort()).toEqual([10, 11]);
  });
});

describe("flattenForest", () => {
  it("emits depth-first rows with guides", () => {
    const rows = flattenForest(buildForest(sample), { collapsed: new Set(), compare: byPid });
    expect(rows.map((r) => [r.process.pid, r.depth])).toEqual([
      [1, 0],
      [2, 1],
      [4, 2],
      [3, 1],
      [5, 0],
      [6, 0],
    ]);
    const four = rows.find((r) => r.process.pid === 4)!;
    // pid 2 has a later sibling (3), so the level-1 guide continues past pid 4.
    expect(four.guides).toEqual([true, true]);
    expect(rows.find((r) => r.process.pid === 3)!.isLast).toBe(true);
    expect(rows[0]!.descendantCount).toBe(3);
  });

  it("honors collapsed keys", () => {
    const f = buildForest(sample);
    const collapsed = new Set([processKey(f.byPid.get(1)!)]);
    const rows = flattenForest(f, { collapsed, compare: byPid });
    expect(rows.map((r) => r.process.pid)).toEqual([1, 5, 6]);
    expect(rows[0]!.expanded).toBe(false);
    expect(rows[0]!.hasChildren).toBe(true);
  });

  it("filters to matches plus ancestors, forcing ancestors open", () => {
    const f = buildForest(sample);
    const collapsed = new Set([processKey(f.byPid.get(1)!)]);
    const rows = flattenForest(f, { collapsed, compare: byPid, match: (p) => p.pid === 4 });
    expect(rows.map((r) => r.process.pid)).toEqual([1, 2, 4]);
  });

  it("sorts siblings with the comparator", () => {
    const f = buildForest([proc(1, null), proc(2, 1, "b", 5), proc(3, 1, "a", 50)]);
    const rows = flattenForest(f, { collapsed: new Set(), compare: (a, b) => b.cpuPercent - a.cpuPercent });
    expect(rows.map((r) => r.process.pid)).toEqual([1, 3, 2]);
  });

  it("lists parent keys", () => {
    expect(parentKeys(buildForest(sample)).sort()).toEqual(["1:101", "2:102"]);
  });
});
