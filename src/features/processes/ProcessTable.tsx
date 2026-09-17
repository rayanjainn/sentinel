import { ArrowDown, ArrowUp, CaretRight, ListMagnifyingGlass } from "@phosphor-icons/react";
import { memo, useMemo, useRef, type KeyboardEvent } from "react";

import type { ProcessInfo } from "../../bindings/ProcessInfo";
import { Button } from "../../components/Button";
import { openContextMenu } from "../../components/ContextMenu";
import { SkeletonRows } from "../../components/Skeleton";
import { EmptyState, ErrorState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatDateTime, formatDuration, formatNice, formatPercent } from "../../lib/format";
import { useVirtualRows } from "../../lib/virtual";
import { useProcesses } from "../../stores/processes";
import { COLUMNS, comparator, GRID_TEMPLATE, queryMatcher, STATUS_LABEL, statusMatches } from "./columns";
import { intensityModel } from "./intensity";
import { processMenuItems } from "./processActions";
import { buildForest, flattenForest, processKey, type TreeRow } from "./tree";
import { useProcessView } from "./viewState";

const ROW_HEIGHT = 28;
const HEADER_HEIGHT = 30;
const INDENT = 16;
const MIN_WIDTH = 1080;

const STATUS_DOT: Record<ProcessInfo["status"], string> = {
  running: "bg-signal",
  sleeping: "bg-fg-subtle",
  idle: "bg-fg-subtle",
  waiting: "bg-fg-subtle",
  stopped: "bg-warn",
  zombie: "bg-danger",
  dead: "bg-danger",
  unknown: "bg-fg-subtle",
};

