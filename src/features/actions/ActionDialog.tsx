import { X } from "@phosphor-icons/react";
import { useEffect, useId, useRef, useState } from "react";

import type { ActionPreview } from "../../bindings/ActionPreview";
import { Button, IconButton } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { toPayload } from "../../lib/errors";
import { ActionPreviewBody } from "./ActionPreviewBody";
import { cancelAction, closeOutcome, confirmAction, retryAction, useActionFlow } from "./flow";
import { confirmationPhrase, confirmLabel, isExpired, phraseMatches } from "./labels";
import { OutcomeSummary } from "./OutcomeSummary";

/** Retrying cannot help when the target is gone or the request itself was invalid. */
function isRetryable(error: unknown): boolean {
  const code = toPayload(error).code;
  return !["pathNotFound", "processNotFound", "processChanged", "invalidInput", "unavailable"].includes(code);
}

function Countdown({ preview }: { preview: ActionPreview }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, []);
  if (isExpired(preview, now)) {
    return <span className="text-[12px] text-warn">Preview expired. Confirming prepares it again first.</span>;
  }
  const secs = Math.ceil((preview.expiresAtMs - now) / 1000);
  if (secs > 120) return null;
  return (
    <span className="text-[12px] text-fg-subtle">
      Preview valid for <span className="num">{Math.floor(secs / 60)}:{String(secs % 60).padStart(2, "0")}</span>
    </span>
  );
}

function PreparingBody() {
  return (
    <div className="flex flex-col gap-4" role="status" aria-label="Preparing preview">
      <Skeleton className="h-2.5 w-24" />
      <Skeleton className="h-3 w-full" />
      <Skeleton className="h-3 w-4/5" />
      <Skeleton className="h-16 w-full" />
      <div className="grid grid-cols-3 gap-6">
        <Skeleton className="h-8" />
        <Skeleton className="h-8" />
        <Skeleton className="h-8" />
      </div>
    </div>
  );
}

/** Host for the shared action flow. Mounted once at the app root. */
export function ActionDialog() {
  const state = useActionFlow((s) => s.state);
  const titleId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);
  const token = state.phase === "confirm" || state.phase === "committing" ? state.preview.token : null;
  // Typed confirmation belongs to one preview token; a refreshed preview starts empty.
  const [typedFor, setTypedFor] = useState<{ token: string | null; value: string }>({ token: null, value: "" });
  const typed = typedFor.token === token ? typedFor.value : "";
  const setTyped = (value: string) => setTypedFor({ token, value });

  const open = state.phase !== "idle";
  const dismissable = state.phase !== "committing";
  const onClose = () => (state.phase === "outcome" ? closeOutcome() : cancelAction());

  let title = "";
  if (state.phase === "preparing" || state.phase === "error") title = confirmLabel(state.action);
  if (state.phase === "confirm" || state.phase === "committing") title = state.preview.title;
  if (state.phase === "outcome") title = state.outcome.status === "succeeded" ? "Completed" : state.outcome.status === "failed" ? "Did not complete" : "Partly completed";

  const preview = state.phase === "confirm" || state.phase === "committing" ? state.preview : null;
  const critical = preview?.risk === "critical";
  const phrase = preview && critical ? confirmationPhrase(preview) : null;
  const phraseOk = !phrase || phraseMatches(phrase, typed);
  const destructive = preview ? preview.risk !== "moderate" : false;

  return (
    <Dialog open={open} onClose={onClose} labelledBy={titleId} dismissable={dismissable} width={580} initialFocus={cancelRef}>
      <div className="flex items-start justify-between gap-4 border-b border-line px-5 pb-3 pt-4">
        <h2 id={titleId} className="text-[15px] font-semibold tracking-display text-fg">
          {title}
        </h2>
        {dismissable && (
          <IconButton label="Close" size="sm" onClick={onClose} className="-mr-1.5">
            <X size={14} />
          </IconButton>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        {state.phase === "preparing" && <PreparingBody />}
        {preview && (
          <div className={state.phase === "committing" ? "pointer-events-none opacity-60 transition-opacity" : undefined}>
            {state.phase === "confirm" && state.refreshed && (
              <p className="mb-4 rounded-[6px] border border-warn/30 bg-warn/10 px-3 py-2 text-[12px] text-fg">
                The earlier preview expired, so it was prepared again against the current system. Review it before
                confirming.
              </p>
            )}
            <ActionPreviewBody preview={preview} />
            {phrase && (
              <label className="mt-5 flex flex-col gap-1.5">
                <span className="text-[12px] text-fg-muted">
                  Type <span className="num text-fg">{phrase}</span> to confirm
                </span>
                <input
                  value={typed}
                  onChange={(e) => setTyped(e.target.value)}
                  spellCheck={false}
                  autoComplete="off"
                  className="num selectable h-8 rounded-[5px] border border-line bg-sunken px-2.5 text-[13px] text-fg outline-none focus:border-danger/60"
                />
              </label>
            )}
          </div>
        )}
        {state.phase === "outcome" && <OutcomeSummary outcome={state.outcome} />}
        {state.phase === "error" && (
          <ErrorState
            error={state.error}
            subject={state.stage === "prepare" ? "Preparing this action" : "This action"}
            onRetry={isRetryable(state.error) ? retryAction : undefined}
          />
        )}
      </div>

      <div className="flex items-center justify-between gap-3 border-t border-line bg-panel px-5 py-3">
        <div className="min-w-0">{state.phase === "confirm" && <Countdown preview={state.preview} />}</div>
        <div className="flex items-center gap-2">
          {state.phase === "outcome" || state.phase === "error" ? (
            <Button ref={cancelRef} variant="secondary" onClick={onClose}>
              {state.phase === "outcome" ? "Done" : "Close"}
            </Button>
          ) : (
            <>
              <Button ref={cancelRef} variant="ghost" onClick={cancelAction} disabled={state.phase === "committing"}>
                Cancel
              </Button>
              <Button
                variant={destructive ? "danger" : "primary"}
                disabled={state.phase !== "confirm" || !phraseOk}
                onClick={() => void confirmAction()}
              >
                {state.phase === "committing"
                  ? "Working…"
                  : preview
                    ? confirmLabel(preview.action)
                    : state.phase === "preparing"
                      ? confirmLabel(state.action)
                      : ""}
              </Button>
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}
