// App-wide stream registry bound to the backend and the sampling interval setting.
import { useEffect } from "react";
import { create } from "zustand";

import type { SamplingConfig } from "../bindings/SamplingConfig";
import type { StreamKind } from "../bindings/StreamKind";
import { useSettings } from "../stores/settings";
import { api } from "./ipc";
import { StreamRegistry } from "./streams";

interface SamplingStatus {
  applied: SamplingConfig | null;
  error: unknown;
}

export const useSamplingStatus = create<SamplingStatus>(() => ({ applied: null, error: null }));

export const streamRegistry = new StreamRegistry(
  (config) => api.setSampling(config),
  useSettings.getState().intervalMs,
);

streamRegistry.onApplied((applied) => useSamplingStatus.setState({ applied, error: null }));
streamRegistry.onError((error) => useSamplingStatus.setState({ error }));

useSettings.subscribe((state, prev) => {
  if (state.intervalMs !== prev.intervalMs) streamRegistry.setInterval(state.intervalMs);
});

/** Keeps `kind` sampled while the calling component is mounted and `active` is true. */
export function useStream(kind: StreamKind, active = true): void {
  useEffect(() => (active ? streamRegistry.acquire(kind) : undefined), [kind, active]);
}

/** Sends the initial configuration (resources only) once the app mounts. */
export function startSampling(): void {
  streamRegistry.invalidate();
}
