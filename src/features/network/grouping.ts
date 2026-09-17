// Pure socket grouping and filtering for the Network view.
import type { SocketEntry } from "../../bindings/SocketEntry";
import type { TcpState } from "../../bindings/TcpState";

export function isListening(s: SocketEntry): boolean {
  if (s.state === "listen") return true;
  return s.protocol === "udp" && (s.remoteAddr === null || s.remotePort === null || s.remotePort === 0);
}

export interface ProcessGroup {
  key: string;
  pid: number | null;
  name: string;
  sockets: SocketEntry[];
  rxBps: number;
  txBps: number;
  bytesIn: number;
  bytesOut: number;
}

/** "Google Chrome — 14 connections": sockets grouped by owning process, busiest first. */
export function groupByProcess(sockets: SocketEntry[]): ProcessGroup[] {
  const groups = new Map<string, ProcessGroup>();
  for (const s of sockets) {
    const key = s.pid === null ? "unknown" : String(s.pid);
    let g = groups.get(key);
    if (!g) {
      g = {
        key,
        pid: s.pid,
        name: s.processName ?? (s.pid === null ? "Unknown process" : `PID ${s.pid}`),
        sockets: [],
        rxBps: 0,
        txBps: 0,
        bytesIn: 0,
        bytesOut: 0,
      };
      groups.set(key, g);
    }
    g.sockets.push(s);
    g.rxBps += s.rxBps ?? 0;
    g.txBps += s.txBps ?? 0;
    g.bytesIn += s.bytesIn ?? 0;
    g.bytesOut += s.bytesOut ?? 0;
  }
  return [...groups.values()].sort(
    (a, b) => b.sockets.length - a.sockets.length || b.rxBps + b.txBps - (a.rxBps + a.txBps) || a.name.localeCompare(b.name),
  );
}

export type ProtocolFilter = "all" | "tcp" | "udp";
export type StateFilter = "all" | "established" | "opening" | "closing" | "listen";

const STATE_GROUP: Record<TcpState, StateFilter> = {
  listen: "listen",
  synSent: "opening",
  synReceived: "opening",
  established: "established",
  finWait1: "closing",
  finWait2: "closing",
  closeWait: "closing",
  closing: "closing",
  lastAck: "closing",
  timeWait: "closing",
  closed: "closing",
  deleteTcb: "closing",
  unknown: "all",
};

export interface SocketFilter {
  query: string;
  protocol: ProtocolFilter;
  state: StateFilter;
  pid: number | null;
}

export function socketMatcher(filter: SocketFilter, hostFor: (ip: string) => string | null = () => null) {
  const terms = filter.query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return (s: SocketEntry): boolean => {
    if (filter.protocol !== "all" && s.protocol !== filter.protocol) return false;
    if (filter.pid !== null && s.pid !== filter.pid) return false;
    if (filter.state !== "all") {
      if (s.state === null) return false;
      if (STATE_GROUP[s.state] !== filter.state) return false;
    }
    if (terms.length === 0) return true;
    const host = s.remoteHost ?? (s.remoteAddr ? hostFor(s.remoteAddr) : null);
    const haystack = [
      s.processName,
      s.pid,
      s.localAddr,
      s.localPort,
      s.remoteAddr,
      s.remotePort,
      host,
      s.geo?.city,
      s.geo?.country,
    ]
      .filter((v) => v !== null && v !== undefined)
      .join("\n")
      .toLowerCase();
    return terms.every((t) => haystack.includes(t));
  };
}

/** Connections whose remote end cannot be drawn on the map, by reason. */
export function unmappedCounts(sockets: SocketEntry[]): { local: number; unlocated: number } {
  let local = 0;
  let unlocated = 0;
  for (const s of sockets) {
    if (isListening(s) || s.remoteAddr === null) continue;
    if (s.remoteScope === "public") {
      if (!s.geo) unlocated += 1;
    } else {
      local += 1;
    }
  }
  return { local, unlocated };
}
