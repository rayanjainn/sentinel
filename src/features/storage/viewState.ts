import { create } from "zustand";

export type StorageTab = "overview" | "largest" | "suggestions" | "duplicates";

export interface Crumb {
  /** null = scan root. */
  id: number | null;
  name: string;
  path: string;
}

interface StorageViewState {
  tab: StorageTab;
  crossMounts: boolean;
  crumbs: Crumb[];
  largestSelected: Set<string>;
  suggestionSelected: Set<string>;
  /** null = the default selection (every copy except the oldest). */
  duplicateSelected: Set<string> | null;
  duplicateMinBytes: number;
  setTab: (tab: StorageTab) => void;
  setCrossMounts: (crossMounts: boolean) => void;
  resetCrumbs: (root: Crumb) => void;
  pushCrumbs: (crumbs: Crumb[]) => void;
  popTo: (index: number) => void;
  setLargestSelected: (s: Set<string>) => void;
  setSuggestionSelected: (s: Set<string>) => void;
  setDuplicateSelected: (s: Set<string> | null) => void;
  setDuplicateMinBytes: (n: number) => void;
  clearSelections: () => void;
}

export const useStorageView = create<StorageViewState>((set) => ({
  tab: "overview",
  crossMounts: false,
  crumbs: [],
  largestSelected: new Set(),
  suggestionSelected: new Set(),
  duplicateSelected: null,
  duplicateMinBytes: 1_000_000,
  setTab: (tab) => set({ tab }),
  setCrossMounts: (crossMounts) => set({ crossMounts }),
  resetCrumbs: (root) => set({ crumbs: [root] }),
  pushCrumbs: (crumbs) => set((s) => ({ crumbs: [...s.crumbs, ...crumbs] })),
  popTo: (index) => set((s) => ({ crumbs: s.crumbs.slice(0, index + 1) })),
  setLargestSelected: (largestSelected) => set({ largestSelected }),
  setSuggestionSelected: (suggestionSelected) => set({ suggestionSelected }),
  setDuplicateSelected: (duplicateSelected) => set({ duplicateSelected }),
  setDuplicateMinBytes: (duplicateMinBytes) => set({ duplicateMinBytes }),
  clearSelections: () => set({ largestSelected: new Set(), suggestionSelected: new Set(), duplicateSelected: null }),
}));
