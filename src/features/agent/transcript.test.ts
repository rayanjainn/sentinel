import { describe, expect, it } from "vitest";

import type { AgentEvent } from "../../bindings/AgentEvent";
import type { Plan } from "../../bindings/Plan";
import { addUserMessage, applyEvent, emptyChat, fromTranscript, pendingPlan, type ChatState } from "./transcript";

function plan(overrides: Partial<Plan> = {}): Plan {
  return {
    id: "plan-1",
    conversationId: "c1",
    request: "what's eating my CPU",
    explanation: "hog (PID 4821) is using 97% CPU.",
    actions: [],
    status: "awaitingReview",
    createdAtMs: 1,
    ...overrides,
  };
}

function run(events: AgentEvent[], start: ChatState = emptyChat): ChatState {
  return events.reduce(applyEvent, start);
}

describe("applyEvent", () => {
  it("merges streamed text and closes it at tool calls", () => {
    const state = run([
      { type: "turnStarted", turnId: "t", provider: "ollama", model: "llama3.1:8b" },
      { type: "textDelta", text: "Checking " },
      { type: "textDelta", text: "live data." },
      { type: "toolCallStarted", callId: "r1", name: "list_processes", access: "read", input: {} },
      { type: "toolCallFinished", callId: "r1", ok: true, summary: "412 processes" },
    ]);
    expect(state.running).toBe(true);
    expect(state.items).toEqual([
      { kind: "assistant", id: "assistant-1", text: "Checking live data.", streaming: false },
      { kind: "tool", id: "r1", name: "list_processes", access: "read", ok: true, summary: "412 processes" },
    ]);
  });

  it("moves the turn's explanation into the plan and updates plans in place", () => {
    let state = addUserMessage(emptyChat, "what's eating my CPU");
    state = run(
      [
        { type: "textDelta", text: "hog (PID 4821) is using 97% CPU." },
        { type: "toolCallStarted", callId: "w1", name: "terminate_process", access: "write", input: {} },
        { type: "planProposed", plan: plan() },
        { type: "turnFinished", stopReason: { type: "endTurn" } },
      ],
      state,
    );
    expect(state.items.map((i) => i.kind)).toEqual(["user", "tool", "plan"]);
    expect(pendingPlan(state)?.id).toBe("plan-1");

    state = applyEvent(state, { type: "planUpdated", plan: plan({ status: "completed" }) });
    expect(state.items.filter((i) => i.kind === "plan")).toHaveLength(1);
    expect(pendingPlan(state)).toBeNull();
    expect(state.running).toBe(false);
  });

  it("keeps earlier turns' text when a later plan arrives", () => {
    let state = addUserMessage(emptyChat, "hi");
    state = applyEvent(state, { type: "textDelta", text: "hog (PID 4821) is using 97% CPU." });
    state = applyEvent(state, { type: "turnFinished", stopReason: { type: "endTurn" } });
    state = addUserMessage(state, "fix it");
    state = applyEvent(state, { type: "planProposed", plan: plan() });
    expect(state.items.map((i) => i.kind)).toEqual(["user", "assistant", "user", "plan"]);
  });

  it("records errors and stops running", () => {
    const state = run([
      { type: "turnStarted", turnId: "t", provider: "gemini", model: "gemini-3.8-flash" },
      {
        type: "error",
        error: { code: "unavailable", feature: "tool calling", reason: "no tools", message: "unavailable" },
      },
    ]);
    expect(state.running).toBe(false);
    expect(state.items[0]?.kind).toBe("error");
  });
});

describe("fromTranscript", () => {
  it("restores the same shape as live events", () => {
    const state = fromTranscript([
      { type: "user", text: "what's eating my CPU", tsMs: 1 },
      { type: "assistant", text: "hog (PID 4821) is using 97% CPU.", tsMs: 2 },
      { type: "toolCall", callId: "w1", name: "terminate_process", access: "write", ok: true, summary: "Proposed" },
      { type: "plan", plan: plan() },
    ]);
    expect(state.items.map((i) => i.kind)).toEqual(["user", "tool", "plan"]);
  });
});
