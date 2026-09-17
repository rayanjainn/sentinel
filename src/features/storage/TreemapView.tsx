// Storage treemap: squarified folders and files colored by kind, with transform-based zoom into
// folders and back out through breadcrumbs.
import { CaretRight, House } from "@phosphor-icons/react";
import { motion } from "motion/react";
import { memo, useId, useMemo, useRef, useState, type PointerEvent, type ReactNode } from "react";

import type { TreeNode } from "../../bindings/TreeNode";
import { openContextMenu } from "../../components/ContextMenu";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState, StatePanel } from "../../components/States";
import { cx } from "../../lib/cx";
import { formatBytes, formatCount, formatDateTime, formatRelativeSecs, pluralize } from "../../lib/format";
import { springPanel, usePrefersReducedMotion } from "../../lib/motion";
import { useResolvedTheme } from "../../shell/useTheme";
import { sliceKey, useStorage } from "../../stores/storage";
import { FILE_KINDS, fileKindFill, fileKindHex, inkOn } from "../../styles/dataviz";
import { CATEGORY_LABEL } from "./categories";
import { nodeMenu } from "./storageActions";
import {
  fitLabel,
  HEADER,
  IDENTITY,
  layoutTreemap,
  rectForPath,
  shrinkInto,
  zoomInto,
  type TreemapRect,
  type ZoomTransform,
} from "./treemap";
import { useStorageView, type Crumb } from "./viewState";

type Frame = ZoomTransform & { opacity: number };

interface Layer {
  key: string;
  crumbId: number | null;
  from: Frame;
  to: Frame;
  exiting: boolean;
}

const crumbOf = (n: TreeNode): Crumb => ({ id: n.id, name: n.name, path: n.path });

function fillFor(r: TreemapRect): string {
  switch (r.node.kind) {
    case "smallFiles":
      return "url(#tm-hatch-small)";
    case "remainder":
      return "url(#tm-hatch-rest)";
    case "directory":
      return r.hasHeader ? "var(--viz-directory)" : "color-mix(in srgb, var(--fg) 9%, var(--viz-directory))";
    default:
      return fileKindFill(r.node.fileKind);
  }
}

function inkFor(r: TreemapRect, mode: "dark" | "light"): string {
  if (r.node.kind === "file" || r.node.kind === "symlink" || r.node.kind === "other") {
    return inkOn(fileKindHex(r.node.fileKind, mode));
  }
  return "var(--fg)";
}

