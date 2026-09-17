import { describe, expect, it } from "vitest";

import type { GeoLocation } from "../../bindings/GeoLocation";
import type { SocketEntry } from "../../bindings/SocketEntry";
import { clusterEndpoints, markerRadius } from "./geo";

const geo = (lat: number, lon: number, city: string): GeoLocation => ({ lat, lon, city, region: null, country: "US", countryCode: "US" });

const s = (id: string, g: GeoLocation | null, over: Partial<SocketEntry> = {}): SocketEntry => ({
  id,
  protocol: "tcp",
  family: "v4",
  localAddr: "10.0.0.2",
  localPort: 5000,
  remoteAddr: `1.1.1.${id}`,
  remotePort: 443,
  remoteScope: "public",
  state: "established",
  pid: 1,
  processName: "x",
  remoteHost: null,
  geo: g,
  bytesIn: null,
  bytesOut: null,
  rxBps: 10,
  txBps: 5,
  firstSeenMs: 0,
  ...over,
});

const identity = ([lon, lat]: [number, number]): [number, number] => [lon, lat];

describe("clusterEndpoints", () => {
  it("pools identical locations and merges nearby pools into the larger", () => {
    const clusters = clusterEndpoints(
      [
        s("1", geo(37.77, -122.42, "San Francisco")),
        s("2", geo(37.77, -122.42, "San Francisco")),
        s("3", geo(37.8, -122.27, "Oakland")),
        s("4", geo(51.5, -0.12, "London")),
      ],
      identity,
      1,
    );
    expect(clusters).toHaveLength(2);
    expect(clusters[0]!.sockets).toHaveLength(3);
    expect(clusters[0]!.label).toBe("San Francisco, US and nearby");
    expect(clusters[0]!.bps).toBe(45);
    expect(clusters[0]!.endpoints).toBe(3);
    expect(clusters[0]!.id).toBe("37.77,-122.42");
    expect(clusters[0]!.keys).toEqual(["37.77,-122.42", "37.80,-122.27"]);
  });

  it("skips private, unlocated and unprojectable endpoints", () => {
    const clusters = clusterEndpoints(
      [s("1", null), s("2", geo(10, 10, "A"), { remoteScope: "private" }), s("3", geo(20, 20, "B"))],
      ([lon]) => (lon === 20 ? null : [0, 0]),
      5,
    );
    expect(clusters).toHaveLength(0);
  });
});

describe("markerRadius", () => {
  it("grows sublinearly and clamps", () => {
    expect(markerRadius(1)).toBe(3);
    expect(markerRadius(5)).toBeGreaterThan(markerRadius(2));
    expect(markerRadius(10_000)).toBe(14);
  });
});
