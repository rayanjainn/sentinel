import { Copy, MagnifyingGlass, Stop } from "@phosphor-icons/react";
import { useMemo, useRef } from "react";

import type { DuplicateGroup } from "../../bindings/DuplicateGroup";
import type { DuplicatePhase } from "../../bindings/DuplicatePhase";
import type { FileEntry } from "../../bindings/FileEntry";
import { Button } from "../../components/Button";
import { openContextMenu } from "../../components/ContextMenu";
import { Checkbox, Select } from "../../components/Field";
import { Skeleton } from "../../components/Skeleton";
import { EmptyState, ErrorState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatCount, formatElapsedMs, formatRelativeSecs, pluralize } from "../../lib/format";
import { useVirtualRows } from "../../lib/virtual";
import { useStorage } from "../../stores/storage";
import { SelectionBar } from "./SelectionBar";
import { defaultDuplicateSelection, keeperOf, keepsOneCopy, summarize, toggle } from "./selection";
import { moveItems, pathMenu, trashItems } from "./storageActions";
import { isHidden } from "./treemap";
import { useStorageView } from "./viewState";

const ROW = 30;

const PHASE_LABEL: Record<DuplicatePhase, string> = {
  groupingBySize: "Grouping files by size",
  hashingHeads: "Comparing the start of each candidate",
  hashingFull: "Comparing full contents",
  complete: "Complete",
  cancelled: "Cancelled",
  failed: "Failed",
};

type Row = { type: "group"; group: DuplicateGroup } | { type: "file"; file: FileEntry; group: DuplicateGroup; keeper: boolean };

