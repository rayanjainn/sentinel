import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));

import { balancedColumns } from "./CoreField";

describe("balancedColumns", () => {
  it("splits cores into even rows instead of leaving an orphan", () => {
    expect(balancedColumns(8, 1180, 156)).toBe(4);
    expect(balancedColumns(10, 1180, 156)).toBe(5);
    expect(balancedColumns(12, 2000, 156)).toBe(12);
  });

  it("uses one row when everything fits and never returns zero", () => {
    expect(balancedColumns(4, 1180, 156)).toBe(4);
    expect(balancedColumns(0, 1000, 156)).toBe(1);
    expect(balancedColumns(6, 50, 156)).toBe(1);
  });
});