const TreemapLayer = memo(function TreemapLayer({
  node,
  width,
  height,
  hidden,
  mode,
  interactive,
  onHover,
  onZoom,
}: {
  node: TreeNode;
  width: number;
  height: number;
  hidden: Set<string>;
  mode: "dark" | "light";
  interactive: boolean;
  onHover: (rect: TreemapRect | null, x: number, y: number) => void;
  onZoom: (rect: TreemapRect, rects: TreemapRect[]) => void;
}) {
  const rects = useMemo(() => layoutTreemap(node, width, height, hidden), [node, width, height, hidden]);
  const patternId = useId();

  const rectAt = (event: { target: EventTarget }) => {
    const el = (event.target as Element).closest("[data-i]");
    const i = el ? Number(el.getAttribute("data-i")) : -1;
    return rects[i] ?? null;
  };

  if (rects.length === 0) {
    return (
      <StatePanel className="h-full">
        <p className="text-fg-muted">{node.sizeBytes > 0 ? "Nothing here is large enough to draw." : "This folder is empty."}</p>
      </StatePanel>
    );
  }

  return (
    <svg
      width={width}
      height={height}
      role="img"
      aria-label={`Treemap of ${node.name}`}
      className="block"
      onPointerMove={(e: PointerEvent<SVGSVGElement>) => {
        if (!interactive) return;
        const box = e.currentTarget.getBoundingClientRect();
        onHover(rectAt(e), e.clientX - box.left, e.clientY - box.top);
      }}
      onPointerLeave={() => onHover(null, 0, 0)}
      onClick={(e) => {
        if (!interactive) return;
        const r = rectAt(e);
        if (r && r.node.kind === "directory") onZoom(r, rects);
      }}
      onContextMenu={(e) => {
        if (!interactive) return;
        const r = rectAt(e);
        if (!r) return;
        openContextMenu(e, nodeMenu(r.node, r.node.kind === "directory" ? () => onZoom(r, rects) : undefined), r.node.name);
      }}
    >
      <defs>
        <pattern id={`${patternId}-small`} width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="6" height="6" fill="var(--viz-kind-folded-alt)" />
          <line x1="0" y1="0" x2="0" y2="6" stroke="var(--viz-hatch)" strokeWidth="1.2" />
        </pattern>
        <pattern id={`${patternId}-rest`} width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(135)">
          <rect width="6" height="6" fill="var(--viz-directory)" />
          <line x1="0" y1="0" x2="0" y2="6" stroke="var(--viz-hatch)" strokeOpacity="0.6" strokeWidth="1" />
        </pattern>
      </defs>
      {rects.map((r, i) => {
        const w = r.x1 - r.x0;
        const h = r.y1 - r.y0;
        const fill = fillFor(r).replace("#tm-hatch-small", `#${patternId}-small`).replace("#tm-hatch-rest", `#${patternId}-rest`);
        const ink = inkFor(r, mode);
        const zoomable = r.node.kind === "directory";
        const size = formatBytes(r.node.sizeBytes, { base: 1000 });
        let label: ReactNode = null;
        if (r.hasHeader) {
          const sizeWidth = size.length * 6.2 + 8;
          const name = fitLabel(r.node.name, w - 12 - (w > 140 ? sizeWidth : 0), 6.4);
          label = (
            <>
              {name && (
                <text x={r.x0 + 6} y={r.y0 + 12.5} fontSize={11} fontWeight={500} fill="var(--fg)">
                  {name}
                </text>
              )}
              {w > 140 && (
                <text x={r.x1 - 6} y={r.y0 + 12.5} fontSize={11} textAnchor="end" fill="var(--fg-muted)" style={{ fontFamily: "var(--font-mono)" }}>
                  {size}
                </text>
              )}
            </>
          );
        } else if (w >= 48 && h >= 30 && r.node.kind !== "remainder") {
          const name = fitLabel(r.node.kind === "smallFiles" ? "Small files" : r.node.name, w - 10, 6.8);
          label = name ? (
            <>
              <text x={r.x0 + 5} y={r.y0 + 15} fontSize={12} fill={ink}>
                {name}
              </text>
              {h >= 44 && (
                <text x={r.x0 + 5} y={r.y0 + 29} fontSize={11} fill={ink} fillOpacity={0.75} style={{ fontFamily: "var(--font-mono)" }}>
                  {size}
                </text>
              )}
            </>
          ) : null;
        }
        return (
          <g key={`${r.node.id}-${i}`} data-i={i} className={cx(interactive && zoomable && "cursor-zoom-in")}>
            <rect x={r.x0} y={r.y0} width={w} height={h} rx={r.depth === 1 ? 3 : 2} fill={fill} />
            {r.hasHeader && <rect x={r.x0} y={r.y0} width={w} height={HEADER} rx={3} fill="transparent" />}
            {label}
          </g>
        );
      })}
    </svg>
  );
});

