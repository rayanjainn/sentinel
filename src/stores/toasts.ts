import type { ReactNode } from "react";
import { create } from "zustand";

export type ToastKind = "success" | "error" | "info";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  detail?: ReactNode;
  action?: { label: string; onClick: () => void };
  durationMs: number;
}

interface ToastState {
  toasts: Toast[];
  push: (toast: Omit<Toast, "id" | "durationMs"> & { durationMs?: number }) => number;
  dismiss: (id: number) => void;
}

let nextId = 1;

export const useToasts = create<ToastState>((set) => ({
  toasts: [],
  push: (toast) => {
    const id = nextId++;
    const durationMs = toast.durationMs ?? (toast.kind === "error" ? 9000 : 5000);
    set((s) => ({ toasts: [...s.toasts.slice(-3), { ...toast, id, durationMs }] }));
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

export const toast = (t: Parameters<ToastState["push"]>[0]) => useToasts.getState().push(t);
