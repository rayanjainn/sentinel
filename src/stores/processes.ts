// Process table state, normalized by pid. Unchanged rows keep object identity across snapshots so
// memoized rows skip re-rendering.
import { useEffect } from "react";
import { create } from "zustand";

import type { ProcessInfo } from "../bindings/ProcessInfo";
import type { ProcessSnapshot } from "../bindings/ProcessSnapshot";
import { subscribe } from "../lib/events";
import { api } from "../lib/ipc";
import { useStream } from "../lib/sampling";

export function sameProcess(a: ProcessInfo, b: ProcessInfo): boolean {
  return (
    a.pid === b.pid &&
    a.startTime === b.startTime &&
    a.ppid === b.ppid &&
    a.name === b.name &&
    a.status === b.status &&
    a.cpuPercent === b.cpuPercent &&
    a.cpuPercentAvg === b.cpuPercentAvg &&
    a.memoryRss === b.memoryRss &&
    a.memoryVirtual === b.memoryVirtual &&
    a.runTimeSecs === b.runTimeSecs &&
    a.threadCount === b.threadCount &&
    a.fdCount === b.fdCount &&
    a.nice === b.nice &&
    a.user === b.user &&
    a.exe === b.exe &&
    a.cmd.length === b.cmd.length &&
    a.cmd.every((part, i) => part === b.cmd[i])
  );
}

export function reconcile(prev: ReadonlyMap<number, ProcessInfo>, next: ProcessInfo[]): ProcessInfo[] {
  return next.map((p) => {
    const old = prev.get(p.pid);
    return old && sameProcess(old, p) ? old : p;
  });
}

type Status = "idle" | "loading" | "ready" | "error";

interface ProcessesState {
  processes: ProcessInfo[];
  byPid: Map<number, ProcessInfo>;
  tsMs: number;
  logicalCores: number;
  totalMemory: number;
  status: Status;
  error: unknown;
  load: () => Promise<void>;
  apply: (snapshot: ProcessSnapshot) => void;
}

export const useProcesses = create<ProcessesState>((set, get) => ({
  processes: [],
  byPid: new Map(),
  tsMs: 0,
  logicalCores: 0,
  totalMemory: 0,
  status: "idle",
  error: null,
  load: async () => {
    if (get().status !== "ready") set({ status: "loading" });
    try {
      get().apply(await api.getProcessSnapshot());
    } catch (error) {
      set({ status: get().processes.length > 0 ? "ready" : "error", error });
    }
  },
  apply: (snapshot) => {
    const { byPid, tsMs } = get();
    if (snapshot.tsMs < tsMs) return;
    const processes = reconcile(byPid, snapshot.processes);
    const nextByPid = new Map<number, ProcessInfo>();
    for (const p of processes) nextByPid.set(p.pid, p);
    set({
      processes,
      byPid: nextByPid,
      tsMs: snapshot.tsMs,
      logicalCores: snapshot.logicalCores,
      totalMemory: snapshot.totalMemory,
      status: "ready",
      error: null,
    });
  },
}));

let listeners = 0;
let unlisten: (() => void) | null = null;

/**
 * Subscribes the calling view to live process snapshots: acquires the `processes` stream, listens
 * for events while any consumer is mounted, and loads a fresh snapshot on mount.
 */
export function useProcessFeed(active = true): void {
  useStream("processes", active);
  useEffect(() => {
    if (!active) return;
    listeners += 1;
    if (!unlisten) unlisten = subscribe("sentinel:processes", (s) => useProcesses.getState().apply(s));
    const { status, tsMs, load } = useProcesses.getState();
    if (status !== "ready" || Date.now() - tsMs > 3000) void load();
    return () => {
      listeners -= 1;
      if (listeners === 0 && unlisten) {
        unlisten();
        unlisten = null;
      }
    };
  }, [active]);
}
