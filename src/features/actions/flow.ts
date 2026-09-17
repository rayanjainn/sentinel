// The single confirm flow for every destructive action:
// prepareAction → confirm dialog → commitAction (or rejectAction) → outcome.
import { create } from "zustand";

import type { Action } from "../../bindings/Action";
import type { ActionOutcome } from "../../bindings/ActionOutcome";
import type { ActionPreview } from "../../bindings/ActionPreview";
import { isCode } from "../../lib/errors";
import { api } from "../../lib/ipc";
import { toast } from "../../stores/toasts";
import { doneLabel, isExpired } from "./labels";

export type FlowState =
  | { phase: "idle" }
  | { phase: "preparing"; action: Action; refreshed: boolean }
  | { phase: "confirm"; preview: ActionPreview; refreshed: boolean }
  | { phase: "committing"; preview: ActionPreview }
  | { phase: "outcome"; outcome: ActionOutcome }
  | { phase: "error"; action: Action; error: unknown; stage: "prepare" | "commit" };

export const useActionFlow = create<{ state: FlowState }>(() => ({ state: { phase: "idle" } }));

const setState = (state: FlowState) => useActionFlow.setState({ state });
const getState = () => useActionFlow.getState().state;

let pending: { resolve: (outcome: ActionOutcome | null) => void } | null = null;
let generation = 0;

function settle(outcome: ActionOutcome | null) {
  const p = pending;
  pending = null;
  p?.resolve(outcome);
}

async function prepare(action: Action, refreshed: boolean): Promise<void> {
  const gen = ++generation;
  setState({ phase: "preparing", action, refreshed });
  try {
    const preview = await api.prepareAction(action);
    if (gen !== generation) {
      void api.rejectAction(preview.token).catch(() => undefined);
      return;
    }
    setState({ phase: "confirm", preview, refreshed });
  } catch (error) {
    if (gen === generation) setState({ phase: "error", action, error, stage: "prepare" });
  }
}

/**
 * Starts the confirm flow for `action`. Resolves with the outcome once committed, or null when the
 * user cancels. A flow already in progress is cancelled first.
 */
export function runAction(action: Action): Promise<ActionOutcome | null> {
  const current = getState();
  if (current.phase === "committing") {
    return Promise.resolve(null);
  }
  if (current.phase !== "idle") cancelAction();
  return new Promise((resolve) => {
    pending = { resolve };
    void prepare(action, false);
  });
}

export async function confirmAction(): Promise<void> {
  const state = getState();
  if (state.phase !== "confirm") return;
  const { preview } = state;
  if (isExpired(preview)) {
    await prepare(preview.action, true);
    return;
  }
  setState({ phase: "committing", preview });
  try {
    const outcome = await api.commitAction(preview.token);
    settle(outcome);
    if (outcome.status === "succeeded") {
      setState({ phase: "idle" });
      toast({
        kind: "success",
        title: doneLabel(outcome.action),
        detail: outcome.summary,
        action: { label: "View details", onClick: () => showOutcome(outcome) },
      });
    } else {
      setState({ phase: "outcome", outcome });
    }
  } catch (error) {
    if (isCode(error, "actionTokenInvalid")) {
      await prepare(preview.action, true);
      return;
    }
    setState({ phase: "error", action: preview.action, error, stage: "commit" });
  }
}

export function cancelAction(): void {
  const state = getState();
  if (state.phase === "committing") return;
  generation += 1;
  if (state.phase === "confirm") {
    // Rejection is recorded in the audit log; an already-expired token has nothing left to reject.
    void api.rejectAction(state.preview.token).catch(() => undefined);
  }
  setState({ phase: "idle" });
  settle(null);
}

export function retryAction(): void {
  const state = getState();
  if (state.phase === "error") void prepare(state.action, false);
}

export function showOutcome(outcome: ActionOutcome): void {
  if (getState().phase !== "idle") return;
  setState({ phase: "outcome", outcome });
}

export function closeOutcome(): void {
  if (getState().phase === "outcome") setState({ phase: "idle" });
}
