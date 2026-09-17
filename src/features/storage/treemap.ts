// Squarified treemap layout over lazily loaded scan tree slices.
import { hierarchy, treemap, treemapSquarify, type HierarchyRectangularNode } from "d3-hierarchy";

import type { TreeNode } from "../../bindings/TreeNode";

export const GAP = 2;
export const HEADER = 18;

export interface TreemapRect {
  node: TreeNode;
  /** 1 = child of the focused folder, 2 = grandchild. */
  depth: 1 | 2;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  /** Depth-1 folders whose children are drawn inside them get a label strip. */
  hasHeader: boolean;
}

export function isHidden(path: string, hidden: ReadonlySet<string>): boolean {
  if (hidden.size === 0) return false;
  if (hidden.has(path)) return true;
  for (const h of hidden) if (isWithin(path, h)) return true;
  return false;
}

/** True when `path` is `ancestor` or lies beneath it (either separator). */
export function isWithin(path: string, ancestor: string): boolean {
  if (path === ancestor) return true;
  const trimmed = ancestor.replace(/[/\\]+$/, "");
  return path.startsWith(`${trimmed}/`) || path.startsWith(`${trimmed}\\`);
}

let syntheticId = -1;

/**
 * Children to lay out: hidden (removed) paths and empty entries dropped, plus a synthetic
 * remainder when loaded children do not account for the folder's full size.
 */
export function effectiveChildren(node: TreeNode, hidden: ReadonlySet<string> = new Set()): TreeNode[] | null {
  if (!node.children || node.children.length === 0) return null;
  const kids = node.children.filter((c) => c.sizeBytes > 0 && !isHidden(c.path, hidden));
  const covered = node.children.reduce((sum, c) => sum + Math.max(0, c.sizeBytes), 0);
  const gap = node.sizeBytes - covered;
  if (gap > node.sizeBytes * 0.01 && gap > 0) {
    kids.push({
      id: syntheticId--,
      name: "Other items",
      path: `${node.path}#remainder`,
      kind: "remainder",
      sizeBytes: gap,
      itemCount: Math.max(0, node.itemCount - node.children.reduce((s, c) => s + c.itemCount, 0)),
      modified: null,
      accessed: null,
      extension: null,
      fileKind: null,
      category: null,
      unreadableEntries: 0,
      hasChildren: false,
      children: null,
    });
  }
  return kids.length > 0 ? kids : null;
}

export function layoutTreemap(
  root: TreeNode,
  width: number,
  height: number,
  hidden: ReadonlySet<string> = new Set(),
): TreemapRect[] {
  if (width <= 0 || height <= 0) return [];
  const top = effectiveChildren(root, hidden);
  if (!top) return [];
  const topSet = new Set(top);
  const childCache = new Map<TreeNode, TreeNode[] | null>();
  const childrenOf = (d: TreeNode): TreeNode[] | null => {
    if (d === root) return top;
    if (!topSet.has(d) || d.kind !== "directory") return null;
    if (!childCache.has(d)) childCache.set(d, effectiveChildren(d, hidden));
    return childCache.get(d) ?? null;
  };

  const h = hierarchy(root, childrenOf)
    .sum((d) => (childrenOf(d) ? 0 : Math.max(0, d.sizeBytes)))
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

  const laidOut = treemap<TreeNode>()
    .tile(treemapSquarify.ratio(1.3))
    .size([width, height])
    .paddingInner(GAP)
    .paddingTop((d) => (d.depth === 1 && d.children ? HEADER : 0))
    .paddingRight((d) => (d.depth === 1 && d.children ? GAP : 0))
    .paddingBottom((d) => (d.depth === 1 && d.children ? GAP : 0))
    .paddingLeft((d) => (d.depth === 1 && d.children ? GAP : 0))
    .round(true)(h);

  return laidOut
    .descendants()
    .filter((d): d is HierarchyRectangularNode<TreeNode> => d.depth === 1 || d.depth === 2)
    // Grandchildren smaller than a few pixels cannot be seen or pointed at; skip their DOM nodes.
    .filter((d) => d.x1 - d.x0 >= 1 && d.y1 - d.y0 >= 1 && (d.depth === 1 || (d.x1 - d.x0) * (d.y1 - d.y0) >= 6))
    .map((d) => ({
      node: d.data,
      depth: d.depth as 1 | 2,
      x0: d.x0,
      y0: d.y0,
      x1: d.x1,
      y1: d.y1,
      hasHeader: d.depth === 1 && Boolean(d.children) && d.y1 - d.y0 > HEADER + 8,
    }));
}

export interface ZoomTransform {
  x: number;
  y: number;
  scaleX: number;
  scaleY: number;
}

/** Transform (origin top-left) that makes `rect` fill a width × height viewport. */
export function zoomInto(rect: Pick<TreemapRect, "x0" | "y0" | "x1" | "y1">, width: number, height: number): ZoomTransform {
  const w = Math.max(1, rect.x1 - rect.x0);
  const h = Math.max(1, rect.y1 - rect.y0);
  const scaleX = width / w;
  const scaleY = height / h;
  return { x: -rect.x0 * scaleX, y: -rect.y0 * scaleY, scaleX, scaleY };
}

export const IDENTITY: ZoomTransform = { x: 0, y: 0, scaleX: 1, scaleY: 1 };

/** Inverse of `zoomInto`: shrinks the full viewport down onto `rect`. */
export function shrinkInto(rect: Pick<TreemapRect, "x0" | "y0" | "x1" | "y1">, width: number, height: number): ZoomTransform {
  return {
    x: rect.x0,
    y: rect.y0,
    scaleX: Math.max(1, rect.x1 - rect.x0) / width,
    scaleY: Math.max(1, rect.y1 - rect.y0) / height,
  };
}

/** Truncates `text` with an ellipsis so it fits `maxWidth` at an average glyph width; null if nothing fits. */
export function fitLabel(text: string, maxWidth: number, charWidth = 6.3): string | null {
  const capacity = Math.floor(maxWidth / charWidth);
  if (capacity < 3) return null;
  if (text.length <= capacity) return text;
  return `${text.slice(0, capacity - 1)}…`;
}

/** Finds the rect for `nodeId` or, failing that, the depth-1 rect whose path contains `path`. */
export function rectForPath(rects: TreemapRect[], nodeId: number, path: string): TreemapRect | null {
  return (
    rects.find((r) => r.node.id === nodeId) ??
    rects.find((r) => r.depth === 1 && r.node.kind === "directory" && isWithin(path, r.node.path)) ??
    null
  );
}
