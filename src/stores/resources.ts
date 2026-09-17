// Machine-wide resource samples: live `sentinel:resources` events plus 15 minutes of backfill.
import { create } from "zustand";

import type { ResourceSample } from "../bindings/ResourceSample";
import type { SystemInfo } from "../bindings/SystemInfo";
import { subscribe } from "../lib/events";
import { api } from "../lib/ipc";
import { mergeSeries } from "../lib/series";

export const HISTORY_WINDOW_SECS = 900;
const MAX_AGE_MS = HISTORY_WINDOW_SECS * 1000 + 5000;

type LoadState = "loading" | "ready" | "error";

interface ResourcesState {
  samples: ResourceSample[];
  latest: ResourceSample | null;
  historyState: LoadState;
  historyError: unknown;
  systemInfo: SystemInfo | null;
  systemInfoError: unknown;
  /** Epoch ms when the last live event arrived (not the sample's own timestamp). */
  lastEventAt: number | null;
  loadHistory: () => Promise<void>;
  loadSystemInfo: () => Promise<void>;
}

export const useResources = create<ResourcesState>((set, get) => ({
  samples: [],
  latest: null,
  historyState: "loading",
  historyError: null,
  systemInfo: null,
  systemInfoError: null,
  lastEventAt: null,
  loadHistory: async () => {
    set({ historyState: "loading", historyError: null });
    try {
      const history = await api.getResourceHistory(HISTORY_WINDOW_SECS);
      const samples = mergeSeries(get().samples, history, MAX_AGE_MS);
      set({ samples, latest: samples[samples.length - 1] ?? null, historyState: "ready" });
    } catch (error) {
      set({ historyState: get().samples.length > 0 ? "ready" : "error", historyError: error });
    }
  },
  loadSystemInfo: async () => {
    try {
      set({ systemInfo: await api.getSystemInfo(), systemInfoError: null });
    } catch (error) {
      set({ systemInfoError: error });
    }
  },
}));

export function startResources(): () => void {
  const state = useResources.getState();
  void state.loadHistory();
  void state.loadSystemInfo();
  return subscribe("sentinel:resources", (sample) => {
    const { samples } = useResources.getState();
    const next = mergeSeries(samples, [sample], MAX_AGE_MS);
    useResources.setState({
      samples: next,
      latest: next[next.length - 1] ?? sample,
      lastEventAt: Date.now(),
      historyState: "ready",
    });
  });
}
