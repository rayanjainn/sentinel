// App-level preferences persisted in localStorage (theme, sampling interval, first-run flag, panel).
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type ThemePreference = "dark" | "light" | "system";
export type SamplingInterval = 500 | 1000 | 2000;
export type ViewId = "processes" | "network" | "resources" | "storage" | "activity" | "settings";

export const PANEL_MIN_WIDTH = 320;
export const PANEL_MAX_WIDTH = 640;

interface SettingsState {
  theme: ThemePreference;
  intervalMs: SamplingInterval;
  onboarded: boolean;
  panelOpen: boolean;
  panelWidth: number;
  view: ViewId;
  sidebarCollapsed: boolean;
  setTheme: (theme: ThemePreference) => void;
  setIntervalMs: (intervalMs: SamplingInterval) => void;
  completeOnboarding: () => void;
  reopenOnboarding: () => void;
  togglePanel: () => void;
  setPanelOpen: (open: boolean) => void;
  setPanelWidth: (width: number) => void;
  setView: (view: ViewId) => void;
}

export const clampPanelWidth = (width: number) =>
  Math.round(Math.min(PANEL_MAX_WIDTH, Math.max(PANEL_MIN_WIDTH, width)));

export const useSettings = create<SettingsState>()(
  persist(
    (set) => ({
      theme: "dark",
      intervalMs: 1000,
      onboarded: false,
      panelOpen: false,
      panelWidth: 400,
      view: "processes",
      sidebarCollapsed: false,
      setTheme: (theme) => set({ theme }),
      setIntervalMs: (intervalMs) => set({ intervalMs }),
      completeOnboarding: () => set({ onboarded: true }),
      reopenOnboarding: () => set({ onboarded: false }),
      togglePanel: () => set((s) => ({ panelOpen: !s.panelOpen })),
      setPanelOpen: (panelOpen) => set({ panelOpen }),
      setPanelWidth: (width) => set({ panelWidth: clampPanelWidth(width) }),
      setView: (view) => set({ view }),
    }),
    {
      name: "sentinel.settings",
      version: 1,
      storage: createJSONStorage(() => localStorage),
      partialize: (s) => ({
        theme: s.theme,
        intervalMs: s.intervalMs,
        onboarded: s.onboarded,
        panelOpen: s.panelOpen,
        panelWidth: s.panelWidth,
        view: s.view,
      }),
    },
  ),
);

export function resolveTheme(pref: ThemePreference, prefersDark: boolean): "dark" | "light" {
  if (pref === "system") return prefersDark ? "dark" : "light";
  return pref;
}
