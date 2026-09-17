import { describe, expect, it, vi } from "vitest";

vi.mock("../lib/ipc", () => ({ api: {}, on: vi.fn(() => Promise.resolve(() => undefined)) }));

import type { SocketEntry } from "../bindings/SocketEntry";
import { hostFor, hostState, RESOLVE_GRACE_MS } from "./network";

const socket = (over: Partial<SocketEntry> = {}): SocketEntry => ({
  id: "s",
  protocol: "tcp",
  family: "v4",
  localAddr: "10.0.0.2",
  localPort: 5000,
  remoteAddr: "160.79.104.10",
  remotePort: 443,
  remoteScope: "public",
  state: "established",
  pid: 1,
  processName: "x",
  appName: null,
  processStartTime: null,
  remoteHost: null,
  geo: null,
  bytesIn: null,
  bytesOut: null,
  rxBps: null,
  txBps: null,
  firstSeenMs: 1_000_000,
  explanation: { headline: "", detail: "", service: null, purpose: null, confidence: "unknown", encrypted: false },
  ...over,
});

describe("hostState", () => {
  const now = 1_000_000 + RESOLVE_GRACE_MS + 1;

  it("prefers the snapshot hostname, then live events", () => {
    expect(hostState(socket({ remoteHost: "a.example" }), new Map(), now)).toEqual({ state: "resolved", host: "a.example" });
    expect(hostFor(socket(), new Map([["160.79.104.10", "b.example"]]))).toBe("b.example");
  });

  it("reports a finished lookup without a name as none", () => {
    expect(hostState(socket(), new Map([["160.79.104.10", null]]), 1_000_001)).toEqual({ state: "none" });
  });

  it("shows pending only within the grace period for new connections", () => {
    expect(hostState(socket(), new Map(), 1_000_500)).toEqual({ state: "pending" });
    expect(hostState(socket(), new Map(), now)).toEqual({ state: "none" });
    expect(hostState(socket({ remoteAddr: null }), new Map(), 1_000_500)).toEqual({ state: "none" });
  });
});
