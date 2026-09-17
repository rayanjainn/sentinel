import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => undefined)) }));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: vi.fn() }));

import { validateHomeInput } from "./NetworkSettings";

describe("validateHomeInput", () => {
  it("accepts a named coordinate", () => {
    expect(validateHomeInput("Lisbon", "38.72", "-9.14")).toBeNull();
  });

  it("rejects missing names and out-of-range coordinates", () => {
    expect(validateHomeInput(" ", "1", "1")).toMatch(/name/);
    expect(validateHomeInput("x", "91", "0")).toMatch(/Latitude/);
    expect(validateHomeInput("x", "", "0")).toMatch(/Latitude/);
    expect(validateHomeInput("x", "10", "-181")).toMatch(/Longitude/);
    expect(validateHomeInput("x", "10", "abc")).toMatch(/Longitude/);
  });
});
