import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Action } from "../../bindings/Action";
import type { ActionOutcome } from "../../bindings/ActionOutcome";
import type { ActionPreview } from "../../bindings/ActionPreview";

const api = vi.hoisted(() => ({
  prepareAction: vi.fn(),
  commitAction: vi.fn(),
  rejectAction: vi.fn(),
}));

vi.mock("../../lib/ipc", async () => {
  class IpcError extends Error {
    constructor(readonly payload: { code: string; message: string }) {
      super(payload.message);
    }
  }
  return { api, IpcError };
});

import { IpcError } from "../../lib/ipc";
import { useToasts } from "../../stores/toasts";
import { cancelAction, confirmAction, runAction, useActionFlow } from "./flow";

const action: Action = { type: "trashPaths", paths: ["/tmp/a"] };

function preview(overrides: Partial<ActionPreview> = {}): ActionPreview {
  return {
    token: "t1",
    action,
    origin: { type: "user" },
    title: "Move 1 item to Trash",
    description: "d",
    targets: [],
    impact: [],
    estimatedBytesFreed: null,
    risk: "moderate",
    reversibility: { type: "recoverable", how: "Put back from Trash" },
    warnings: [],
    requiresElevation: false,
    createdAtMs: Date.now(),
    expiresAtMs: Date.now() + 300_000,
    ...overrides,
  };
}

function outcome(status: ActionOutcome["status"]): ActionOutcome {
  return {
    action,
    origin: { type: "user" },
    status,
    items: [],
    before: [],
    after: [],
    summary: "Moved 1 item to Trash",
    auditId: 1,
    finishedAtMs: Date.now(),
  };
}

const flush = () => new Promise((r) => setTimeout(r, 0));
const phase = () => useActionFlow.getState().state.phase;

describe("action flow", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.rejectAction.mockResolvedValue(undefined);
    useActionFlow.setState({ state: { phase: "idle" } });
    useToasts.setState({ toasts: [] });
  });

  it("prepares, commits and resolves the outcome", async () => {
    api.prepareAction.mockResolvedValue(preview());
    api.commitAction.mockResolvedValue(outcome("succeeded"));
    const result = runAction(action);
    await flush();
    expect(phase()).toBe("confirm");
    await confirmAction();
    await expect(result).resolves.toMatchObject({ status: "succeeded" });
    expect(phase()).toBe("idle");
    expect(api.commitAction).toHaveBeenCalledWith("t1");
    expect(useToasts.getState().toasts[0]?.title).toBe("Moved to Trash");
  });

  it("keeps partial outcomes open for review", async () => {
    api.prepareAction.mockResolvedValue(preview());
    api.commitAction.mockResolvedValue(outcome("partiallySucceeded"));
    void runAction(action);
    await flush();
    await confirmAction();
    expect(phase()).toBe("outcome");
  });

  it("rejects the token on cancel", async () => {
    api.prepareAction.mockResolvedValue(preview());
    const result = runAction(action);
    await flush();
    cancelAction();
    await expect(result).resolves.toBeNull();
    expect(api.rejectAction).toHaveBeenCalledWith("t1");
    expect(api.commitAction).not.toHaveBeenCalled();
  });

  it("re-prepares when the backend reports an invalid token", async () => {
    api.prepareAction.mockResolvedValueOnce(preview()).mockResolvedValueOnce(preview({ token: "t2" }));
    api.commitAction.mockRejectedValueOnce(new IpcError({ code: "actionTokenInvalid", message: "expired" }));
    void runAction(action);
    await flush();
    await confirmAction();
    const state = useActionFlow.getState().state;
    expect(state.phase).toBe("confirm");
    expect(state.phase === "confirm" && state.preview.token).toBe("t2");
    expect(state.phase === "confirm" && state.refreshed).toBe(true);
  });

  it("re-prepares an expired preview instead of committing it", async () => {
    api.prepareAction
      .mockResolvedValueOnce(preview({ expiresAtMs: Date.now() - 1 }))
      .mockResolvedValueOnce(preview({ token: "fresh" }));
    void runAction(action);
    await flush();
    await confirmAction();
    expect(api.commitAction).not.toHaveBeenCalled();
    expect(api.prepareAction).toHaveBeenCalledTimes(2);
  });

  it("surfaces prepare errors", async () => {
    api.prepareAction.mockRejectedValue(new IpcError({ code: "unavailable", feature: "actions", reason: "not wired", message: "nope" }));
    void runAction(action);
    await flush();
    expect(phase()).toBe("error");
  });
});
