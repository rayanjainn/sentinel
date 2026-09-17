// Processes view UI state. Survives view switches; mode and sort persist across launches.
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import type { ProcessIdentity } from "../../bindings/ProcessIdentity";
import { COLUMNS, type SortDir, type SortKey, type StatusFilter } from "./columns";

export type ProcessMode = "list" | "tree";

interface ProcessViewState {
  mode: ProcessMode;
  sortKey: SortKey;
  sortDir: SortDir;
  query: string;
  status: StatusFilter;
  user: string | null;
  collapsed: Set<string>;
  selected: ProcessIdentity | null;
  drawerOpen: boolean;
  setMode: (mode: ProcessMode) => void;
  toggleSort: (key: SortKey) => void;
  setQuery: (query: string) => void;
  setStatus: (status: StatusFilter) => void;
  setUser: (user: string | null) => void;
  toggleCollapsed: (key: string) => void;
  setCollapsedKeys: (keys: string[]) => void;
  select: (identity: ProcessIdentity | null, openDrawer?: boolean) => void;
  closeDrawer: () => void;
}

export const useProcessView = create<ProcessViewState>()(
  persist(
    (set) => ({
      mode: "list",
      sortKey: "cpu",
      sortDir: "desc",
      query: "",
      status: "all",
      user: null,
      collapsed: new Set(),
      selected: null,
      drawerOpen: false,
      setMode: (mode) => set({ mode }),
      toggleSort: (key) =>
        set((s) =>
          s.sortKey === key
            ? { sortDir: s.sortDir === "asc" ? "desc" : "asc" }
            : { sortKey: key, sortDir: COLUMNS.find((c) => c.key === key)?.defaultDir ?? "desc" },
        ),
      setQuery: (query) => set({ query }),
      setStatus: (status) => set({ status }),
      setUser: (user) => set({ user }),
      toggleCollapsed: (key) =>
        set((s) => {
          const collapsed = new Set(s.collapsed);
          if (collapsed.has(key)) collapsed.delete(key);
          else collapsed.add(key);
          return { collapsed };
        }),
      setCollapsedKeys: (keys) => set({ collapsed: new Set(keys) }),
      select: (selected, openDrawer) => set((s) => ({ selected, drawerOpen: openDrawer ?? s.drawerOpen })),
      closeDrawer: () => set({ drawerOpen: false }),
    }),
    {
      name: "sentinel.processes",
      version: 1,
      storage: createJSONStorage(() => localStorage),
      partialize: (s) => ({ mode: s.mode, sortKey: s.sortKey, sortDir: s.sortDir }),
    },
  ),
);
