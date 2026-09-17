// Storage scans: volumes, scan lifecycle with streamed progress and partial trees, lazily fetched
// tree slices, summary, duplicate detection, and paths removed since the scan.
import { homeDir } from "@tauri-apps/api/path";
import { create } from "zustand";

import type { DuplicateProgress } from "../bindings/DuplicateProgress";
import type { DuplicateReport } from "../bindings/DuplicateReport";
import type { ScanProgress } from "../bindings/ScanProgress";
import type { ScanSummary } from "../bindings/ScanSummary";
import type { TreeNode } from "../bindings/TreeNode";
import type { VolumeInfo } from "../bindings/VolumeInfo";
import { subscribe } from "../lib/events";
import { api } from "../lib/ipc";

type Status = "idle" | "loading" | "ready" | "error";

export const SLICE_DEPTH = 2;
export const SLICE_MAX_CHILDREN = 60;
const ROOT_KEY = "root";

export type SliceKey = number | typeof ROOT_KEY;
export const sliceKey = (nodeId: number | null): SliceKey => nodeId ?? ROOT_KEY;

interface StorageState {
  volumes: VolumeInfo[];
  volumesStatus: Status;
  volumesError: unknown;
  home: string | null;

  scanId: string | null;
  scanRoot: string | null;
  startError: unknown;
  progress: ScanProgress | null;
  partial: TreeNode | null;
  summary: ScanSummary | null;
  summaryError: unknown;

  slices: Map<SliceKey, TreeNode>;
  sliceErrors: Map<SliceKey, unknown>;

  /** Paths trashed or moved since the scan; hidden everywhere until the next scan. */
  removed: Set<string>;

  dupJobId: string | null;
  dupProgress: DuplicateProgress | null;
  dupReport: DuplicateReport | null;
  dupError: unknown;

  loadVolumes: () => Promise<void>;
  startScan: (root: string, crossMounts: boolean) => Promise<void>;
  cancelScan: () => Promise<void>;
  fetchSlice: (nodeId: number | null) => Promise<TreeNode>;
  loadSummary: () => Promise<void>;
  markRemoved: (paths: string[]) => void;
  startDuplicates: (minSizeBytes: number) => Promise<void>;
  cancelDuplicates: () => Promise<void>;
}

const inflight = new Map<SliceKey, Promise<TreeNode>>();

export const useStorage = create<StorageState>((set, get) => ({
  volumes: [],
  volumesStatus: "idle",
  volumesError: null,
  home: null,
  scanId: null,
  scanRoot: null,
  startError: null,
  progress: null,
  partial: null,
  summary: null,
  summaryError: null,
  slices: new Map(),
  sliceErrors: new Map(),
  removed: new Set(),
  dupJobId: null,
  dupProgress: null,
  dupReport: null,
  dupError: null,

  loadVolumes: async () => {
    set({ volumesStatus: get().volumes.length ? "ready" : "loading" });
    const [volumes, home] = await Promise.allSettled([api.listVolumes(), get().home ? Promise.resolve(get().home!) : homeDir()]);
    set({
      volumes: volumes.status === "fulfilled" ? volumes.value : get().volumes,
      volumesStatus: volumes.status === "fulfilled" ? "ready" : "error",
      volumesError: volumes.status === "rejected" ? volumes.reason : null,
      home: home.status === "fulfilled" ? home.value : get().home,
    });
  },

  startScan: async (root, crossMounts) => {
    inflight.clear();
    set({
      startError: null,
      scanRoot: root,
      progress: null,
      partial: null,
      summary: null,
      summaryError: null,
      slices: new Map(),
      sliceErrors: new Map(),
      removed: new Set(),
      dupJobId: null,
      dupProgress: null,
      dupReport: null,
      dupError: null,
    });
    try {
      const scanId = await api.startScan({ root, crossMounts });
      set({ scanId });
    } catch (startError) {
      set({ startError, scanId: null });
    }
  },

  cancelScan: async () => {
    const { scanId } = get();
    if (scanId) await api.cancelScan(scanId);
  },

  fetchSlice: (nodeId) => {
    const key = sliceKey(nodeId);
    const cached = get().slices.get(key);
    if (cached) return Promise.resolve(cached);
    const existing = inflight.get(key);
    if (existing) return existing;
    const { scanId } = get();
    if (!scanId) return Promise.reject(new Error("No scan is loaded"));
    const promise = api
      .getScanTree({ scanId, nodeId, depth: SLICE_DEPTH, maxChildren: SLICE_MAX_CHILDREN })
      .then((tree) => {
        if (get().scanId !== scanId) return tree;
        const slices = new Map(get().slices);
        slices.set(key, tree);
        const sliceErrors = new Map(get().sliceErrors);
        sliceErrors.delete(key);
        set({ slices, sliceErrors });
        return tree;
      })
      .catch((error: unknown) => {
        if (get().scanId === scanId) {
          const sliceErrors = new Map(get().sliceErrors);
          sliceErrors.set(key, error);
          set({ sliceErrors });
        }
        throw error;
      })
      .finally(() => inflight.delete(key));
    inflight.set(key, promise);
    return promise;
  },

  loadSummary: async () => {
    const { scanId } = get();
    if (!scanId) return;
    try {
      set({ summary: await api.getScanSummary(scanId), summaryError: null });
    } catch (summaryError) {
      set({ summaryError });
    }
  },

  markRemoved: (paths) => {
    const removed = new Set(get().removed);
    for (const p of paths) removed.add(p);
    set({ removed });
  },

  startDuplicates: async (minSizeBytes) => {
    const { scanId } = get();
    if (!scanId) return;
    set({ dupError: null, dupReport: null, dupProgress: null });
    try {
      const dupJobId = await api.startDuplicateScan(scanId, minSizeBytes);
      set({ dupJobId });
    } catch (dupError) {
      set({ dupError, dupJobId: null });
    }
  },

  cancelDuplicates: async () => {
    const { dupJobId } = get();
    if (dupJobId) await api.cancelDuplicateScan(dupJobId);
  },
}));

let started = false;

/** Scan and duplicate events are app-wide so a scan keeps streaming while other views are open. */
export function ensureStorageEvents(): void {
  if (started) return;
  started = true;
  subscribe("sentinel:scan-progress", (progress) => {
    if (progress.scanId !== useStorage.getState().scanId) return;
    useStorage.setState({ progress });
    if (progress.phase === "complete" && !useStorage.getState().summary) void useStorage.getState().loadSummary();
  });
  subscribe("sentinel:scan-partial", (partial) => {
    if (partial.scanId === useStorage.getState().scanId) useStorage.setState({ partial: partial.root });
  });
  subscribe("sentinel:scan-complete", (summary) => {
    if (summary.scanId === useStorage.getState().scanId) useStorage.setState({ summary, summaryError: null });
  });
  subscribe("sentinel:duplicate-progress", (dupProgress) => {
    if (dupProgress.jobId === useStorage.getState().dupJobId) useStorage.setState({ dupProgress });
  });
  subscribe("sentinel:duplicate-complete", (dupReport) => {
    if (dupReport.jobId === useStorage.getState().dupJobId) useStorage.setState({ dupReport });
  });
}

export function isScanning(progress: ScanProgress | null, scanId: string | null): boolean {
  if (!scanId) return false;
  return !progress || progress.phase === "walking" || progress.phase === "summarizing";
}
