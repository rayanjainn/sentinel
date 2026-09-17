import { DotsThree } from "@phosphor-icons/react";
import { useMemo, useRef } from "react";

import { IconButton } from "../../components/Button";
import { openContextMenu, openMenuAt } from "../../components/ContextMenu";
import { Checkbox } from "../../components/Field";
import { InfoTip } from "../../components/InfoTip";
import { EmptyState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatDateTime, formatRelativeSecs } from "../../lib/format";
import { useVirtualRows } from "../../lib/virtual";
import { useStorage } from "../../stores/storage";
import { FILE_KINDS, fileKindFill } from "../../styles/dataviz";
import { SelectionBar } from "./SelectionBar";
import { summarize, toggle } from "./selection";
import { moveItems, pathMenu, trashItems } from "./storageActions";
import { isHidden } from "./treemap";
import { useStorageView } from "./viewState";

const ROW = 40;
const HEAD = 30;
const GRID = "36px minmax(260px,1fr) 110px 120px 40px";

export function LargestFiles() {
  const summary = useStorage((s) => s.summary);
  const removed = useStorage((s) => s.removed);
  const selected = useStorageView((s) => s.largestSelected);
  const setSelected = useStorageView((s) => s.setLargestSelected);
  const scrollRef = useRef<HTMLDivElement>(null);
  const files = useMemo(() => (summary?.largestFiles ?? []).filter((f) => !isHidden(f.path, removed)), [summary, removed]);
  const { start, end, totalHeight } = useVirtualRows(scrollRef, files.length, ROW, { headerOffset: HEAD });
  const chosen = files.filter((f) => selected.has(f.path));
  const { count, bytes } = summarize(chosen);
  const allSelected = files.length > 0 && chosen.length === files.length;

  if (files.length === 0) {
    return (
      <StatePanel>
        <EmptyState title="No large files to show" detail="Files that were moved or trashed since the scan are hidden. Rescan to refresh." />
      </StatePanel>
    );
  }

  return (
    <div className="relative h-full">
      <div ref={scrollRef} role="grid" aria-label="Largest files" className="h-full overflow-auto pb-20">
        <div style={{ minWidth: 640 }}>
          <div role="row" className="sticky top-0 z-[5] grid items-center border-b border-line bg-ground text-[12px] text-fg-muted" style={{ gridTemplateColumns: GRID, height: HEAD }}>
            <span className="flex justify-center">
              <Checkbox
                label="Select all"
                checked={allSelected}
                indeterminate={chosen.length > 0 && !allSelected}
                onChange={(on) => setSelected(on ? new Set(files.map((f) => f.path)) : new Set())}
              />
            </span>
            <span>File</span>
            <span className="flex items-center justify-end gap-1.5 px-3 text-right">
              Size
              <InfoTip id="apparentSize" />
            </span>
            <span className="px-3">Modified</span>
            <span />
          </div>
          <div className="relative" style={{ height: totalHeight }}>
            {files.slice(start, end).map((f, i) => {
              const on = selected.has(f.path);
              return (
                <div
                  key={f.path}
                  role="row"
                  aria-selected={on}
                  className={cx("absolute left-0 right-0 grid items-center border-b border-line text-[12px]", on ? "bg-signal/10" : "hover:bg-raised/60")}
                  style={{ gridTemplateColumns: GRID, height: ROW, transform: `translateY(${(start + i) * ROW}px)` }}
                  onClick={() => setSelected(toggle(selected, f.path))}
                  onContextMenu={(e) => openContextMenu(e, pathMenu(f), f.name)}
                >
                  <span className="flex justify-center">
                    <Checkbox label={`Select ${f.name}`} checked={on} onChange={(v) => setSelected(toggle(selected, f.path, v))} />
                  </span>
                  <span className="flex min-w-0 flex-col">
                    <span className="flex items-center gap-2">
                      <span className="size-2 shrink-0 rounded-[2px]" style={{ background: fileKindFill(f.fileKind) }} title={FILE_KINDS[f.fileKind].label} />
                      <span className="truncate text-fg">{f.name}</span>
                    </span>
                    <span className="num truncate pl-4 text-[11px] text-fg-subtle" title={f.path}>
                      {f.path}
                    </span>
                  </span>
                  <span className="num px-3 text-right text-fg">{formatBytes(f.sizeBytes, { base: 1000 })}</span>
                  <span className="px-3 text-fg-muted" title={f.modified ? formatDateTime(f.modified * 1000) : undefined}>
                    {formatRelativeSecs(f.modified)}
                  </span>
                  <span className="flex justify-center">
                    <IconButton
                      label={`Actions for ${f.name}`}
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        openMenuAt(e.currentTarget, pathMenu(f), f.name);
                      }}
                    >
                      <DotsThree size={14} weight="bold" />
                    </IconButton>
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      </div>
      <SelectionBar
        count={count}
        bytes={bytes}
        onClear={() => setSelected(new Set())}
        onTrash={() => void trashItems(chosen).then((ok) => ok && setSelected(new Set()))}
        onMove={() => void moveItems(chosen).then((ok) => ok && setSelected(new Set()))}
      />
    </div>
  );
}
