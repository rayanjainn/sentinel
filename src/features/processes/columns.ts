// Column definitions, sorting and search for the process table.
import type { ProcessInfo } from "../../bindings/ProcessInfo";
import type { ProcessStatus } from "../../bindings/ProcessStatus";
import type { GlossaryId } from "../../lib/glossary";

export type SortKey =
  | "name"
  | "pid"
  | "user"
  | "status"
  | "cpu"
  | "cpuAvg"
  | "memory"
  | "virtual"
  | "threads"
  | "fds"
  | "nice"
  | "started"
  | "runtime";

export type SortDir = "asc" | "desc";

export interface Column {
  key: SortKey;
  label: string;
  /** CSS grid track. */
  track: string;
  align: "left" | "right";
  defaultDir: SortDir;
  /** Glossary entry explained by this column's `?`/`i` tooltip. */
  glossaryId?: GlossaryId;
}

export const COLUMNS: Column[] = [
  { key: "name", label: "Name", track: "minmax(220px,1fr)", align: "left", defaultDir: "asc" },
  { key: "pid", label: "PID", track: "64px", align: "right", defaultDir: "asc", glossaryId: "pid" },
  { key: "user", label: "User", track: "104px", align: "left", defaultDir: "asc" },
  { key: "status", label: "Status", track: "96px", align: "left", defaultDir: "asc", glossaryId: "processStatus" },
  { key: "cpu", label: "CPU", track: "72px", align: "right", defaultDir: "desc", glossaryId: "cpuPercent" },
  { key: "cpuAvg", label: "Avg CPU", track: "82px", align: "right", defaultDir: "desc", glossaryId: "cpuPercentAvg" },
  { key: "memory", label: "Memory", track: "84px", align: "right", defaultDir: "desc", glossaryId: "memoryRss" },
  { key: "virtual", label: "Virtual", track: "84px", align: "right", defaultDir: "desc", glossaryId: "memoryVirtual" },
  { key: "threads", label: "Threads", track: "76px", align: "right", defaultDir: "desc", glossaryId: "threadCount" },
  { key: "fds", label: "Files", track: "62px", align: "right", defaultDir: "desc", glossaryId: "fdCount" },
  { key: "nice", label: "Nice", track: "58px", align: "right", defaultDir: "asc", glossaryId: "nice" },
  { key: "started", label: "Started", track: "96px", align: "right", defaultDir: "desc", glossaryId: "startTime" },
  { key: "runtime", label: "Running", track: "80px", align: "right", defaultDir: "desc", glossaryId: "runTime" },
];

export const GRID_TEMPLATE = COLUMNS.map((c) => c.track).join(" ");

export const STATUS_LABEL: Record<ProcessStatus, string> = {
  running: "Running",
  sleeping: "Sleeping",
  idle: "Idle",
  waiting: "Waiting",
  stopped: "Stopped",
  zombie: "Zombie",
  dead: "Exited",
  unknown: "Unknown",
};

type Getter = (p: ProcessInfo) => number | string | null;

const GETTERS: Record<SortKey, Getter> = {
  name: (p) => p.name.toLowerCase(),
  pid: (p) => p.pid,
  user: (p) => p.user?.toLowerCase() ?? null,
  status: (p) => p.status,
  cpu: (p) => p.cpuPercent,
  cpuAvg: (p) => p.cpuPercentAvg,
  memory: (p) => p.memoryRss,
  virtual: (p) => p.memoryVirtual,
  threads: (p) => p.threadCount,
  fds: (p) => p.fdCount,
  nice: (p) => p.nice,
  started: (p) => p.startTime,
  runtime: (p) => p.runTimeSecs,
};

/** Comparator with unknown values (null) always last, ties broken by PID for a stable order. */
export function comparator(key: SortKey, dir: SortDir): (a: ProcessInfo, b: ProcessInfo) => number {
  const get = GETTERS[key];
  const sign = dir === "asc" ? 1 : -1;
  return (a, b) => {
    const va = get(a);
    const vb = get(b);
    if (va === null && vb === null) return a.pid - b.pid;
    if (va === null) return 1;
    if (vb === null) return -1;
    if (va < vb) return -sign;
    if (va > vb) return sign;
    return a.pid - b.pid;
  };
}

export type StatusFilter = "all" | "running" | "sleeping" | "stopped" | "zombie";

export function statusMatches(filter: StatusFilter, status: ProcessStatus): boolean {
  switch (filter) {
    case "all":
      return true;
    case "running":
      return status === "running";
    case "sleeping":
      return status === "sleeping" || status === "idle" || status === "waiting";
    case "stopped":
      return status === "stopped";
    case "zombie":
      return status === "zombie" || status === "dead";
  }
}

/** Space-separated terms must all match the name, PID, user or command line. */
export function queryMatcher(query: string): ((p: ProcessInfo) => boolean) | null {
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return null;
  return (p) => {
    const haystack = `${p.name}\n${p.pid}\n${p.user ?? ""}\n${p.cmd.join(" ")}\n${p.exe ?? ""}`.toLowerCase();
    return terms.every((t) => haystack.includes(t));
  };
}
