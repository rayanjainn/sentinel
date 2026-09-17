import { ArrowsClockwise, Check, PencilSimple, X } from "@phosphor-icons/react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";

import type { Plan } from "../../bindings/Plan";
import type { PlanAction } from "../../bindings/PlanAction";
import { Button } from "../../components/Button";
import { ErrorState } from "../../components/States";
import { cx } from "../../lib/cx";
import { pluralize } from "../../lib/format";
import { springInteraction, usePrefersReducedMotion } from "../../lib/motion";
import { ActionPreviewBody, OutcomeSummary } from "../actions";
import { confirmationPhrase, confirmLabel, isExpired, phraseMatches } from "../actions/labels";
import { EditActionForm, isEditable } from "./EditActionForm";
import { Markdown } from "./Markdown";
import { approvedCount, emptyReview, needsTypedConfirmation, type PlanReview } from "./model";
import { useAgent } from "./store";

/** Re-renders periodically so preview expiry shows up without interaction. */
function useNow(intervalMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return now;
}

function Running() {
  return (
    <div className="flex items-center gap-2 text-[12px] text-fg">
      <span className="relative flex size-2">
        <span className="live-ping absolute inset-0 rounded-full bg-signal" />
        <span className="size-2 rounded-full bg-signal" />
      </span>
      Running
    </div>
  );
}

