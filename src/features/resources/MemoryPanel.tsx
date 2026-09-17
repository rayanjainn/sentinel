import { useMemo } from "react";

import type { ResourceSample } from "../../bindings/ResourceSample";
import { AnimatedNumber } from "../../components/AnimatedNumber";
import { LiveChart } from "../../components/charts/LiveChart";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState } from "../../components/States";
import { formatBytes, formatPercent } from "../../lib/format";
import { useProcesses, useProcessFeed } from "../../stores/processes";
import { useResources } from "../../stores/resources";
import { useSettings } from "../../stores/settings";
import { MEMORY_SERIES } from "../../styles/dataviz";
import { useProcessView } from "../processes/viewState";
import { availableSegments, memorySegments, type MemorySegmentKey } from "./memory";
import { resourceSource, sampleTime } from "./sources";

const TOP_COUNT = 8;

export function MemoryChart({ windowMs }: { windowMs: number }) {
  const latest = useResources((s) => s.latest);
  const intervalMs = useSettings((s) => s.intervalMs);
  const keysSignature = availableSegments(latest?.memory).join(",");
  const series = useMemo(
    () =>
      (keysSignature.split(",") as MemorySegmentKey[]).map((key) => ({
        key,
        label: MEMORY_SERIES[key].label,
        color: MEMORY_SERIES[key].fill,
        value: (d: ResourceSample) => memorySegments(d.memory)[key],
      })),
    [keysSignature],
  );

  if (!latest) return null;
  const m = latest.memory;
  const segments = memorySegments(m);
  const usedPct = m.total > 0 ? (m.used / m.total) * 100 : 0;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-2">
        <div className="flex items-baseline gap-2">
          <AnimatedNumber value={m.used} format={(v) => formatBytes(v)} className="text-[28px] font-semibold tracking-display text-fg" />
          <span className="text-fg-muted">
            of {formatBytes(m.total)} used, <AnimatedNumber value={usedPct} format={(v) => formatPercent(v, 0)} />
          </span>
        </div>
        <span className="text-[12px] text-fg-muted">
          <span className="num text-fg">{formatBytes(m.available)}</span> available
        </span>
      </div>
      <LiveChart
        source={resourceSource}
        series={series}
        time={sampleTime}
        windowMs={windowMs}
        intervalMs={intervalMs}
        height={150}
        yMax={m.total}
        stacked
        grid={[0.5, 1]}
        formatValue={(v) => formatBytes(v)}
        formatTick={(v) => formatBytes(v, { digits: 0 })}
        showTimeAxis
        ariaLabel="Memory composition over time"
      />
      <ul aria-label="Memory legend" className="flex flex-wrap gap-x-5 gap-y-1.5">
        {series.map((s) => (
          <li key={s.key} className="flex items-center gap-1.5 text-[12px]">
            <span className="size-2.5 rounded-[2px]" style={{ background: s.color, opacity: s.key === "free" ? 1 : 0.85, boxShadow: s.key === "free" ? "inset 0 0 0 1px var(--line-strong)" : undefined }} />
            <span className="text-fg-muted">{s.label}</span>
            <span className="num text-fg">{formatBytes(segments[s.key as MemorySegmentKey])}</span>
          </li>
        ))}
      </ul>
      <SwapMeter used={m.swapUsed} total={m.swapTotal} />
    </div>
  );
}

function SwapMeter({ used, total }: { used: number; total: number }) {
  if (total <= 0) {
    return <p className="text-[12px] text-fg-subtle">No swap space is configured.</p>;
  }
  const ratio = Math.min(1, used / total);
  return (
    <div className="flex items-center gap-3 border-t border-line pt-3 text-[12px]">
      <span className="w-12 text-fg-muted">Swap</span>
      <div className="relative h-1.5 flex-1 overflow-hidden rounded-full bg-[var(--viz-track)]">
        <div
          className="absolute inset-0 origin-left rounded-full bg-[var(--viz-2)] transition-transform duration-300 ease-out motion-reduce:transition-none"
          style={{ transform: `scaleX(${ratio})` }}
        />
      </div>
      <span className="num text-fg">{formatBytes(used)}</span>
      <span className="text-fg-muted">of {formatBytes(total)}</span>
    </div>
  );
}

export function TopMemoryList() {
  useProcessFeed();
  const processes = useProcesses((s) => s.processes);
  const status = useProcesses((s) => s.status);
  const error = useProcesses((s) => s.error);
  const load = useProcesses((s) => s.load);
  const totalMemory = useProcesses((s) => s.totalMemory);

  const top = useMemo(() => [...processes].sort((a, b) => b.memoryRss - a.memoryRss).slice(0, TOP_COUNT), [processes]);

  if (processes.length === 0 && status === "error") {
    return <ErrorState error={error} subject="The process list" compact onRetry={() => void load()} />;
  }
  if (processes.length === 0) {
    return (
      <div className="flex flex-col gap-3">
        {Array.from({ length: TOP_COUNT }, (_, i) => (
          <div key={i} className="flex flex-col gap-1.5">
            <Skeleton className="h-2.5" style={{ width: `${70 - i * 5}%` }} />
            <Skeleton className="h-1" />
          </div>
        ))}
      </div>
    );
  }

  return (
    <ol className="flex flex-col">
      {top.map((p) => {
        const share = totalMemory > 0 ? p.memoryRss / totalMemory : 0;
        return (
          <li key={`${p.pid}:${p.startTime}`}>
            <button
              type="button"
              className="group flex w-full flex-col gap-1 rounded-[5px] px-2 py-1.5 text-left hover:bg-raised"
              title={`Show ${p.name} in Processes`}
              onClick={() => {
                useProcessView.getState().select({ pid: p.pid, startTime: p.startTime }, true);
                useSettings.getState().setView("processes");
              }}
            >
              <div className="flex items-baseline justify-between gap-3 text-[12px]">
                <span className="truncate text-fg">{p.name}</span>
                <span className="num shrink-0 text-fg">{formatBytes(p.memoryRss)}</span>
              </div>
              <div className="relative h-1 overflow-hidden rounded-full bg-[var(--viz-track)]">
                <div
                  className="absolute inset-0 origin-left rounded-full bg-[var(--viz-1)] transition-transform duration-300 ease-out motion-reduce:transition-none"
                  style={{ transform: `scaleX(${Math.min(1, share)})` }}
                />
              </div>
            </button>
          </li>
        );
      })}
    </ol>
  );
}
