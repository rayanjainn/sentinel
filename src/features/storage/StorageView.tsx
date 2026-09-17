import { ArrowCounterClockwise, ArrowsClockwise } from "@phosphor-icons/react";
import { useEffect, useRef } from "react";

import { Button } from "../../components/Button";
import { Select } from "../../components/Field";
import { Segmented } from "../../components/Segmented";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState, StatePanel } from "../../components/States";
import { formatDateTime } from "../../lib/format";
import { useElementSize } from "../../lib/motion";
import { HeaderToolbar } from "../../shell/HeaderSlot";
import { ensureStorageEvents, isScanning, sliceKey, useStorage } from "../../stores/storage";
import { Duplicates } from "./Duplicates";
import { ByType, FolderContents } from "./FolderContents";
import { LargestFiles } from "./LargestFiles";
import { ScanBar } from "./ScanBar";
import { StartScreen } from "./StartScreen";
import { Suggestions } from "./Suggestions";
import { Breadcrumbs, TreemapStage, useTreemapNavigation } from "./TreemapView";
import { useStorageView, type StorageTab } from "./viewState";

function baseName(path: string): string {
  const parts = path.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function startScan(root: string, name: string) {
  const view = useStorageView.getState();
  view.resetCrumbs({ id: null, name, path: root });
  view.clearSelections();
  view.setTab("overview");
  void useStorage.getState().startScan(root, view.crossMounts);
}

function StorageToolbar() {
  const scanId = useStorage((s) => s.scanId);
  const scanRoot = useStorage((s) => s.scanRoot);
  const progress = useStorage((s) => s.progress);
  const summary = useStorage((s) => s.summary);
  const volumes = useStorage((s) => s.volumes);
  const home = useStorage((s) => s.home);
  const tab = useStorageView((s) => s.tab);
  const setTab = useStorageView((s) => s.setTab);
  const crossMounts = useStorageView((s) => s.crossMounts);
  const setCrossMounts = useStorageView((s) => s.setCrossMounts);
  const crumbs = useStorageView((s) => s.crumbs);
  const scanning = isScanning(progress, scanId);
  if (!scanId) return null;

  const suggestionCount = summary?.suggestions.length ?? 0;
  return (
    <div className="flex min-w-0 flex-1 items-center gap-2">
      <Segmented<StorageTab>
        label="Storage view"
        value={tab}
        onChange={setTab}
        options={[
          { value: "overview", label: "Overview" },
          { value: "largest", label: "Largest files" },
          { value: "suggestions", label: suggestionCount > 0 ? `Suggestions ${suggestionCount}` : "Suggestions" },
          { value: "duplicates", label: "Duplicates" },
        ]}
      />
      <span className="flex-1" />
      <Select
        label="Scan location"
        value={scanRoot ?? ""}
        disabled={scanning}
        onChange={(e) => {
          const root = e.target.value;
          const name = root === home ? "Home" : (volumes.find((v) => v.mountPoint === root)?.name ?? baseName(root));
          startScan(root, name);
        }}
        className="max-w-52"
      >
        {home && <option value={home}>Home folder</option>}
        {volumes.map((v) => (
          <option key={v.mountPoint} value={v.mountPoint}>
            {v.name || v.mountPoint}
          </option>
        ))}
        {scanRoot && scanRoot !== home && !volumes.some((v) => v.mountPoint === scanRoot) && <option value={scanRoot}>{scanRoot}</option>}
      </Select>
      <label className="flex items-center gap-1.5 whitespace-nowrap text-[12px] text-fg-muted" title="Descend into other volumes mounted inside the scanned folder">
        <input type="checkbox" checked={crossMounts} onChange={(e) => setCrossMounts(e.target.checked)} className="accent-[var(--signal)]" />
        Other volumes
      </label>
      <Button
        size="md"
        icon={<ArrowsClockwise size={14} />}
        disabled={scanning || !scanRoot}
        onClick={() => scanRoot && startScan(scanRoot, crumbs[0]?.name ?? baseName(scanRoot))}
      >
        Rescan
      </Button>
    </div>
  );
}

function Overview() {
  const summary = useStorage((s) => s.summary);
  const slices = useStorage((s) => s.slices);
  const removed = useStorage((s) => s.removed);
  const stageRef = useRef<HTMLDivElement>(null);
  const { width, height } = useElementSize(stageRef);
  const navigation = useTreemapNavigation(width, height);
  const { crumbs, provisional, zoomOutTo, zoomToNode } = navigation;
  const current = crumbs[crumbs.length - 1];
  const node = current ? (slices.get(sliceKey(current.id)) ?? (current.id !== null ? provisional.get(current.id) : undefined)) : undefined;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target && /input|select|textarea/i.test(target.tagName)) return;
      if ((event.key === "Escape" || event.key === "Backspace") && crumbs.length > 1) {
        event.preventDefault();
        void zoomOutTo(crumbs.length - 2);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [crumbs.length, zoomOutTo]);

  return (
    <div className="grid h-full grid-cols-[minmax(0,1fr)_320px]">
      <div className="flex min-h-0 flex-col">
        <div className="flex h-9 shrink-0 items-center gap-2 border-b border-line px-3">
          {summary ? <Breadcrumbs onNavigate={(i) => void zoomOutTo(i)} /> : <span className="text-[12px] text-fg-muted">The map fills in as folders are measured</span>}
          <span className="flex-1" />
          {summary && crumbs.length > 1 && (
            <Button size="sm" variant="ghost" icon={<ArrowCounterClockwise size={12} />} onClick={() => void zoomOutTo(crumbs.length - 2)}>
              Zoom out
            </Button>
          )}
        </div>
        <div ref={stageRef} className="relative min-h-0 flex-1 overflow-hidden bg-ground p-0">
          {width > 0 && height > 0 && <TreemapStage width={width} height={height} navigation={navigation} />}
        </div>
        {removed.size > 0 && summary && (
          <p className="shrink-0 border-t border-line px-4 py-1.5 text-[11px] text-fg-subtle">
            Items you moved or trashed are hidden. Sizes are from the scan finished {formatDateTime(summary.startedAtMs + summary.durationMs)}; rescan for current totals.
          </p>
        )}
      </div>
      <aside className="flex min-h-0 flex-col gap-5 overflow-y-auto border-l border-line py-3">
        {summary ? (
          <>
            <FolderContents node={node} hidden={removed} onZoom={zoomToNode} />
            <div className="border-t border-line pt-4">
              <ByType stats={summary.byExtension} />
            </div>
          </>
        ) : (
          <div className="flex flex-col gap-3 px-4">
            <Skeleton className="h-3 w-32" />
            {Array.from({ length: 8 }, (_, i) => (
              <Skeleton key={i} className="h-2.5" style={{ width: `${90 - i * 7}%` }} />
            ))}
          </div>
        )}
      </aside>
    </div>
  );
}

