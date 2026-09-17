import { describe, expect, it } from "vitest";

import { datavizCss, FILE_KIND_ORDER, FILE_KINDS, fileKindHex, inkOn } from "./dataviz";

describe("dataviz palette", () => {
  it("defines every file kind for both themes", () => {
    const css = datavizCss();
    for (const kind of FILE_KIND_ORDER) {
      expect(css).toContain(FILE_KINDS[kind].varName);
      expect(fileKindHex(kind, "dark")).toMatch(/^#[0-9a-f]{6}$/);
      expect(fileKindHex(kind, "light")).toMatch(/^#[0-9a-f]{6}$/);
    }
    expect(css).toContain('[data-theme="light"]');
  });

  it("never reuses a categorical slot for two unfolded kinds", () => {
    const slots = FILE_KIND_ORDER.filter((k) => k !== "diskImage")
      .map((k) => FILE_KINDS[k].slot)
      .filter((s): s is number => s !== null);
    expect(new Set(slots).size).toBe(slots.length);
  });

  it("picks the more legible label ink", () => {
    expect(inkOn("#0e1217")).toBe("#ffffff");
    expect(inkOn("#f3f5f7")).toBe("#0e1217");
  });
});
