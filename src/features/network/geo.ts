// Clusters geolocated endpoints in screen space so nearby connections share one map marker.
import type { SocketEntry } from "../../bindings/SocketEntry";

export interface EndpointCluster {
  id: string;
  /** Location keys (see `geoKey`) pooled into this cluster, for filtering the table by selection. */
  keys: string[];
  x: number;
  y: number;
  lat: number;
  lon: number;
  sockets: SocketEntry[];
  /** Distinct remote addresses. */
  endpoints: number;
  bps: number;
  label: string;
}

type Projector = (lonLat: [number, number]) => [number, number] | null;

export function geoKey(s: SocketEntry): string | null {
  return s.geo ? `${s.geo.lat.toFixed(2)},${s.geo.lon.toFixed(2)}` : null;
}

function placeLabel(s: SocketEntry): string {
  const g = s.geo;
  if (!g) return "Unknown location";
  return [g.city, g.country].filter(Boolean).join(", ") || "Unknown location";
}

/**
 * Greedy clustering: exact locations are pooled first, then pools within `radius` pixels merge
 * into the larger one. Cluster ids derive from the anchor location so they stay stable while
 * counts change, which keeps map animations and selection steady across snapshots.
 */
export function clusterEndpoints(sockets: SocketEntry[], project: Projector, radius: number): EndpointCluster[] {
  const pools = new Map<string, EndpointCluster>();
  for (const s of sockets) {
    if (!s.geo || s.remoteScope !== "public" || s.remoteAddr === null) continue;
    const key = geoKey(s)!;
    let pool = pools.get(key);
    if (!pool) {
      const p = project([s.geo.lon, s.geo.lat]);
      if (!p) continue;
      pool = { id: key, keys: [key], x: p[0], y: p[1], lat: s.geo.lat, lon: s.geo.lon, sockets: [], endpoints: 0, bps: 0, label: placeLabel(s) };
      pools.set(key, pool);
    }
    pool.sockets.push(s);
    pool.bps += (s.rxBps ?? 0) + (s.txBps ?? 0);
  }

  const ordered = [...pools.values()].sort((a, b) => b.sockets.length - a.sockets.length || a.id.localeCompare(b.id));
  const clusters: EndpointCluster[] = [];
  const r2 = radius * radius;
  for (const pool of ordered) {
    const target = clusters.find((c) => (c.x - pool.x) ** 2 + (c.y - pool.y) ** 2 <= r2);
    if (target) {
      target.sockets.push(...pool.sockets);
      target.keys.push(...pool.keys);
      target.bps += pool.bps;
      if (target.label !== pool.label && !target.label.endsWith("nearby")) target.label = `${target.label} and nearby`;
    } else {
      clusters.push({ ...pool, sockets: [...pool.sockets], keys: [...pool.keys] });
    }
  }
  for (const c of clusters) c.endpoints = new Set(c.sockets.map((s) => s.remoteAddr)).size;
  return clusters;
}

/** Marker radius grows with the square root of connections so area tracks count. */
export function markerRadius(count: number, min = 3, max = 14): number {
  return Math.min(max, min + Math.sqrt(Math.max(0, count - 1)) * 2.2);
}
