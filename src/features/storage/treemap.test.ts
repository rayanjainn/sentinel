import { describe, expect, it } from "vitest";

import type { TreeNode } from "../../bindings/TreeNode";
import { effectiveChildren, fitLabel, isWithin, layoutTreemap, rectForPath, shrinkInto, zoomInto } from "./treemap";

let id = 1;
function node(name: string, size: number, children: TreeNode[] | null = null, over: Partial<TreeNode> = {}): TreeNode {
  return {
    id: id++,
    name,
    path: `/root/${name}`,
    kind: children ? "directory" : "file",
    sizeBytes: size,
    itemCount: children ? children.length : 1,
    modified: null,
    accessed: null,
    extension: null,
    fileKind: children ? null : "document",
    category: null,
    unreadableEntries: 0,
    hasChildren: Boolean(children),
    children,
    ...over,
  };
}

describe("effectiveChildren", () => {
  it("adds a remainder when loaded children do not cover the folder", () => {
    const dir = node("d", 100, [node("a", 60), node("b", 20)]);
    const kids = effectiveChildren(dir)!;
    expect(kids.map((k) => k.kind)).toEqual(["file", "file", "remainder"]);
    expect(kids[2]!.sizeBytes).toBe(20);
  });

  it("drops hidden and empty children", () => {
    const dir = node("d", 80, [node("a", 60), node("b", 20), node("z", 0)]);
    const kids = effectiveChildren(dir, new Set(["/root/b"]))!;
    expect(kids.map((k) => k.name)).toEqual(["a"]);
  });
});

describe("layoutTreemap", () => {
  it("lays out two levels with areas proportional to size", () => {
    const root = node("root", 400, [node("big", 300, [node("x", 200), node("y", 100)]), node("small", 100)]);
    const rects = layoutTreemap(root, 400, 200);
    const big = rects.find((r) => r.node.name === "big")!;
    const small = rects.find((r) => r.node.name === "small")!;
    expect(big.depth).toBe(1);
    expect(big.hasHeader).toBe(true);
    expect(rects.find((r) => r.node.name === "x")!.depth).toBe(2);
    const area = (r: typeof big) => (r.x1 - r.x0) * (r.y1 - r.y0);
    expect(area(big) / area(small)).toBeGreaterThan(2);
    expect(area(big) / area(small)).toBeLessThan(4);
    // Grandchildren sit inside their parent, below the label strip.
    const x = rects.find((r) => r.node.name === "x")!;
    expect(x.y0).toBeGreaterThanOrEqual(big.y0 + 18);
    expect(x.x1).toBeLessThanOrEqual(big.x1);
  });

  it("returns nothing for empty folders or zero size", () => {
    expect(layoutTreemap(node("r", 0, []), 100, 100)).toEqual([]);
    expect(layoutTreemap(node("r", 10, [node("a", 10)]), 0, 100)).toEqual([]);
  });
});

describe("zoom and labels", () => {
  it("maps a rect onto the viewport", () => {
    expect(zoomInto({ x0: 100, y0: 50, x1: 300, y1: 150 }, 400, 200)).toEqual({ x: -200, y: -100, scaleX: 2, scaleY: 2 });
    expect(shrinkInto({ x0: 100, y0: 50, x1: 300, y1: 150 }, 400, 200)).toEqual({ x: 100, y: 50, scaleX: 0.5, scaleY: 0.5 });
  });

  it("fits labels with an ellipsis", () => {
    expect(fitLabel("Documents", 200)).toBe("Documents");
    expect(fitLabel("Applications", 40)).toBe("Appli…");
    expect(fitLabel("abc", 10)).toBeNull();
  });

  it("matches paths on separator boundaries", () => {
    expect(isWithin("/a/b/c", "/a/b")).toBe(true);
    expect(isWithin("/a/bc", "/a/b")).toBe(false);
    expect(isWithin("C:\\Users\\x", "C:\\Users")).toBe(true);
  });

  it("finds the rect containing a deeper focus", () => {
    const root = node("root", 400, [node("big", 300, [node("x", 300)]), node("small", 100)]);
    const rects = layoutTreemap(root, 400, 200);
    const deep = rectForPath(rects, -999, "/root/big/nested/file");
    expect(deep?.node.name).toBe("big");
  });
});
