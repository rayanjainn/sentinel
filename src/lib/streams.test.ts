import { describe, expect, it, vi } from "vitest";

import type { SamplingConfig } from "../bindings/SamplingConfig";
import { StreamRegistry } from "./streams";

function setup() {
  const sent: SamplingConfig[] = [];
  const queue: Array<() => void> = [];
  const send = vi.fn(async (c: SamplingConfig) => {
    sent.push(c);
    return c;
  });
  const registry = new StreamRegistry(send, 1000, (fn) => queue.push(fn));
  const drain = () => {
    while (queue.length) queue.shift()?.();
  };
  return { registry, sent, drain, send };
}

describe("StreamRegistry", () => {
  it("always includes resources", () => {
    const { registry } = setup();
    expect(registry.streams()).toEqual(["resources"]);
  });

  it("sends the union of acquired streams, batched", () => {
    const { registry, sent, drain } = setup();
    registry.acquire("network");
    registry.acquire("processes");
    drain();
    expect(sent).toEqual([{ intervalMs: 1000, streams: ["resources", "processes", "network"] }]);
  });

  it("ref-counts releases", () => {
    const { registry, drain } = setup();
    const a = registry.acquire("processes");
    const b = registry.acquire("processes");
    drain();
    a();
    drain();
    expect(registry.streams()).toContain("processes");
    b();
    drain();
    expect(registry.streams()).toEqual(["resources"]);
  });

  it("release is idempotent", () => {
    const { registry, drain } = setup();
    const a = registry.acquire("processes");
    registry.acquire("processes");
    a();
    a();
    drain();
    expect(registry.streams()).toContain("processes");
  });

  it("does not resend an unchanged config", () => {
    const { registry, sent, drain } = setup();
    const release = registry.acquire("network");
    drain();
    release();
    registry.acquire("network");
    drain();
    expect(sent).toHaveLength(1);
  });

  it("resends when the interval changes", () => {
    const { registry, sent, drain } = setup();
    drain();
    registry.setInterval(500);
    drain();
    expect(sent.at(-1)).toEqual({ intervalMs: 500, streams: ["resources"] });
  });

  it("retries after a failed send", async () => {
    const queue: Array<() => void> = [];
    const send = vi.fn().mockRejectedValueOnce(new Error("down")).mockResolvedValue({});
    const registry = new StreamRegistry(send, 1000, (fn) => queue.push(fn));
    const errors: unknown[] = [];
    registry.onError((e) => errors.push(e));
    registry.invalidate();
    queue.shift()?.();
    await Promise.resolve();
    await Promise.resolve();
    expect(errors).toHaveLength(1);
    registry.invalidate();
    queue.shift()?.();
    expect(send).toHaveBeenCalledTimes(2);
  });
});
