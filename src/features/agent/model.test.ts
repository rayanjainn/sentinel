import { describe, expect, it } from "vitest";

import type { AgentSettings } from "../../bindings/AgentSettings";
import type { OllamaStatus } from "../../bindings/OllamaStatus";
import type { Plan } from "../../bindings/Plan";
import type { PlanAction } from "../../bindings/PlanAction";
import {
  approvedCount,
  decisionsFor,
  emptyReview,
  isApproved,
  readiness,
  readinessFromError,
  toolLabel,
  type PlanReview,
} from "./model";

function action(id: string, overrides: Partial<PlanAction["preview"]> = {}, state: PlanAction["state"] = { state: "pending" }): PlanAction {
  return {
    id,
    rationale: "because",
    state,
    preview: {
      token: `tok-${id}`,
      action: { type: "terminateProcess", target: { pid: 4821, startTime: 1 } },
      origin: { type: "user" },
      title: "Quit hog (PID 4821)",
      description: "Sends SIGTERM",
      targets: [],
      impact: [],
      estimatedBytesFreed: null,
      risk: "high",
      reversibility: { type: "irreversible" },
      warnings: [],
      requiresElevation: false,
      createdAtMs: 0,
      expiresAtMs: 1,
      ...overrides,
    },
  };
}

const firewall = action("fw", {
  risk: "critical",
  action: { type: "addFirewallRule", target: { type: "remoteIp", ip: "203.0.113.9" }, direction: "both" },
  title: "Block 203.0.113.9",
});

const plan: Plan = {
  id: "p",
  conversationId: "c",
  request: "r",
  explanation: "e",
  actions: [action("a"), action("b"), firewall, action("done", {}, { state: "rejected" })],
  status: "awaitingReview",
  createdAtMs: 0,
};

describe("plan review", () => {
  it("counts only approved pending actions and requires typing for critical ones", () => {
    const review: PlanReview = { decisions: { a: "approve", b: "reject", fw: "approve", done: "approve" }, confirmations: {} };
    expect(isApproved(firewall, review)).toBe(false);
    expect(approvedCount(plan, review)).toBe(1);
    const confirmed = { ...review, confirmations: { fw: " 203.0.113.9 " } };
    expect(approvedCount(plan, confirmed)).toBe(2);
    expect(decisionsFor(plan, confirmed)).toEqual([
      { actionId: "a", decision: "approve" },
      { actionId: "b", decision: "reject" },
      { actionId: "fw", decision: "approve" },
    ]);
  });

  it("rejects everything that was not reviewed", () => {
    expect(decisionsFor(plan, emptyReview).every((d) => d.decision === "reject")).toBe(true);
    expect(approvedCount(plan, emptyReview)).toBe(0);
  });
});

const settings: AgentSettings = {
  activeProvider: "ollama",
  selectedModels: { ollama: "llama3.1:8b" },
  ollamaBaseUrl: "http://127.0.0.1:11434",
  maxToolRounds: 12,
};

function ollama(overrides: Partial<OllamaStatus>): OllamaStatus {
  return { installed: true, running: true, version: "0.33.3", baseUrl: settings.ollamaBaseUrl, models: [], ...overrides };
}

describe("readiness", () => {
  const base = { settings, descriptors: null, statuses: null, models: null };

  it("walks the local Ollama checks in order", () => {
    expect(readiness({ ...base, ollama: null }).state).toBe("checking");
    expect(readiness({ ...base, ollama: ollama({ installed: false, running: false }) }).state).toBe("ollamaMissing");
    expect(readiness({ ...base, ollama: ollama({ running: false }) }).state).toBe("ollamaStopped");
    expect(readiness({ ...base, ollama: ollama({}) })).toEqual({ state: "modelMissing", model: "llama3.1:8b" });
    const model = { id: "llama3.1:8b", displayName: "llama3.1:8b", supportsTools: false, contextWindow: null, recommended: true };
    expect(readiness({ ...base, ollama: ollama({ models: [model] }) }).state).toBe("noTools");
    expect(readiness({ ...base, ollama: ollama({ models: [{ ...model, supportsTools: true }] }) }).state).toBe("ready");
  });

  it("requires a key for cloud providers", () => {
    const cloud = { ...settings, activeProvider: "gemini" as const };
    const statuses = [{ id: "gemini" as const, key: { state: "missing" as const }, ready: false }];
    expect(readiness({ ...base, settings: cloud, ollama: null, statuses })).toEqual({ state: "missingKey", provider: "gemini" });
    const valid = [{ id: "gemini" as const, key: { state: "valid" as const, checkedAtMs: 1 }, ready: true }];
    expect(readiness({ ...base, settings: cloud, ollama: null, statuses: valid }).state).toBe("ready");
  });

  it("recognizes readiness problems in send errors", () => {
    expect(
      readinessFromError({ code: "unavailable", feature: "Gemini API key", reason: "r", message: "m" }, { ...settings, activeProvider: "gemini" }, "m"),
    ).toEqual({ state: "missingKey", provider: "gemini" });
    expect(readinessFromError({ code: "unavailable", feature: "Ollama", reason: "r", message: "m" }, settings, "m")?.state).toBe(
      "ollamaStopped",
    );
    expect(
      readinessFromError(
        { code: "provider", provider: "Ollama (local)", status: 404, detail: "Run `ollama pull x`", message: "m" },
        settings,
        "x",
      ),
    ).toEqual({ state: "modelMissing", model: "x" });
    expect(readinessFromError({ code: "cancelled", message: "m" }, settings, "m")).toBeNull();
  });
});

describe("toolLabel", () => {
  it("describes known tools and falls back to the raw name", () => {
    expect(toolLabel("list_processes", false)).toBe("Listing processes");
    expect(toolLabel("move_to_trash", true)).toBe("Proposed Move to Trash");
    expect(toolLabel("rm_rf", true)).toBe("rm_rf");
  });
});
