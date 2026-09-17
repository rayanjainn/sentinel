import { CheckCircle, WarningCircle, XCircle } from "@phosphor-icons/react";
import { useState } from "react";

import type { ActionOutcome } from "../../bindings/ActionOutcome";
import type { Metric } from "../../bindings/Metric";
import { describeError } from "../../lib/errors";
import { cx } from "../../lib/cx";
import { formatDateTime, formatMetric } from "../../lib/format";

const ITEM_LIMIT = 30;

function pairMetrics(before: Metric[], after: Metric[]) {
  const keys = [...new Set([...before.map((m) => m.key), ...after.map((m) => m.key)])];
  return keys.map((key) => ({
    key,
    before: before.find((m) => m.key === key) ?? null,
    after: after.find((m) => m.key === key) ?? null,
  }));
}

/** Result of a committed action: headline, measured before/after metrics and per-item results. */
export function OutcomeSummary({ outcome, className }: { outcome: ActionOutcome; className?: string }) {
  const [showAll, setShowAll] = useState(false);
  const failures = outcome.items.filter((i) => !i.success).length;
  const items = showAll ? outcome.items : outcome.items.slice(0, ITEM_LIMIT);
  const metrics = pairMetrics(outcome.before, outcome.after);
  const Icon = outcome.status === "succeeded" ? CheckCircle : outcome.status === "failed" ? XCircle : WarningCircle;

  return (
    <div className={cx("flex flex-col gap-4", className)}>
      <div className="flex items-start gap-2.5">
        <Icon
          size={18}
          weight="fill"
          className={cx(
            "mt-px shrink-0",
            outcome.status === "succeeded" && "text-signal",
            outcome.status === "partiallySucceeded" && "text-warn",
            outcome.status === "failed" && "text-danger",
          )}
        />
        <div className="flex flex-col gap-0.5">
          <p className="selectable text-fg">{outcome.summary}</p>
          <span className="text-[12px] text-fg-subtle">
            {outcome.status === "partiallySucceeded" && `${failures} of ${outcome.items.length} items failed. `}
            Finished {formatDateTime(outcome.finishedAtMs)}
          </span>
        </div>
      </div>

      {metrics.length > 0 && (
        <table className="w-full text-left text-[12px]">
          <thead className="text-fg-muted">
            <tr className="border-b border-line">
              <th className="py-1.5 font-normal">Measured</th>
              <th className="py-1.5 text-right font-normal">Before</th>
              <th className="py-1.5 text-right font-normal">After</th>
            </tr>
          </thead>
          <tbody>
            {metrics.map((m) => (
              <tr key={m.key} className="border-b border-line last:border-0">
                <td className="py-1.5 text-fg">{(m.after ?? m.before)?.label}</td>
                <td className="num py-1.5 text-right text-fg-muted">{m.before ? formatMetric(m.before) : "—"}</td>
                <td className="num py-1.5 text-right text-fg">{m.after ? formatMetric(m.after) : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {outcome.items.length > 1 || failures > 0 ? (
        <div className="flex flex-col gap-1.5">
          <ul className="max-h-56 divide-y divide-line overflow-y-auto rounded-[6px] border border-line bg-sunken">
            {items.map((item, i) => (
              <li key={`${item.label}-${i}`} className="flex items-start gap-2 px-3 py-1.5">
                {item.success ? (
                  <CheckCircle size={13} weight="fill" className="mt-0.5 shrink-0 text-signal" />
                ) : (
                  <XCircle size={13} weight="fill" className="mt-0.5 shrink-0 text-danger" />
                )}
                <div className="flex min-w-0 flex-col">
                  <span className="selectable break-all text-[12px] text-fg">{item.label}</span>
                  {item.error && (
                    <span className="selectable text-[12px] text-fg-muted">
                      {describeError(item.error).title}. {describeError(item.error).detail}
                    </span>
                  )}
                </div>
              </li>
            ))}
          </ul>
          {outcome.items.length > items.length && (
            <button type="button" className="self-start text-[12px] text-fg-muted hover:text-fg" onClick={() => setShowAll(true)}>
              Show {outcome.items.length - items.length} more
            </button>
          )}
        </div>
      ) : null}
    </div>
  );
}
