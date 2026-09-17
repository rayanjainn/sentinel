import { useId, useState } from "react";
import { create } from "zustand";

import type { ProcessInfo } from "../../bindings/ProcessInfo";
import { Button } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import { cx } from "../../lib/cx";
import { formatNice } from "../../lib/format";
import { runAction } from "../actions";

const usePriorityTarget = create<{ process: ProcessInfo | null }>(() => ({ process: null }));

export function openPriorityDialog(process: ProcessInfo) {
  usePriorityTarget.setState({ process });
}

const PRESETS = [
  { label: "Lowest", nice: 19 },
  { label: "Low", nice: 10 },
  { label: "Normal", nice: 0 },
  { label: "High", nice: -10 },
  { label: "Highest", nice: -20 },
];

function describeNice(nice: number): string {
  if (nice === 0) return "Normal scheduling priority.";
  if (nice > 0) return "Runs after other work, leaving more CPU for everything else.";
  return "Runs ahead of other work. Raising priority needs administrator authorization.";
}

function PriorityBody({ process, onClose }: { process: ProcessInfo; onClose: () => void }) {
  const titleId = useId();
  const current = process.nice ?? 0;
  const [nice, setNice] = useState(current);
  return (
    <>
      <div className="border-b border-line px-5 pb-3 pt-4">
        <h2 id={titleId} className="text-[15px] font-semibold tracking-display">
          Change priority of {process.name}
        </h2>
        <p className="mt-0.5 text-[12px] text-fg-muted">
          PID <span className="num">{process.pid}</span>, currently <span className="num">{formatNice(process.nice)}</span>
        </p>
      </div>
      <div className="flex flex-col gap-4 px-5 py-4">
        <div className="flex items-baseline justify-between">
          <span className="text-fg-muted">Nice value</span>
          <span className="num text-[20px] text-fg">{formatNice(nice)}</span>
        </div>
        <input
          type="range"
          min={-20}
          max={19}
          step={1}
          value={-nice}
          aria-label="Priority"
          aria-valuetext={`Nice ${formatNice(nice)}`}
          onChange={(e) => setNice(-Number(e.target.value))}
          className="w-full accent-[var(--signal)]"
        />
        <div className="flex justify-between text-[11px] text-fg-subtle">
          <span>Lower priority</span>
          <span>Higher priority</span>
        </div>
        <div className="flex flex-wrap gap-1.5">
          {PRESETS.map((p) => (
            <button
              key={p.label}
              type="button"
              onClick={() => setNice(p.nice)}
              className={cx(
                "h-6 rounded-[5px] border px-2 text-[12px]",
                nice === p.nice ? "border-signal/50 bg-signal/10 text-fg" : "border-line text-fg-muted hover:text-fg",
              )}
            >
              {p.label}
            </button>
          ))}
        </div>
        <p className="text-[12px] text-fg-muted">{describeNice(nice)}</p>
      </div>
      <div className="flex justify-end gap-2 border-t border-line px-5 py-3">
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button
          variant="primary"
          disabled={nice === current}
          onClick={() => {
            onClose();
            void runAction({ type: "setProcessPriority", target: { pid: process.pid, startTime: process.startTime }, nice });
          }}
        >
          Preview change
        </Button>
      </div>
    </>
  );
}

export function PriorityDialog() {
  const process = usePriorityTarget((s) => s.process);
  const close = () => usePriorityTarget.setState({ process: null });
  return (
    <Dialog open={process !== null} onClose={close} labelledBy="priority-dialog" width={420}>
      {process && (
        <div aria-labelledby="priority-dialog">
          <PriorityBody key={`${process.pid}:${process.startTime}`} process={process} onClose={close} />
        </div>
      )}
    </Dialog>
  );
}
