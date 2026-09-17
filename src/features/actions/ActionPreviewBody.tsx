import { ArrowCounterClockwise, LockKey, ShieldWarning, Warning, WarningOctagon } from "@phosphor-icons/react";
import { useState } from "react";

import type { ActionPreview } from "../../bindings/ActionPreview";
import type { PreviewTarget } from "../../bindings/PreviewTarget";
import { cx } from "../../lib/cx";
import { formatBytes, formatMetric } from "../../lib/format";
import { riskLabel } from "./labels";

const TARGET_LIMIT = 40;

function looksLikePath(label: string): boolean {
  return /^(\/|~|[A-Za-z]:\\|\\\\)/.test(label);
}

function TargetRow({ target }: { target: PreviewTarget }) {
  return (
    <li className="flex flex-col gap-0.5 px-3 py-2">
      <div className="flex items-baseline justify-between gap-4">
        <span className={cx("selectable min-w-0 break-all text-fg", looksLikePath(target.label) ? "num text-[12px]" : "text-[13px]")}>
          {target.label}
        </span>
        {target.sizeBytes !== null && (
          <span className="num shrink-0 text-[12px] text-fg-muted">{formatBytes(target.sizeBytes, { base: 1000 })}</span>
        )}
      </div>
      {target.detail && <span className="selectable break-all text-[12px] text-fg-muted">{target.detail}</span>}
      {target.problem && (
        <span className="flex items-center gap-1.5 text-[12px] text-warn">
          <Warning size={12} weight="bold" />
          {target.problem}. It will be skipped.
        </span>
      )}
    </li>
  );
}

/**
 * Everything a person needs to judge an action before it runs. Prop-driven so the agent's plan
 * cards can render the same body as the confirm dialog.
 */
export function ActionPreviewBody({
  preview,
  showTitle = false,
  showRisk = true,
  className,
}: {
  preview: ActionPreview;
  showTitle?: boolean;
  showRisk?: boolean;
  className?: string;
}) {
  const [showAll, setShowAll] = useState(false);
  const targets = showAll ? preview.targets : preview.targets.slice(0, TARGET_LIMIT);
  const hidden = preview.targets.length - targets.length;
  const RiskIcon = preview.risk === "critical" ? WarningOctagon : preview.risk === "high" ? ShieldWarning : null;

  return (
    <div className={cx("flex flex-col gap-4", className)}>
      {(showTitle || showRisk) && (
        <div className="flex flex-col gap-1.5">
          {showRisk && (
            <span
              className={cx(
                "inline-flex items-center gap-1.5 text-[12px] font-medium",
                preview.risk === "moderate" && "text-fg-muted",
                preview.risk === "high" && "text-warn",
                preview.risk === "critical" && "text-danger",
              )}
            >
              {RiskIcon && <RiskIcon size={13} weight="fill" />}
              {riskLabel(preview.risk)}
            </span>
          )}
          {showTitle && <h3 className="text-[15px] font-semibold tracking-display text-fg">{preview.title}</h3>}
        </div>
      )}

      <p className="selectable text-fg">{preview.description}</p>

      {preview.targets.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <span className="text-[12px] text-fg-muted">
            {preview.targets.length === 1 ? "Target" : `${preview.targets.length} targets`}
          </span>
          <ul className="max-h-56 divide-y divide-line overflow-y-auto rounded-[6px] border border-line bg-sunken">
            {targets.map((t, i) => (
              <TargetRow key={`${t.label}-${i}`} target={t} />
            ))}
          </ul>
          {hidden > 0 && (
            <button type="button" className="self-start text-[12px] text-fg-muted hover:text-fg" onClick={() => setShowAll(true)}>
              Show {hidden} more
            </button>
          )}
        </div>
      )}

      {preview.impact.length > 0 && (
        <dl className="grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-x-6 gap-y-3">
          {preview.impact.map((m) => (
            <div key={m.key} className="flex flex-col gap-0.5">
              <dt className="text-[12px] text-fg-muted">{m.label}</dt>
              <dd className="num text-[15px] text-fg">{formatMetric(m)}</dd>
            </div>
          ))}
        </dl>
      )}

      {preview.warnings.length > 0 && (
        <ul className="flex flex-col gap-1.5">
          {preview.warnings.map((w) => (
            <li key={w} className="flex items-start gap-2 text-warn">
              <Warning size={14} weight="bold" className="mt-0.5 shrink-0" />
              <span className="text-[13px]">{w}</span>
            </li>
          ))}
        </ul>
      )}

      <div className="flex flex-col gap-1.5 border-t border-line pt-3 text-[12px]">
        <span className={cx("flex items-start gap-2", preview.reversibility.type === "irreversible" ? "text-danger" : "text-fg-muted")}>
          <ArrowCounterClockwise size={13} className="mt-0.5 shrink-0" />
          {preview.reversibility.type === "recoverable" && <span>Recoverable: {preview.reversibility.how}</span>}
          {preview.reversibility.type === "undoable" && <span>Can be undone: {preview.reversibility.how}</span>}
          {preview.reversibility.type === "irreversible" && <span>Cannot be undone.</span>}
        </span>
        {preview.requiresElevation && (
          <span className="flex items-start gap-2 text-fg-muted">
            <LockKey size={13} className="mt-0.5 shrink-0" />
            <span>Administrator authorization will be requested when you confirm.</span>
          </span>
        )}
      </div>
    </div>
  );
}
