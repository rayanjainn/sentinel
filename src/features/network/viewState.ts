import { create } from "zustand";

import type { ProtocolFilter, StateFilter } from "./grouping";

export type NetworkTab = "connections" | "listening" | "firewall";

export interface ClusterSelection {
  id: string;
  label: string;
  keys: string[];
}

interface NetworkViewState {
  tab: NetworkTab;
  query: string;
  protocol: ProtocolFilter;
  state: StateFilter;
  pid: number | null;
  /** Map location the table is narrowed to. */
  cluster: ClusterSelection | null;
  selectedSocketId: string | null;
  expandedGroups: Set<string>;
  setTab: (tab: NetworkTab) => void;
  setQuery: (query: string) => void;
  setProtocol: (protocol: ProtocolFilter) => void;
  setState: (state: StateFilter) => void;
  setPid: (pid: number | null) => void;
  selectCluster: (cluster: ClusterSelection | null) => void;
  selectSocket: (id: string | null) => void;
  toggleGroup: (key: string) => void;
  clearFilters: () => void;
}

export const useNetworkView = create<NetworkViewState>((set) => ({
  tab: "connections",
  query: "",
  protocol: "all",
  state: "all",
  pid: null,
  cluster: null,
  selectedSocketId: null,
  expandedGroups: new Set(),
  setTab: (tab) => set({ tab }),
  setQuery: (query) => set({ query }),
  setProtocol: (protocol) => set({ protocol }),
  setState: (state) => set({ state }),
  setPid: (pid) => set({ pid }),
  selectCluster: (cluster) => set({ cluster, selectedSocketId: null }),
  selectSocket: (selectedSocketId) => set({ selectedSocketId }),
  toggleGroup: (key) =>
    set((s) => {
      const expandedGroups = new Set(s.expandedGroups);
      if (expandedGroups.has(key)) expandedGroups.delete(key);
      else expandedGroups.add(key);
      return { expandedGroups };
    }),
  clearFilters: () => set({ query: "", protocol: "all", state: "all", pid: null, cluster: null }),
}));
