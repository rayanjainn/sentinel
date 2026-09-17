import { describe, expect, it } from "vitest";

import type { SocketEntry } from "../../bindings/SocketEntry";
import { groupByProcess, isListening, socketMatcher, unmappedCounts } from "./grouping";

export function socket(over: Partial<SocketEntry> = {}): SocketEntry {
  return {
    id: Math.random().toString(36),
    protocol: "tcp",
    family: "v4",
    localAddr: "192.168.1.4",
    localPort: 50000,
    remoteAddr: "142.250.1.1",
    remotePort: 443,
    remoteScope: "public",
    state: "established",
    pid: 100,
    processName: "Google Chrome",
    remoteHost: null,
    geo: null,
    bytesIn: null,
    bytesOut: null,
    rxBps: null,
    txBps: null,
    firstSeenMs: 0,
    ...over,
  };
}

describe("isListening", () => {
  it("detects TCP listeners and unconnected UDP sockets", () => {
    expect(isListening(socket({ state: "listen", remoteAddr: null, remotePort: null }))).toBe(true);
    expect(isListening(socket({ protocol: "udp", state: null, remoteAddr: null, remotePort: null }))).toBe(true);
    expect(isListening(socket())).toBe(false);
  });
});

describe("groupByProcess", () => {
  it("groups by pid with busiest first and sums traffic", () => {
    const groups = groupByProcess([
      socket({ pid: 1, processName: "a", rxBps: 10 }),
      socket({ pid: 2, processName: "b", rxBps: 5 }),
      socket({ pid: 2, processName: "b", txBps: 7 }),
      socket({ pid: null, processName: null }),
    ]);
    expect(groups.map((g) => g.name)).toEqual(["b", "a", "Unknown process"]);
    expect(groups[0]!.rxBps + groups[0]!.txBps).toBe(12);
  });
});

describe("socketMatcher", () => {
  it("filters by protocol, state, pid and text including resolved hosts", () => {
    const s = socket({ remoteAddr: "1.1.1.1" });
    const base = { query: "", protocol: "all", state: "all", pid: null } as const;
    expect(socketMatcher({ ...base, protocol: "udp" })(s)).toBe(false);
    expect(socketMatcher({ ...base, state: "closing" })(socket({ state: "timeWait" }))).toBe(true);
    expect(socketMatcher({ ...base, pid: 5 })(s)).toBe(false);
    expect(socketMatcher({ ...base, query: "cloudflare" }, () => "one.one.one.cloudflare")(s)).toBe(true);
    expect(socketMatcher({ ...base, query: "chrome 443" })(s)).toBe(true);
  });
});

describe("unmappedCounts", () => {
  it("counts local and unlocated remote endpoints", () => {
    expect(
      unmappedCounts([
        socket({ remoteScope: "private" }),
        socket({ remoteScope: "loopback" }),
        socket({ remoteScope: "public", geo: null }),
        socket({ remoteScope: "public", geo: { lat: 1, lon: 1, city: null, region: null, country: null, countryCode: null } }),
        socket({ state: "listen", remoteAddr: null, remotePort: null, remoteScope: null }),
      ]),
    ).toEqual({ local: 2, unlocated: 1 });
  });
});
