// Permission probe results. Refreshed on demand and whenever the window regains focus (the user
// typically returns from System Settings), never on a timer.
import { create } from "zustand";

import type { PermissionStatus } from "../bindings/PermissionStatus";
import { api } from "../lib/ipc";

interface PermissionsState {
  status: PermissionStatus | null;
  error: unknown;
  loading: boolean;
  refresh: () => Promise<void>;
}

export const usePermissions = create<PermissionsState>((set) => ({
  status: null,
  error: null,
  loading: true,
  refresh: async () => {
    set({ loading: true });
    try {
      set({ status: await api.getPermissionStatus(), error: null, loading: false });
    } catch (error) {
      set({ error, loading: false });
    }
  },
}));

export function startPermissionWatch(): () => void {
  const refresh = () => void usePermissions.getState().refresh();
  refresh();
  const onVisible = () => {
    if (document.visibilityState === "visible") refresh();
  };
  window.addEventListener("focus", refresh);
  document.addEventListener("visibilitychange", onVisible);
  return () => {
    window.removeEventListener("focus", refresh);
    document.removeEventListener("visibilitychange", onVisible);
  };
}
