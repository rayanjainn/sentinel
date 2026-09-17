// Folds `sentinel:agent` events and restored transcripts into the items the chat panel renders.
// Pure so the streaming rules are unit-tested without a webview.
import type { AgentEvent } from "../../bindings/AgentEvent";
import type { ErrorPayload } from "../../bindings/ErrorPayload";
import type { Plan } from "../../bindings/Plan";
import type { ToolAccess } from "../../bindings/ToolAccess";
import type { TranscriptItem } from "../../bindings/TranscriptItem";

export type ChatItem =
  | { kind: "user"; id: string; text: string }
  | { kind: "assistant"; id: string; text: string; streaming: boolean }
  | { kind: "tool"; id: string; name: string; access: ToolAccess; ok: boolean | null; summary: string | null }
  | { kind: "plan"; id: string; plan: Plan }
  | { kind: "error"; id: string; error: ErrorPayload };

export interface ChatState {
  items: ChatItem[];
  running: boolean;
  provider: string | null;
  model: string | null;
  /** Monotonic counter for synthetic item ids. */
  seq: number;
}

export const emptyChat: ChatState = { items: [], running: false, provider: null, model: null, seq: 0 };

function closeStreaming(items: ChatItem[]): ChatItem[] {
  const last = items[items.length - 1];
  if (last?.kind === "assistant" && last.streaming) {
    return [...items.slice(0, -1), { ...last, streaming: false }];
  }
  return items;
}

/**
 * A plan's explanation is the assistant text of the turn that produced it, so that text moves into
 * the plan section instead of appearing twice.
 */
function withPlan(items: ChatItem[], plan: Plan): ChatItem[] {
  const existing = items.findIndex((i) => i.kind === "plan" && i.id === plan.id);
  if (existing >= 0) {
    return items.map((item, index) => (index === existing ? { kind: "plan", id: plan.id, plan } : item));
  }
  let turnStart = 0;
  for (let i = items.length - 1; i >= 0; i -= 1) {
    if (items[i]?.kind === "user") {
      turnStart = i + 1;
      break;
    }
  }
  const explanation = plan.explanation.trim();
  const kept = items.filter(
    (item, index) => !(index >= turnStart && item.kind === "assistant" && explanation.includes(item.text.trim())),
  );
  return [...kept, { kind: "plan", id: plan.id, plan }];
}

export function applyEvent(state: ChatState, event: AgentEvent): ChatState {
  switch (event.type) {
    case "turnStarted":
      return { ...state, running: true, provider: event.provider, model: event.model };
    case "textDelta": {
      const last = state.items[state.items.length - 1];
      if (last?.kind === "assistant" && last.streaming) {
        return { ...state, items: [...state.items.slice(0, -1), { ...last, text: last.text + event.text }] };
      }
      const seq = state.seq + 1;
      return {
        ...state,
        seq,
        items: [...state.items, { kind: "assistant", id: `assistant-${seq}`, text: event.text, streaming: true }],
      };
    }
    case "toolCallStarted":
      return {
        ...state,
        items: [
          ...closeStreaming(state.items),
          { kind: "tool", id: event.callId, name: event.name, access: event.access, ok: null, summary: null },
        ],
      };
    case "toolCallFinished":
      return {
        ...state,
        items: state.items.map((item) =>
          item.kind === "tool" && item.id === event.callId ? { ...item, ok: event.ok, summary: event.summary } : item,
        ),
      };
    case "planProposed":
    case "planUpdated":
      return { ...state, items: withPlan(closeStreaming(state.items), event.plan) };
    case "turnFinished":
      return { ...state, running: false, items: closeStreaming(state.items) };
    case "error": {
      const seq = state.seq + 1;
      return {
        ...state,
        seq,
        running: false,
        items: [...closeStreaming(state.items), { kind: "error", id: `error-${seq}`, error: event.error }],
      };
    }
  }
}

export function addUserMessage(state: ChatState, text: string): ChatState {
  const seq = state.seq + 1;
  return { ...state, seq, items: [...state.items, { kind: "user", id: `user-${seq}`, text }] };
}

export function fromTranscript(transcript: TranscriptItem[]): ChatState {
  let state: ChatState = { ...emptyChat };
  for (const entry of transcript) {
    switch (entry.type) {
      case "user":
        state = addUserMessage(state, entry.text);
        break;
      case "assistant": {
        const seq = state.seq + 1;
        state = {
          ...state,
          seq,
          items: [...state.items, { kind: "assistant", id: `assistant-${seq}`, text: entry.text, streaming: false }],
        };
        break;
      }
      case "toolCall":
        state = {
          ...state,
          items: [
            ...state.items,
            { kind: "tool", id: entry.callId, name: entry.name, access: entry.access, ok: entry.ok, summary: entry.summary },
          ],
        };
        break;
      case "plan":
        state = { ...state, items: withPlan(state.items, entry.plan) };
        break;
    }
  }
  return state;
}

/** The most recent plan still waiting for decisions, if any. */
export function pendingPlan(state: ChatState): Plan | null {
  for (let i = state.items.length - 1; i >= 0; i -= 1) {
    const item = state.items[i];
    if (item?.kind === "plan" && item.plan.status === "awaitingReview") return item.plan;
  }
  return null;
}
