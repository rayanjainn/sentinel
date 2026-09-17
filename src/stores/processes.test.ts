import { describe, expect, it, vi } from "vitest";

vi.mock("../lib/ipc", () => ({ api: {}, on: vi.fn(() => Promise.resolve(() => undefined)) }));

import type { ProcessInfo } from "../bindings/ProcessInfo";
import { reconcile, sameProcess, useProcesses } from "./processes";

const base: ProcessInfo = {
  pid: 7,
  ppid: 1,
  name: "node",
  cmd: ["node", "server.js"],
  exe: "/usr/local/bin/node",
  user: "rayan",
  status: "running",
  cpuPercent: 3.5,
  cpuPercentAvg: 2.1,
  memoryRss: 1024,
  memoryVirtual: 4096,
  startTime: 1000,
  runTimeSecs: 60,
  threadCount: 9,
  fdCount: 30,
  nice: 0,
};

describe("process reconciliation", () => {
  it("reuses unchanged rows and replaces changed ones", () => {
    const prev = new Map([[7, base]]);
    const same = { ...base, cmd: [...base.cmd] };
    const changed = { ...base, pid: 8, cpuPercent: 9 };
    const out = reconcile(prev, [same, changed]);
    expect(out[0]).toBe(base);
    expect(out[1]).toBe(changed);
  });

  it("detects command line changes", () => {
    expect(sameProcess(base, { ...base, cmd: ["node", "other.js"] })).toBe(false);
  });

  it("ignores snapshots older than the current one", () => {
    const { apply } = useProcesses.getState();
    apply({ tsMs: 2000, processes: [base], logicalCores: 8, totalMemory: 16 });
    apply({ tsMs: 1000, processes: [], logicalCores: 8, totalMemory: 16 });
    expect(useProcesses.getState().processes).toHaveLength(1);
    expect(useProcesses.getState().status).toBe("ready");
  });
});