export function StorageView() {
  const scanId = useStorage((s) => s.scanId);
  const summary = useStorage((s) => s.summary);
  const summaryError = useStorage((s) => s.summaryError);
  const loadSummary = useStorage((s) => s.loadSummary);
  const slices = useStorage((s) => s.slices);
  const tab = useStorageView((s) => s.tab);
  const crumbs = useStorageView((s) => s.crumbs);

  useEffect(() => {
    ensureStorageEvents();
    void useStorage.getState().loadVolumes();
  }, []);

  useEffect(() => {
    if (!summary) return;
    if (useStorageView.getState().crumbs.length === 0) {
      useStorageView.getState().resetCrumbs({ id: null, name: baseName(summary.root), path: summary.root });
    }
    if (!slices.has(sliceKey(null))) void useStorage.getState().fetchSlice(null).catch(() => undefined);
  }, [summary, slices, crumbs.length]);

  return (
    <div className="flex h-full flex-col">
      <HeaderToolbar>
        <StorageToolbar />
      </HeaderToolbar>
      {!scanId ? (
        <StartScreen onScan={startScan} />
      ) : (
        <>
          <ScanBar />
          <div className="min-h-0 flex-1">
            {summaryError !== null && !summary && (
              <StatePanel>
                <ErrorState error={summaryError} subject="The scan summary" onRetry={() => void loadSummary()} />
              </StatePanel>
            )}
            {tab === "overview" && summaryError === null && <Overview />}
            {tab !== "overview" && !summary && summaryError === null && (
              <StatePanel>
                <p className="text-fg-muted">Available when the scan finishes.</p>
              </StatePanel>
            )}
            {tab === "largest" && summary && <LargestFiles />}
            {tab === "suggestions" && summary && <Suggestions />}
            {tab === "duplicates" && summary && <Duplicates />}
          </div>
        </>
      )}
    </div>
  );
}
