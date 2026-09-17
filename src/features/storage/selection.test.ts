import { describe, expect, it } from "vitest";

import type { DuplicateGroup } from "../../bindings/DuplicateGroup";
import type { FileEntry } from "../../bindings/FileEntry";
import { collapseNested, defaultDuplicateSelection, keepsOneCopy, keeperOf, summarize, toggle } from "./selection";

const file = (path: string, modified: number | null): FileEntry => ({
  nodeId: 1,
  path,
  name: path.split("/").pop() ?? path,
  sizeBytes: 10,
  modified,
  accessed: null,
  extension: null,
  fileKind: "other",
});

const group = (files: FileEntry[]): DuplicateGroup => ({ hash: "h", sizeBytes: 10, files, reclaimableBytes: 10 * (files.length - 1) });

describe("collapseNested", () => {
  it("drops children of selected folders", () => {
    const out = collapseNested([
      { path: "/a/b/c.txt", sizeBytes: 1 },
      { path: "/a/b", sizeBytes: 5 },
      { path: "/a/bc", sizeBytes: 2 },
    ]);
    expect(out.map((i) => i.path).sort()).toEqual(["/a/b", "/a/bc"]);
    expect(summarize([{ path: "/x", sizeBytes: 4 }, { path: "/x/y", sizeBytes: 3 }])).toEqual({ count: 1, bytes: 4 });
  });
});

describe("duplicate selection", () => {
  it("keeps the oldest copy and selects the others", () => {
    const g = group([file("/new/copy.jpg", 300), file("/old/photo.jpg", 100), file("/mid/photo.jpg", 200)]);
    expect(keeperOf(g)).toBe("/old/photo.jpg");
    const selected = defaultDuplicateSelection([g]);
    expect([...selected].sort()).toEqual(["/mid/photo.jpg", "/new/copy.jpg"]);
    expect(keepsOneCopy(g, selected)).toBe(true);
    expect(keepsOneCopy(g, new Set(g.files.map((f) => f.path)))).toBe(false);
  });

  it("prefers known timestamps and shorter paths", () => {
    const g = group([file("/a/very/long/path.bin", null), file("/b/x.bin", null)]);
    expect(keeperOf(g)).toBe("/b/x.bin");
  });

  it("toggles set membership immutably", () => {
    const s = new Set(["a"]);
    expect([...toggle(s, "b")]).toEqual(["a", "b"]);
    expect([...toggle(s, "a")]).toEqual([]);
    expect([...toggle(s, "a", true)]).toEqual(["a"]);
    expect([...s]).toEqual(["a"]);
  });
});
