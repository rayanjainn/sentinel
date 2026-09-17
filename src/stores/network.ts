// Network sockets, reverse DNS, geolocation database, home location and Sentinel firewall rules.
import { useEffect } from "react";
import { create } from "zustand";

import type { FirewallRule } from "../bindings/FirewallRule";
import type { FirewallStatus } from "../bindings/FirewallStatus";
import type { GeoDbStatus } from "../bindings/GeoDbStatus";
import type { HomeLocation } from "../bindings/HomeLocation";
import type { HomeLocationInput } from "../bindings/HomeLocationInput";
import type { NetworkSnapshot } from "../bindings/NetworkSnapshot";
import type { SocketEntry } from "../bindings/SocketEntry";
import type { TrafficSource } from "../bindings/TrafficSource";
import { subscribe } from "../lib/events";
import { api } from "../lib/ipc";
import { useStream } from "../lib/sampling";

type Status = "idle" | "loading" | "ready" | "error";

interface NetworkState {
  sockets: SocketEntry[];
  tsMs: number;
  trafficSource: TrafficSource | null;
  status: Status;
  error: unknown;
  /** Reverse-DNS results that arrived after the socket was last snapshotted. */
  hostnames: Map<string, string | null>;

  geo: GeoDbStatus | null;
  geoError: unknown;
  downloadError: unknown;

  home: HomeLocation | null;
  homeError: unknown;

  rules: FirewallRule[];
  rulesStatus: Status;
  rulesError: unknown;
  firewall: FirewallStatus | null;
  firewallError: unknown;

  load: () => Promise<void>;
  apply: (snapshot: NetworkSnapshot) => void;
  loadGeo: () => Promise<void>;
  downloadGeo: () => Promise<void>;
  loadHome: () => Promise<void>;
  setHome: (input: HomeLocationInput | null) => Promise<void>;
  loadFirewall: () => Promise<void>;
}

export const useNetwork = create<NetworkState>((set, get) => ({
  sockets: [],
  tsMs: 0,
  trafficSource: null,
  status: "idle",
  error: null,
  hostnames: new Map(),
  geo: null,
  geoError: null,
  downloadError: null,
  home: null,
  homeError: null,
  rules: [],
  rulesStatus: "idle",
  rulesError: null,
  firewall: null,
  firewallError: null,

  load: async () => {
    if (get().status !== "ready") set({ status: "loading" });
    try {
      get().apply(await api.getNetworkSnapshot());
    } catch (error) {
      set({ status: get().sockets.length > 0 ? "ready" : "error", error });
    }
  },
  apply: (snapshot) => {
    if (snapshot.tsMs < get().tsMs) return;
    set({
      sockets: snapshot.sockets,
      tsMs: snapshot.tsMs,
      trafficSource: snapshot.trafficSource,
      status: "ready",
      error: null,
    });
  },
  loadGeo: async () => {
    try {
      set({ geo: await api.getGeoDbStatus(), geoError: null });
    } catch (geoError) {
      set({ geoError });
    }
  },
  downloadGeo: async () => {
    set({ downloadError: null, geo: { state: "downloading", downloadedBytes: 0, totalBytes: null } });
    try {
      await api.downloadGeoDb();
      await get().loadGeo();
    } catch (downloadError) {
      set({ downloadError });
      await get().loadGeo();
    }
  },
  loadHome: async () => {
    try {
      set({ home: await api.getHomeLocation(), homeError: null });
    } catch (homeError) {
      set({ homeError });
    }
  },
  setHome: async (input) => {
    const home = await api.setHomeLocation(input);
    set({ home, homeError: null });
  },
  loadFirewall: async () => {
    if (get().rulesStatus !== "ready") set({ rulesStatus: "loading" });
    const [rules, status] = await Promise.allSettled([api.listFirewallRules(), api.getFirewallStatus()]);
    set({
      rules: rules.status === "fulfilled" ? rules.value : get().rules,
      rulesStatus: rules.status === "fulfilled" ? "ready" : "error",
      rulesError: rules.status === "rejected" ? rules.reason : null,
      firewall: status.status === "fulfilled" ? status.value : get().firewall,
      firewallError: status.status === "rejected" ? status.reason : null,
    });
  },
}));

/** How long a new connection shows "Resolving…" before an absent name reads as "No hostname". */
export const RESOLVE_GRACE_MS = 15_000;

export type HostState = { state: "resolved"; host: string } | { state: "pending" } | { state: "none" };

/**
 * Distinguishes a reverse-DNS lookup still in flight from one that finished without a name, so
 * addresses without a PTR record do not show "Resolving…" forever.
 */
export function hostState(socket: SocketEntry, hostnames: Map<string, string | null>, nowMs: number): HostState {
  const host = hostFor(socket, hostnames);
  if (host) return { state: "resolved", host };
  if (!socket.remoteAddr) return { state: "none" };
  if (hostnames.has(socket.remoteAddr)) return { state: "none" };
  return nowMs - socket.firstSeenMs < RESOLVE_GRACE_MS ? { state: "pending" } : { state: "none" };
}

/** Resolved hostname for an IP, preferring live reverse-DNS events over the last snapshot. */
export function hostFor(socket: SocketEntry, hostnames: Map<string, string | null>): string | null {
  if (socket.remoteHost) return socket.remoteHost;
  if (!socket.remoteAddr) return null;
  return hostnames.get(socket.remoteAddr) ?? null;
}

let consumers = 0;
let stop: (() => void) | null = null;

/** Keeps the network stream sampled and its events applied while a consumer is mounted. */
export function useNetworkFeed(active = true): void {
  useStream("network", active);
  useEffect(() => {
    if (!active) return;
    consumers += 1;
    if (!stop) {
      const offSnapshot = subscribe("sentinel:network", (s) => useNetwork.getState().apply(s));
      const offHost = subscribe("sentinel:host-resolved", ({ ip, hostname }) => {
        const hostnames = new Map(useNetwork.getState().hostnames);
        hostnames.set(ip, hostname);
        if (hostnames.size > 4096) hostnames.delete(hostnames.keys().next().value as string);
        useNetwork.setState({ hostnames });
      });
      stop = () => {
        offSnapshot();
        offHost();
      };
    }
    const { status, tsMs, load } = useNetwork.getState();
    if (status !== "ready" || Date.now() - tsMs > 3000) void load();
    return () => {
      consumers -= 1;
      if (consumers === 0 && stop) {
        stop();
        stop = null;
      }
    };
  }, [active]);
}

let geoWatch: (() => void) | null = null;

/** Geolocation database status plus download progress events; safe to call from many views. */
export function useGeoStatus(): void {
  useEffect(() => {
    const { geo, loadGeo, home, loadHome } = useNetwork.getState();
    if (!geo) void loadGeo();
    if (!home) void loadHome();
    if (!geoWatch) geoWatch = subscribe("sentinel:geo-db", (geo) => useNetwork.setState({ geo, geoError: null }));
  }, []);
}
