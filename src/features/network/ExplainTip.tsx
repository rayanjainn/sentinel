// Per-connection "what is this" popover: the backend's plain-language explanation for one socket
// — who the other end is, what it's usually for, and whether the contents can be read. Never
// invents a tab or a URL; see crates/sentinel-core/src/service/explain.
import { Info } from "@phosphor-icons/react";

import type { ConnectionExplanation } from "../../bindings/ConnectionExplanation";
import { Tip } from "../../components/Tip";
import { cx } from "../../lib/cx";

const CONFIDENCE_LABEL: Record<ConnectionExplanation["confidence"], string> = {
  known: "Known",
  likely: "Looks like",
  unknown: "Not recognised",
};

export function ExplainTip({ explanation, className }: { explanation: ConnectionExplanation; className?: string }) {
  return (
    <Tip
      label={`What is this connection? ${explanation.headline}`}
      icon={<Info size={13} weight="bold" />}
      className={className}
      width={288}
      content={
        <>
          <div className="flex items-baseline justify-between gap-3">
            <span className="font-medium text-fg">{explanation.headline}</span>
            <span
              className={cx(
                "shrink-0 text-[11px]",
                explanation.confidence === "unknown" ? "text-fg-subtle" : "text-fg-muted",
              )}
            >
              {CONFIDENCE_LABEL[explanation.confidence]}
            </span>
          </div>
          <span className="text-fg-muted">{explanation.detail}</span>
        </>
      }
    />
  );
}