export function Duplicates() {
  const summary = useStorage((s) => s.summary);
  const removed = useStorage((s) => s.removed);
  const jobId = useStorage((s) => s.dupJobId);
  const progress = useStorage((s) => s.dupProgress);
  const report = useStorage((s) => s.dupReport);
  const error = useStorage((s) => s.dupError);
  const start = useStorage((s) => s.startDuplicates);
  const cancel = useStorage((s) => s.cancelDuplicates);
  const minBytes = useStorageView((s) => s.duplicateMinBytes);
  const setMinBytes = useStorageView((s) => s.setDuplicateMinBytes);
  const manual = useStorageView((s) => s.duplicateSelected);
  const setManual = useStorageView((s) => s.setDuplicateSelected);
  const scrollRef = useRef<HTMLDivElement>(null);

  const groups = useMemo(
    () =>
      (report?.groups ?? [])
        .map((g) => ({ ...g, files: g.files.filter((f) => !isHidden(f.path, removed)) }))
        .filter((g) => g.files.length > 1)
        .sort((a, b) => b.reclaimableBytes - a.reclaimableBytes),
    [report, removed],
  );
  const selected = useMemo(() => manual ?? defaultDuplicateSelection(groups), [manual, groups]);
  const rows = useMemo<Row[]>(
    () =>
      groups.flatMap((group) => {
        const keep = keeperOf(group);
        return [{ type: "group" as const, group }, ...group.files.map((file) => ({ type: "file" as const, file, group, keeper: file.path === keep }))];
      }),
    [groups],
  );
  const { start: first, end, totalHeight } = useVirtualRows(scrollRef, rows.length, ROW);

  const running = jobId !== null && !report && progress?.phase !== "failed" && progress?.phase !== "cancelled";
  const chosen = groups.flatMap((g) => g.files).filter((f) => selected.has(f.path));
  const { count, bytes } = summarize(chosen);
  const unsafe = groups.filter((g) => !keepsOneCopy(g, selected)).length;

  return (
    <div className="relative flex h-full flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-3 border-b border-line px-5 py-3">
        <span className="text-fg-muted">Compare files larger than</span>
        <Select label="Minimum size" value={minBytes} onChange={(e) => setMinBytes(Number(e.target.value))} disabled={running}>
          <option value={100_000}>100 KB</option>
          <option value={1_000_000}>1 MB</option>
          <option value={10_000_000}>10 MB</option>
          <option value={100_000_000}>100 MB</option>
        </Select>
        {running ? (
          <Button size="md" variant="ghost" icon={<Stop size={12} weight="fill" />} onClick={() => void cancel()}>
            Stop
          </Button>
        ) : (
          <Button
            variant={report ? "secondary" : "primary"}
            icon={<MagnifyingGlass size={14} />}
            disabled={!summary}
            onClick={() => {
              setManual(null);
              void start(minBytes);
            }}
          >
            {report ? "Search again" : "Find duplicates"}
          </Button>
        )}
        <span className="text-[12px] text-fg-subtle">Files are compared by content hash, only within this scan.</span>
      </div>

      {running && (
        <div className="flex shrink-0 flex-col gap-2 border-b border-line px-5 py-3" role="status" aria-live="polite">
          <div className="flex items-center gap-4 text-[12px]">
            <span className="font-medium text-fg">{progress ? PHASE_LABEL[progress.phase] : "Starting"}</span>
            {progress && (
              <>
                <span className="text-fg-muted">
                  <span className="num text-fg">{formatCount(progress.hashed)}</span> of <span className="num">{formatCount(progress.candidates)}</span> candidates
                </span>
                <span className="num text-fg-muted">{formatBytes(progress.bytesHashed, { base: 1000 })} read</span>
              </>
            )}
          </div>
          <div className="relative h-1 overflow-hidden rounded-full bg-[var(--viz-track)]">
            {progress && progress.candidates > 0 ? (
              <div className="absolute inset-0 origin-left rounded-full bg-signal transition-transform duration-300" style={{ transform: `scaleX(${Math.min(1, progress.hashed / progress.candidates)})` }} />
            ) : (
              <div className="skeleton absolute inset-0" />
            )}
          </div>
        </div>
      )}

      <div className="min-h-0 flex-1">
        {error !== null && (
          <StatePanel>
            <ErrorState error={error} subject="The duplicate search" onRetry={() => void start(minBytes)} />
          </StatePanel>
        )}
        {progress?.phase === "failed" && progress.error && (
          <StatePanel>
            <p className="text-danger">{progress.error}</p>
          </StatePanel>
        )}
        {!jobId && error === null && (
          <StatePanel>
            <EmptyState
              icon={<Copy size={22} />}
              title="Find identical files"
              detail="Reads and hashes candidate files, so it takes longer than the scan. Start it when you are ready."
            />
          </StatePanel>
        )}
        {running && (
          <div className="flex flex-col gap-3 px-5 py-4">
            {Array.from({ length: 6 }, (_, i) => (
              <Skeleton key={i} className="h-3" style={{ width: `${85 - i * 8}%` }} />
            ))}
          </div>
        )}
        {report && groups.length === 0 && (
          <StatePanel>
            <EmptyState title="No duplicates found" detail={`No files larger than ${formatBytes(minBytes, { base: 1000 })} share identical contents.`} />
          </StatePanel>
        )}
        {report && groups.length > 0 && (
          <div className="flex h-full flex-col">
            <p className="shrink-0 px-5 py-2.5 text-[12px] text-fg-muted">
              {pluralize(groups.length, "group")} of identical files,{" "}
              <span className="num text-fg">{formatBytes(groups.reduce((s, g) => s + g.reclaimableBytes, 0), { base: 1000 })}</span> could be
              reclaimed. Searched in {formatElapsedMs(report.durationMs)}. The oldest copy of each file is kept by default.
            </p>
            <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto pb-24">
              <div className="relative" style={{ height: totalHeight }}>
                {rows.slice(first, end).map((row, i) => {
                  const top = (first + i) * ROW;
                  if (row.type === "group") {
                    const g = row.group;
                    return (
                      <div key={`g-${g.hash}`} className="absolute left-0 right-0 flex items-center gap-4 border-b border-line bg-ground px-5 text-[12px]" style={{ height: ROW, transform: `translateY(${top}px)` }}>
                        <span className="font-medium text-fg">{pluralize(g.files.length, "copy", "copies")}</span>
                        <span className="num text-fg-muted">{formatBytes(g.sizeBytes, { base: 1000 })} each</span>
                        <span className="flex-1" />
                        {!keepsOneCopy(g, selected) && <span className="text-warn">Every copy selected</span>}
                        <span className="text-fg-muted">
                          <span className="num text-fg">{formatBytes(g.reclaimableBytes, { base: 1000 })}</span> reclaimable
                        </span>
                      </div>
                    );
                  }
                  const on = selected.has(row.file.path);
                  return (
                    <div
                      key={row.file.path}
                      className={cx("absolute left-0 right-0 grid grid-cols-[20px_minmax(0,1fr)_110px_90px] items-center gap-3 pl-8 pr-5 text-[12px]", on ? "bg-signal/10" : "hover:bg-raised/60")}
                      style={{ height: ROW, transform: `translateY(${top}px)` }}
                      onClick={() => setManual(toggle(selected, row.file.path))}
                      onContextMenu={(e) => openContextMenu(e, pathMenu(row.file), row.file.name)}
                    >
                      <Checkbox label={`Select ${row.file.path}`} checked={on} onChange={(v) => setManual(toggle(selected, row.file.path, v))} />
                      <span className="num truncate text-fg" title={row.file.path}>
                        {row.file.path}
                      </span>
                      <span className="text-fg-muted">{formatRelativeSecs(row.file.modified)}</span>
                      <span className="text-right text-[11px] text-fg-subtle">{row.keeper ? "Oldest copy" : ""}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        )}
      </div>

      <SelectionBar
        count={count}
        bytes={bytes}
        disabled={unsafe > 0}
        warning={unsafe > 0 ? `Keep at least one copy in ${pluralize(unsafe, "group")}.` : undefined}
        onClear={() => setManual(new Set([" "]))}
        onTrash={() => void trashItems(chosen).then((ok) => ok && setManual(null))}
        onMove={() => void moveItems(chosen).then((ok) => ok && setManual(null))}
      />
    </div>
  );
}
