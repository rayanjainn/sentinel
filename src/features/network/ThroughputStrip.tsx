import { ArrowDown, ArrowUp } from "@phosphor-icons/react";

import type { ResourceSample } from "../../bindings/ResourceSample";
import { AnimatedNumber } from "../../components/AnimatedNumber";
import { LiveChart } from "../../components/charts/LiveChart";
import { formatBytes, formatRate } from "../../lib/format";
import { useResources } from "../../stores/resources";
import { useSettings } from "../../stores/settings";
import { SERIES } from "../../styles/dataviz";
import { resourceSource, sampleTime } from "../resources/sources";

const SERIES_DEF = [
  { key: "rx", label: "Download", color: SERIES.rx, value: (d: ResourceSample) => d.network.rxBps },
  { key: "tx", label: "Upload", color: SERIES.tx, value: (d: ResourceSample) => d.network.txBps },
];

/** Machine-wide inbound and outbound throughput, from the always-on resources stream. */
export function ThroughputStrip() {
  const net = useResources((s) => s.latest?.network ?? null);
  const intervalMs = useSettings((s) => s.intervalMs);
  return (
    <div className="grid grid-cols-[220px_minmax(0,1fr)] items-center gap-6">
      <div className="flex flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <span className="h-0.5 w-3 rounded-full" style={{ background: SERIES.rx }} />
          <ArrowDown size={12} className="text-fg-muted" />
          <span className="w-16 text-[12px] text-fg-muted">Download</span>
          <AnimatedNumber value={net?.rxBps ?? 0} format={formatRate} className="num text-[13px] text-fg" />
        </div>
        <div className="flex items-center gap-2">
          <span className="h-0.5 w-3 rounded-full" style={{ background: SERIES.tx }} />
          <ArrowUp size={12} className="text-fg-muted" />
          <span className="w-16 text-[12px] text-fg-muted">Upload</span>
          <AnimatedNumber value={net?.txBps ?? 0} format={formatRate} className="num text-[13px] text-fg" />
        </div>
        {net && (
          <span className="text-[11px] text-fg-subtle">
            <span className="num">{formatBytes(net.rxTotal, { base: 1000 })}</span> in,{" "}
            <span className="num">{formatBytes(net.txTotal, { base: 1000 })}</span> out since boot
          </span>
        )}
      </div>
      <LiveChart
        source={resourceSource}
        series={SERIES_DEF}
        time={sampleTime}
        windowMs={120_000}
        intervalMs={intervalMs}
        height={56}
        yFloor={16_000}
        grid={[1]}
        formatValue={formatRate}
        formatTick={formatRate}
        ariaLabel="Machine-wide download and upload rate over the last two minutes"
      />
    </div>
  );
}