function Decision({ plan, item, review, now }: { plan: Plan; item: PlanAction; review: PlanReview; now: number }) {
  const decide = useAgent((s) => s.decide);
  const confirm = useAgent((s) => s.confirm);
  const revise = useAgent((s) => s.revise);
  const [editing, setEditing] = useState(false);
  const [refreshError, setRefreshError] = useState<unknown>(null);
  const [refreshing, setRefreshing] = useState(false);
  const decision = review.decisions[item.id];
  const phrase = confirmationPhrase(item.preview);
  const typed = review.confirmations[item.id] ?? "";

  if (isExpired(item.preview, now)) {
    return (
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between gap-3">
          <span className="text-[12px] text-fg-muted">This preview expired. Refresh it to check the target again.</span>
          <Button
            size="sm"
            icon={<ArrowsClockwise size={13} />}
            disabled={refreshing}
            onClick={async () => {
              setRefreshing(true);
              setRefreshError(await revise(plan.id, item.id, item.preview.action));
              setRefreshing(false);
            }}
          >
            {refreshing ? "Refreshing preview" : "Refresh preview"}
          </Button>
        </div>
        {refreshError !== null && <ErrorState error={refreshError} compact />}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-wrap items-center gap-1.5">
        <Button
          size="sm"
          variant={decision === "approve" ? "primary" : "secondary"}
          aria-pressed={decision === "approve"}
          icon={<Check size={13} weight="bold" />}
          onClick={() => decide(plan.id, item.id, decision === "approve" ? null : "approve")}
        >
          {decision === "approve" ? "Approved" : "Approve"}
        </Button>
        <Button
          size="sm"
          variant={decision === "reject" ? "dangerQuiet" : "ghost"}
          aria-pressed={decision === "reject"}
          icon={<X size={13} weight="bold" />}
          onClick={() => decide(plan.id, item.id, decision === "reject" ? null : "reject")}
          className={cx(decision === "reject" && "bg-danger/10")}
        >
          {decision === "reject" ? "Rejected" : "Reject"}
        </Button>
        {isEditable(item.preview.action) && (
          <Button size="sm" variant="ghost" icon={<PencilSimple size={13} />} onClick={() => setEditing((v) => !v)} aria-expanded={editing}>
            Edit
          </Button>
        )}
      </div>
      {decision === "approve" && needsTypedConfirmation(item) && (
        <label className="flex flex-col gap-1.5">
          <span className="text-[12px] text-fg-muted">
            Type <span className="num text-fg">{phrase}</span> to confirm this firewall change
          </span>
          <input
            value={typed}
            onChange={(e) => confirm(plan.id, item.id, e.target.value)}
            spellCheck={false}
            autoComplete="off"
            className={cx(
              "num selectable h-7 rounded-[5px] border bg-sunken px-2.5 text-[12px] text-fg focus:outline-none",
              phraseMatches(phrase, typed) ? "border-signal/60" : "border-line focus:border-line-strong",
            )}
          />
        </label>
      )}
      {editing && <EditActionForm planId={plan.id} item={item} onDone={() => setEditing(false)} />}
    </div>
  );
}

function ActionCard({ plan, item, review, now }: { plan: Plan; item: PlanAction; review: PlanReview; now: number }) {
  const awaiting = plan.status === "awaitingReview" && item.state.state === "pending";
  const decision = review.decisions[item.id];
  const state = item.state;
  return (
    <motion.li
      layout="position"
      className={cx(
        "flex flex-col gap-3 rounded-[8px] border bg-raised p-3.5 transition-[border-color,opacity] duration-200",
        awaiting && decision === "approve" ? "border-signal/45" : "border-line",
        (state.state === "rejected" || (awaiting && decision === "reject")) && "opacity-60",
      )}
    >
      <ActionPreviewBody preview={item.preview} showTitle />
      {item.rationale && (
        <p className="selectable text-[12px] text-fg-muted">
          <span className="text-fg-subtle">Why: </span>
          {item.rationale}
        </p>
      )}
      <div className="border-t border-line pt-3">
        {awaiting && <Decision plan={plan} item={item} review={review} now={now} />}
        {state.state === "pending" && !awaiting && <span className="text-[12px] text-fg-muted">Waiting to run</span>}
        {state.state === "executing" && <Running />}
        {(state.state === "succeeded" || state.state === "partiallySucceeded") && <OutcomeSummary outcome={state.outcome} />}
        {state.state === "failed" && <ErrorState error={state.error} compact />}
        {state.state === "rejected" && <span className="text-[12px] text-fg-muted">Not run. You declined this change.</span>}
        {state.state === "expired" && (
          <span className="text-[12px] text-fg-muted">Not run. The preview expired before it was approved.</span>
        )}
      </div>
    </motion.li>
  );
}

export function PlanSection({ plan }: { plan: Plan }) {
  const review = useAgent((s) => s.reviews[plan.id] ?? emptyReview);
  const running = useAgent((s) => s.runningPlans[plan.id] ?? false);
  const error = useAgent((s) => s.planErrors[plan.id]);
  const runPlan = useAgent((s) => s.runPlan);
  const reduced = usePrefersReducedMotion();
  const now = useNow(15_000);
  const awaiting = plan.status === "awaitingReview";
  const approved = approvedCount(plan, review);
  const pending = plan.actions.filter((a) => a.state.state === "pending").length;
  const unconfirmed = plan.actions.some(
    (a) => review.decisions[a.id] === "approve" && needsTypedConfirmation(a) && !phraseMatches(confirmationPhrase(a.preview), review.confirmations[a.id] ?? ""),
  );
  const summary = awaiting
    ? `${plan.actions.length} ${pluralize(plan.actions.length, "change")} waiting for your review`
    : plan.status === "executing"
      ? "Running approved changes"
      : "Finished";
  const onlyAction = plan.actions.length === 1 ? plan.actions[0] : undefined;

  return (
    <motion.section
      aria-label="Proposed plan"
      initial={reduced ? false : { opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={springInteraction}
      className="flex flex-col gap-3"
    >
      <div className="flex items-baseline justify-between gap-3">
        <h3 className="text-[13px] font-semibold text-fg">Proposed plan</h3>
        <span className="text-[12px] text-fg-muted">{summary}</span>
      </div>
      {plan.explanation && <Markdown text={plan.explanation} />}
      <ul className="flex flex-col gap-2.5">
        <AnimatePresence initial={false}>
          {plan.actions.map((item) => (
            <ActionCard key={item.id} plan={plan} item={item} review={review} now={now} />
          ))}
        </AnimatePresence>
      </ul>
      {awaiting && pending > 0 && (
        <div className="sticky bottom-0 flex items-center justify-between gap-3 border-t border-line bg-panel py-2.5">
          <span className="text-[12px] text-fg-muted">
            {unconfirmed ? "Type the target to confirm firewall changes." : "Only approved changes run. The rest are declined."}
          </span>
          <Button variant="primary" disabled={approved === 0 || running} onClick={() => void runPlan(plan)}>
            {running
              ? "Running"
              : approved === 1 && onlyAction
                ? confirmLabel(onlyAction.preview.action)
                : `Run ${approved} approved ${pluralize(approved, "change")}`}
          </Button>
        </div>
      )}
      {error && <ErrorState error={error} compact />}
    </motion.section>
  );
}