function Tooltip({ node, x, y, width, height }: { node: TreeNode; x: number; y: number; width: number; height: number }) {
  const flipX = x > width - 320;
  const flipY = y > height - 160;
  const kindLabel =
    node.kind === "smallFiles"
      ? "Files under 64 KB in this folder, grouped together"
      : node.kind === "remainder"
        ? "Items too small to draw at this level"
        : node.kind === "directory"
          ? "Folder"
          : node.fileKind
            ? FILE_KINDS[node.fileKind].label
            : "File";
  return (
    <div
      role="tooltip"
      className="shadow-float pointer-events-none absolute z-20 flex w-[300px] flex-col gap-1.5 rounded-[8px] bg-raised px-3 py-2.5 text-[12px]"
      style={{ left: flipX ? undefined : x + 14, right: flipX ? width - x + 14 : undefined, top: flipY ? undefined : y + 14, bottom: flipY ? height - y + 14 : undefined }}
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="truncate font-medium text-fg">{node.kind === "smallFiles" ? "Small files" : node.name}</span>
        <span className="num shrink-0 text-fg">{formatBytes(node.sizeBytes, { base: 1000 })}</span>
      </div>
      {node.kind !== "remainder" && node.kind !== "smallFiles" && <span className="num break-all text-[11px] text-fg-muted">{node.path}</span>}
      <dl className="grid grid-cols-[88px_1fr] gap-x-2 gap-y-0.5 text-fg-muted">
        <dt>Type</dt>
        <dd className="text-fg">{kindLabel}</dd>
        {/* This is the on-disk allocation (st_blocks / GetCompressedFileSize), the number that
            frees space when moved to the Trash — see the "Size on disk" InfoTip elsewhere in this
            view. An InfoTip is not placed inside this hover panel itself: moving the pointer off
            the treemap SVG to reach it would close this panel first. */}
        <dt>Size on disk</dt>
        <dd className="num text-fg">{formatCount(node.sizeBytes)} bytes</dd>
        {node.kind !== "file" && (
          <>
            <dt>Items</dt>
            <dd className="num text-fg">{formatCount(node.itemCount)}</dd>
          </>
        )}
        {node.modified !== null && (
          <>
            <dt>Modified</dt>
            <dd className="text-fg" title={formatDateTime(node.modified * 1000)}>
              {formatRelativeSecs(node.modified)}
            </dd>
          </>
        )}
      </dl>
      {node.category && <span className="text-signal">Suggested cleanup: {CATEGORY_LABEL[node.category]}</span>}
      {node.unreadableEntries > 0 && (
        <span className="text-warn">{pluralize(node.unreadableEntries, "entry", "entries")} could not be read, so this size may be low</span>
      )}
      {node.kind === "directory" && node.hasChildren && <span className="text-fg-subtle">Click to zoom in</span>}
    </div>
  );
}

export function Breadcrumbs({ onNavigate }: { onNavigate: (index: number) => void }) {
  const crumbs = useStorageView((s) => s.crumbs);
  return (
    <nav aria-label="Folder path" className="flex min-w-0 items-center gap-0.5 overflow-hidden text-[12px]">
      {crumbs.map((c, i) => {
        const last = i === crumbs.length - 1;
        return (
          <span key={`${c.id ?? "root"}-${i}`} className={cx("flex min-w-0 items-center gap-0.5", !last && "shrink")}>
            {i > 0 && <CaretRight size={10} className="shrink-0 text-fg-subtle" />}
            <button
              type="button"
              disabled={last}
              aria-current={last ? "location" : undefined}
              onClick={() => onNavigate(i)}
              title={c.path}
              className={cx(
                "flex min-w-0 items-center gap-1 rounded-[4px] px-1.5 py-0.5",
                last ? "font-medium text-fg" : "text-fg-muted hover:bg-raised hover:text-fg",
              )}
            >
              {i === 0 && <House size={12} className="shrink-0" />}
              <span className="truncate">{c.name}</span>
            </button>
          </span>
        );
      })}
    </nav>
  );
}

let layerSeq = 0;