function startedLabel(startTime: number): string {
  const d = new Date(startTime * 1000);
  const now = new Date();
  if (d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

interface RowProps {
  process: ProcessInfo;
  rowKey: string;
  index: number;
  tree: boolean;
  depth: number;
  hasChildren: boolean;
  expanded: boolean;
  /** "1" per ancestor level whose vertical guide continues. */
  guides: string;
  isLast: boolean;
  descendants: number;
  score: number;
  selected: boolean;
}

const ProcessRow = memo(function ProcessRow({
  process: p,
  rowKey,
  index,
  tree,
  depth,
  hasChildren,
  expanded,
  guides,
  isLast,
  descendants,
  score,
  selected,
}: RowProps) {
  const tint = score > 0 ? `color-mix(in srgb, var(--danger) ${Math.round(score * 16)}%, transparent)` : undefined;
  const nameOffset = tree ? 10 + depth * INDENT + 18 : 12;

  return (
    <div
      role="row"
      aria-selected={selected}
      aria-rowindex={index + 2}
      aria-level={tree ? depth + 1 : undefined}
      aria-expanded={tree && hasChildren ? expanded : undefined}
      data-index={index}
      className={cx(
        "group absolute left-0 right-0 grid items-center text-[12px]",
        selected ? "bg-signal/12 text-fg" : "text-fg hover:bg-raised/70",
      )}
      style={{ gridTemplateColumns: GRID_TEMPLATE, height: ROW_HEIGHT, transform: `translateY(${index * ROW_HEIGHT}px)`, backgroundColor: selected ? undefined : tint }}
      onClick={() => useProcessView.getState().select({ pid: p.pid, startTime: p.startTime }, true)}
      onContextMenu={(event) => {
        useProcessView.getState().select({ pid: p.pid, startTime: p.startTime });
        openContextMenu(event, processMenuItems(p), `Actions for ${p.name}`);
      }}
    >
      {selected && <span className="absolute inset-y-0 left-0 w-0.5 bg-signal" />}
      <div role="gridcell" className="relative flex h-full min-w-0 items-center" style={{ paddingLeft: nameOffset }}>
        {tree && depth > 0 && (
          <>
            {Array.from({ length: depth - 1 }, (_, level) =>
              guides[level + 1] === "1" ? (
                <span key={level} className="absolute inset-y-0 w-px bg-line-strong" style={{ left: 10 + level * INDENT + 7 }} />
              ) : null,
            )}
            <span
              className="absolute top-0 w-px bg-line-strong"
              style={{ left: 10 + (depth - 1) * INDENT + 7, height: isLast ? "50%" : "100%" }}
            />
            <span className="absolute top-1/2 h-px bg-line-strong" style={{ left: 10 + (depth - 1) * INDENT + 7, width: hasChildren ? 6 : 12 }} />
          </>
        )}
        {tree && hasChildren && (
          <button
            type="button"
            tabIndex={-1}
            aria-label={expanded ? `Collapse ${p.name}` : `Expand ${p.name}`}
            onClick={(event) => {
              event.stopPropagation();
              useProcessView.getState().toggleCollapsed(rowKey);
            }}
            className="absolute flex size-4 items-center justify-center rounded-[3px] text-fg-muted hover:bg-line-strong hover:text-fg"
            style={{ left: 10 + depth * INDENT }}
          >
            <CaretRight size={10} weight="bold" className={cx("transition-transform duration-150", expanded && "rotate-90")} />
          </button>
        )}
        <span className="truncate" title={p.cmd.length > 0 ? p.cmd.join(" ") : p.name}>
          {p.name}
        </span>
        {tree && hasChildren && !expanded && (
          <span className="num ml-2 shrink-0 rounded-[4px] bg-line-strong px-1 text-[11px] text-fg-muted">+{descendants}</span>
        )}
      </div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{p.pid}</div>
      <div role="gridcell" className="truncate px-3 text-fg-muted">{p.user ?? "—"}</div>
      <div role="gridcell" className="flex items-center gap-1.5 px-3 text-fg-muted">
        <span className={cx("size-1.5 shrink-0 rounded-full", STATUS_DOT[p.status])} />
        <span className="truncate">{STATUS_LABEL[p.status]}</span>
      </div>
      <div role="gridcell" className="relative flex h-full items-center justify-end px-3">
        <span className="num">{formatPercent(p.cpuPercent)}</span>
        <span
          aria-hidden
          className="absolute bottom-[5px] right-3 h-px w-[calc(100%-24px)] origin-right bg-[var(--viz-1)] opacity-60"
          style={{ transform: `scaleX(${Math.min(1, p.cpuPercent / 100)})` }}
        />
      </div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{formatPercent(p.cpuPercentAvg)}</div>
      <div role="gridcell" className="num px-3 text-right">{formatBytes(p.memoryRss)}</div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{formatBytes(p.memoryVirtual)}</div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{p.threadCount ?? "—"}</div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{p.fdCount ?? "—"}</div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{formatNice(p.nice)}</div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted" title={formatDateTime(p.startTime * 1000)}>
        {startedLabel(p.startTime)}
      </div>
      <div role="gridcell" className="num px-3 text-right text-fg-muted">{formatDuration(p.runTimeSecs)}</div>
    </div>
  );
});

function HeaderRow() {
  const sortKey = useProcessView((s) => s.sortKey);
  const sortDir = useProcessView((s) => s.sortDir);
  const toggleSort = useProcessView((s) => s.toggleSort);
  return (
    <div
      role="row"
      className="sticky top-0 z-10 grid border-b border-line bg-ground text-[12px] text-fg-muted"
      style={{ gridTemplateColumns: GRID_TEMPLATE, height: HEADER_HEIGHT }}
    >
      {COLUMNS.map((c) => {
        const active = c.key === sortKey;
        const Arrow = sortDir === "asc" ? ArrowUp : ArrowDown;
        return (
          <button
            key={c.key}
            type="button"
            role="columnheader"
            aria-sort={active ? (sortDir === "asc" ? "ascending" : "descending") : "none"}
            title={c.title}
            onClick={() => toggleSort(c.key)}
            className={cx(
              "flex h-full items-center gap-1 px-3 hover:text-fg",
              c.align === "right" ? "justify-end" : "justify-start",
              active && "text-fg",
            )}
          >
            {c.align === "right" && active && <Arrow size={10} weight="bold" />}
            <span className="truncate">{c.label}</span>
            {c.align === "left" && active && <Arrow size={10} weight="bold" />}
          </button>
        );
      })}
    </div>
  );
}

export function ProcessTable() {
  const processes = useProcesses((s) => s.processes);
  const totalMemory = useProcesses((s) => s.totalMemory);
  const status = useProcesses((s) => s.status);
  const error = useProcesses((s) => s.error);
  const load = useProcesses((s) => s.load);
  const mode = useProcessView((s) => s.mode);
  const sortKey = useProcessView((s) => s.sortKey);
  const sortDir = useProcessView((s) => s.sortDir);
  const query = useProcessView((s) => s.query);
  const statusFilter = useProcessView((s) => s.status);
  const user = useProcessView((s) => s.user);
  const collapsed = useProcessView((s) => s.collapsed);
  const selected = useProcessView((s) => s.selected);
  const scrollRef = useRef<HTMLDivElement>(null);

  const matcher = useMemo(() => {
    const q = queryMatcher(query);
    if (!q && statusFilter === "all" && !user) return null;
    return (p: ProcessInfo) => statusMatches(statusFilter, p.status) && (!user || p.user === user) && (!q || q(p));
  }, [query, statusFilter, user]);

  const compare = useMemo(() => comparator(sortKey, sortDir), [sortKey, sortDir]);
  const intensity = useMemo(() => intensityModel(processes, totalMemory), [processes, totalMemory]);

  const rows = useMemo<TreeRow[]>(() => {
    if (mode === "tree") return flattenForest(buildForest(processes), { collapsed, compare, match: matcher });
    const list = matcher ? processes.filter(matcher) : [...processes];
    list.sort(compare);
    return list.map((p) => ({
      process: p,
      key: processKey(p),
      depth: 0,
      hasChildren: false,
      expanded: false,
      descendantCount: 0,
      guides: [],
      isLast: false,
    }));
  }, [mode, processes, collapsed, compare, matcher]);

  const { start, end, totalHeight, scrollToIndex } = useVirtualRows(scrollRef, rows.length, ROW_HEIGHT, {
    headerOffset: HEADER_HEIGHT,
  });

  const selectedIndex = selected
    ? rows.findIndex((r) => r.process.pid === selected.pid && r.process.startTime === selected.startTime)
    : -1;

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const view = useProcessView.getState();
    const move = (index: number) => {
      const row = rows[Math.max(0, Math.min(rows.length - 1, index))];
      if (!row) return;
      view.select({ pid: row.process.pid, startTime: row.process.startTime });
      scrollToIndex(rows.indexOf(row));
    };
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        move(selectedIndex + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        move(selectedIndex <= 0 ? 0 : selectedIndex - 1);
        break;
      case "Home":
        event.preventDefault();
        move(0);
        break;
      case "End":
        event.preventDefault();
        move(rows.length - 1);
        break;
      case "Enter":
        if (selected) view.select(selected, true);
        break;
      case "Escape":
        if (view.drawerOpen) view.closeDrawer();
        else view.select(null);
        break;
      case "ArrowRight":
      case "ArrowLeft": {
        const row = rows[selectedIndex];
        if (mode !== "tree" || !row) break;
        event.preventDefault();
        if (event.key === "ArrowRight" && row.hasChildren && !row.expanded) view.toggleCollapsed(row.key);
        else if (event.key === "ArrowLeft" && row.hasChildren && row.expanded) view.toggleCollapsed(row.key);
        else if (event.key === "ArrowLeft" && row.depth > 0) {
          for (let i = selectedIndex - 1; i >= 0; i -= 1) {
            if (rows[i]!.depth === row.depth - 1) {
              move(i);
              break;
            }
          }
        }
        break;
      }
    }
  };

  if (processes.length === 0 && (status === "idle" || status === "loading")) {
    return (
      <div className="h-full overflow-hidden">
        <div className="h-[30px] border-b border-line" />
        <SkeletonRows rows={24} columns={[6, 1.5, 2, 2, 1.5, 1.5, 2, 2, 1.5, 1.2, 1, 2, 2]} />
      </div>
    );
  }

  if (processes.length === 0 && status === "error") {
    return (
      <StatePanel>
        <ErrorState error={error} subject="The process list" onRetry={() => void load()} />
      </StatePanel>
    );
  }

  return (
    <div
      ref={scrollRef}
      role="treegrid"
      aria-label="Processes"
      aria-rowcount={rows.length + 1}
      tabIndex={0}
      onKeyDown={onKeyDown}
      className="h-full overflow-auto focus-visible:outline-none"
    >
      <div style={{ minWidth: MIN_WIDTH }}>
        <HeaderRow />
        {rows.length === 0 ? (
          <StatePanel className="h-[60vh]">
            <EmptyState
              icon={<ListMagnifyingGlass size={22} />}
              title={query ? `No processes match “${query}”` : "No processes match these filters"}
              detail="Search looks at names, PIDs, users and command lines."
              action={
                <Button
                  size="sm"
                  onClick={() => {
                    const v = useProcessView.getState();
                    v.setQuery("");
                    v.setStatus("all");
                    v.setUser(null);
                  }}
                >
                  Clear filters
                </Button>
              }
            />
          </StatePanel>
        ) : (
          <div role="rowgroup" className="relative" style={{ height: totalHeight }}>
            {rows.slice(start, end).map((row, i) => {
              const index = start + i;
              return (
                <ProcessRow
                  key={row.key}
                  process={row.process}
                  rowKey={row.key}
                  index={index}
                  tree={mode === "tree"}
                  depth={row.depth}
                  hasChildren={row.hasChildren}
                  expanded={row.expanded}
                  guides={row.guides.map((g) => (g ? "1" : "0")).join("")}
                  isLast={row.isLast}
                  descendants={row.descendantCount}
                  score={intensity.score(row.process)}
                  selected={index === selectedIndex}
                />
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
