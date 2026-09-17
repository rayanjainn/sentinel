import { memo, useMemo, useRef } from "react";

import type { ResourceSample } from "../../bindings/ResourceSample";
import { AnimatedNumber } from "../../components/AnimatedNumber";
import { LiveChart } from "../../components/charts/LiveChart";
import { formatPercent } from "../../lib/format";
import { useElementSize } from "../../lib/motion";
import { useResources } from "../../stores/resources";
import { useSettings } from "../../stores/settings";
import { SERIES } from "../../styles/dataviz";
import { resourceSource, sampleTime } from "./sources";

const SPARK_WINDOW_MS = 60_000;
const fmt = (v: number) => formatPercent(v, 0);

const CoreTile = memo(function CoreTile({ index }: { index: number }) {
  const value = useResources((s) => s.latest?.perCore[index] ?? 0);
  const intervalMs = useSettings((s) => s.intervalMs);
  const series = useMemo(
    () => [{ key: "core", label: `Core ${index + 1}`, color: SERIES.cpu, value: (d: ResourceSample) => d.perCore[index] ?? 0 }],
    [index],
  );
  return (
    <div className="flex min-w-0 flex-col gap-1.5 bg-ground px-3 pb-2.5 pt-2">
      <div className="flex items-baseline justify-between">
        <span className="text-[11px] text-fg-muted">Core {index + 1}</span>
        <AnimatedNumber value={value} format={fmt} className="num text-[13px] text-fg" />
      </div>
      <div className="flex items-end gap-2">
        <div className="min-w-0 flex-1">
          <LiveChart
            source={resourceSource}
            series={series}
            time={sampleTime}
            windowMs={SPARK_WINDOW_MS}
            intervalMs={intervalMs}
            height={34}
            yMax={100}
            grid={[]}
            tooltip={false}
            formatValue={fmt}
            ariaLabel={`Core ${index + 1} usage over the last minute`}
          />
        </div>
        <div className="relative h-[34px] w-1.5 shrink-0 overflow-hidden rounded-full bg-[var(--viz-track)]">
          <div
            className="absolute inset-0 origin-bottom rounded-full bg-[var(--viz-1)] transition-transform duration-300 ease-out motion-reduce:transition-none"
            style={{ transform: `scaleY(${Math.min(1, Math.max(0, value / 100))})` }}
          />
        </div>
      </div>
    </div>
  );
});

/** Columns that fit `width` and split `count` tiles into even rows, so no row ends with an orphan. */
export function balancedColumns(count: number, width: number, minTile: number): number {
  if (count <= 0) return 1;
  const maxCols = Math.max(1, Math.min(count, Math.floor(width / minTile)));
  const rows = Math.ceil(count / maxCols);
  return Math.ceil(count / rows);
}

/** One live tile per logical core: tweened value, level bar and a scrolling one-minute sparkline. */
export function CoreField() {
  const cores = useResources((s) => s.latest?.perCore.length ?? 0);
  const ref = useRef<HTMLDivElement>(null);
  const { width } = useElementSize(ref);
  const columns = balancedColumns(cores, width || 1200, cores > 24 ? 132 : 156);
  return (
    <div
      ref={ref}
      role="list"
      aria-label="Per-core usage"
      className={cores === 0 ? "hidden" : "grid gap-px overflow-hidden rounded-[8px] border border-line bg-line"}
      style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
    >
      {Array.from({ length: cores }, (_, i) => (
        <div role="listitem" key={i} className="contents">
          <CoreTile index={i} />
        </div>
      ))}
    </div>
  );
}