export function useTreemapNavigation(width: number, height: number) {
  const crumbs = useStorageView((s) => s.crumbs);
  const reduced = usePrefersReducedMotion();
  const [layers, setLayers] = useState<Layer[]>([]);
  const [provisional, setProvisional] = useState<Map<number, TreeNode>>(new Map());

  const zoomIn = (rect: TreemapRect, rects: TreemapRect[]) => {
    const { crumbs: current, pushCrumbs } = useStorageView.getState();
    const store = useStorage.getState();
    if (rect.node.kind !== "directory") return;
    const chain: TreeNode[] = [];
    if (rect.depth === 2) {
      const parent = rects.find((r) => r.depth === 1 && r.node.kind === "directory" && r.node.children?.some((c) => c.id === rect.node.id));
      if (parent) chain.push(parent.node);
    }
    chain.push(rect.node);
    setProvisional((prev) => {
      const next = new Map(prev);
      for (const n of chain) next.set(n.id, n);
      return next;
    });
    const fromId = current[current.length - 1]?.id ?? null;
    pushCrumbs(chain.map(crumbOf));
    void store.fetchSlice(rect.node.id).catch(() => undefined);
    if (reduced || width <= 0) {
      setLayers([]);
      return;
    }
    const seq = ++layerSeq;
    setLayers([
      { key: `out-${seq}`, crumbId: fromId, from: { ...IDENTITY, opacity: 1 }, to: { ...zoomInto(rect, width, height), opacity: 0 }, exiting: true },
      { key: `in-${seq}`, crumbId: rect.node.id, from: { ...IDENTITY, opacity: 0 }, to: { ...IDENTITY, opacity: 1 }, exiting: false },
    ]);
  };

  const zoomOutTo = async (index: number) => {
    const { crumbs: current, popTo } = useStorageView.getState();
    const store = useStorage.getState();
    const target = current[index];
    const from = current[current.length - 1];
    if (!target || !from || index >= current.length - 1) return;
    let node = store.slices.get(sliceKey(target.id)) ?? (target.id !== null ? provisional.get(target.id) : undefined);
    if (!node) node = await store.fetchSlice(target.id).catch(() => undefined);
    popTo(index);
    if (!node || reduced || width <= 0) {
      setLayers([]);
      return;
    }
    const rects = layoutTreemap(node, width, height, store.removed);
    const r = rectForPath(rects, from.id ?? Number.NaN, from.path);
    const seq = ++layerSeq;
    setLayers([
      { key: `out-${seq}`, crumbId: from.id, from: { ...IDENTITY, opacity: 1 }, to: { ...(r ? shrinkInto(r, width, height) : IDENTITY), opacity: 0 }, exiting: true },
      { key: `in-${seq}`, crumbId: target.id, from: { ...(r ? zoomInto(r, width, height) : IDENTITY), opacity: 0 }, to: { ...IDENTITY, opacity: 1 }, exiting: false },
    ]);
  };

  /** Zoom from a list row: animates from the node's rect when it is visible in the current layout. */
  const zoomToNode = (node: TreeNode) => {
    const { crumbs: current } = useStorageView.getState();
    const store = useStorage.getState();
    const focus = current[current.length - 1];
    const focusNode = focus ? (store.slices.get(sliceKey(focus.id)) ?? (focus.id !== null ? provisional.get(focus.id) : undefined)) : undefined;
    const rects = focusNode && width > 0 ? layoutTreemap(focusNode, width, height, store.removed) : [];
    const rect = rects.find((r) => r.node.id === node.id) ?? {
      node,
      depth: 1 as const,
      x0: 0,
      y0: 0,
      x1: width,
      y1: height,
      hasHeader: false,
    };
    zoomIn(rect, rects);
  };

  return { crumbs, layers, setLayers, provisional, zoomIn, zoomOutTo, zoomToNode };
}

