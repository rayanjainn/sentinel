import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { describeError, isCode } from "./errors";
import { IpcError } from "./ipc";

describe("describeError", () => {
  it("renders unavailable as a quiet note", () => {
    const d = describeError(
      new IpcError({ code: "unavailable", feature: "thermal", reason: "no sensors", message: "x" }),
      "Temperature",
    );
    expect(d.quiet).toBe(true);
    expect(d.title).toBe("Temperature is not available on this system");
    expect(d.detail).toBe("no sensors");
  });

  it("treats work in progress as not ready rather than unsupported", () => {
    const d = describeError(
      new IpcError({ code: "unavailable", feature: "scan summary", reason: "still scanning", message: "x" }),
      "The scan summary",
    );
    expect(d.quiet).toBe(true);
    expect(d.title).toBe("The scan summary is not ready yet");
    expect(d.detail).toBe("Sentinel is still scanning. Try again when it finishes.");
  });

  it("links permission errors about files to Full Disk Access", () => {
    const d = describeError(
      new IpcError({
        code: "permissionDenied",
        operation: "read directory",
        target: "/Library/Caches",
        hint: "Grant Full Disk Access",
        message: "x",
      }),
    );
    expect(d.permission).toBe("fullDiskAccess");
    expect(d.title).toContain("/Library/Caches");
  });

  it("links other permission errors to administrator rights", () => {
    const d = describeError(
      new IpcError({ code: "permissionDenied", operation: "change priority", target: null, hint: null, message: "m" }),
    );
    expect(d.permission).toBe("administrator");
  });

  it("names the process that vanished", () => {
    const d = describeError(new IpcError({ code: "processNotFound", pid: 4821, message: "x" }));
    expect(d.title).toBe("Process 4821 is no longer running");
  });

  it("wraps unknown errors as internal", () => {
    const d = describeError(new Error("boom"), "Scan");
    expect(d.code).toBe("internal");
    expect(d.title).toBe("Scan failed");
    expect(d.detail).toBe("boom");
  });

  it("checks codes", () => {
    expect(isCode(new IpcError({ code: "cancelled", message: "c" }), "cancelled")).toBe(true);
    expect(isCode(new Error("x"), "cancelled")).toBe(false);
  });
});