export function TreemapStage({
  width,
  height,
  navigation,
}: {
  width: number;
  height: number;
  navigation: ReturnType<typeof useTreemapNavigation>;
}) {
  const { crumbs, layers, setLayers, provisional, zoomIn } = navigation;
  const slices = useStorage((s) => s.slices);
  const sliceErrors = useStorage((s) => s.sliceErrors);
  const removed = useStorage((s) => s.removed);
  const summary = useStorage((s) => s.summary);
  const partial = useStorage((s) => s.partial);
  const fetchSlice = useStorage((s) => s.fetchSlice);
  const mode = useResolvedTheme();
  const [hover, setHover] = useState<{ rect: TreemapRect; x: number; y: number } | null>(null);
  const prefetchTimer = useRef<number | null>(null);

  const nodeFor = (id: number | null): TreeNode | undefined =>
    slices.get(sliceKey(id)) ?? (id !== null ? provisional.get(id) : undefined);

  const onHover = (rect: TreemapRect | null, x: number, y: number) => {
    setHover((prev) => (rect ? { rect, x, y } : prev ? null : prev));
    if (prefetchTimer.current) window.clearTimeout(prefetchTimer.current);
    if (rect && rect.node.kind === "directory" && rect.node.hasChildren) {
      const id = rect.node.id;
      prefetchTimer.current = window.setTimeout(() => void fetchSlice(id).catch(() => undefined), 160);
    }
  };

  // Live partial tree while the scan is still walking.
  if (!summary) {
    if (!partial) {
      return <Skeleton className="absolute inset-0 rounded-none" />;
    }
    return (
      <TreemapLayer node={partial} width={width} height={height} hidden={removed} mode={mode} interactive={false} onHover={() => undefined} onZoom={() => undefined} />
    );
  }

  const current = crumbs[crumbs.length - 1] ?? { id: null, name: "", path: "" };
  const currentNode = nodeFor(current.id);
  const error = sliceErrors.get(sliceKey(current.id));

  const rendered: Layer[] =
    layers.length > 0
      ? layers
      : [{ key: `static-${current.id ?? "root"}`, crumbId: current.id, from: { ...IDENTITY, opacity: 1 }, to: { ...IDENTITY, opacity: 1 }, exiting: false }];

  return (
    <>
      {rendered.map((layer) => {
        const node = nodeFor(layer.crumbId);
        const active = !layer.exiting && layers.length === 0;
        return (
          <motion.div
            key={layer.key}
            className={cx("absolute inset-0", layer.exiting && "pointer-events-none")}
            style={{ originX: 0, originY: 0 }}
            initial={{ ...layer.from }}
            animate={{ ...layer.to }}
            transition={{ default: springPanel, opacity: { duration: layer.exiting ? 0.32 : 0.28, delay: layer.exiting ? 0.06 : 0.14 } }}
            onAnimationComplete={() => {
              if (!layer.exiting) setLayers([]);
            }}
          >
            {node && (node.children || node.kind !== "directory") ? (
              <TreemapLayer
                node={node}
                width={width}
                height={height}
                hidden={removed}
                mode={mode}
                interactive={active}
                onHover={onHover}
                onZoom={(r, rects) => {
                  setHover(null);
                  zoomIn(r, rects);
                }}
              />
            ) : (
              !layer.exiting && !error && <Skeleton className="absolute inset-0 rounded-none" />
            )}
          </motion.div>
        );
      })}
      {error !== undefined && !currentNode?.children && (
        <StatePanel className="absolute inset-0 bg-ground/80">
          <ErrorState error={error} subject="This folder" onRetry={() => void fetchSlice(current.id).catch(() => undefined)} />
        </StatePanel>
      )}
      {hover && layers.length === 0 && (
        <>
          <svg className="pointer-events-none absolute inset-0" width={width} height={height} aria-hidden>
            <rect
              x={hover.rect.x0 + 0.75}
              y={hover.rect.y0 + 0.75}
              width={Math.max(0, hover.rect.x1 - hover.rect.x0 - 1.5)}
              height={Math.max(0, hover.rect.y1 - hover.rect.y0 - 1.5)}
              rx={2}
              fill="none"
              stroke="var(--fg)"
              strokeOpacity={0.9}
              strokeWidth={1.5}
            />
          </svg>
          <Tooltip node={hover.rect.node} x={hover.x} y={hover.y} width={width} height={height} />
        </>
      )}
    </>
  );
}
