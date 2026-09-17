import { useMemo } from "react";
import { create } from "zustand";

import type { ResourceSample } from "../../bindings/ResourceSample";
import { AnimatedNumber } from "../../components/AnimatedNumber";
import { LiveChart } from "../../components/charts/LiveChart";
import { Segmented } from "../../components/Segmented";
import { Skeleton } from "../../components/Skeleton";
import { ErrorState, StatePanel } from "../../components/States";
import { formatPercent } from "../../lib/format";
import { HeaderToolbar } from "../../shell/HeaderSlot";
import { useResources } from "../../stores/resources";
import { useSettings } from "../../stores/settings";
import { SERIES } from "../../styles/dataviz";
import { CoreField } from "./CoreField";
import { MemoryChart, TopMemoryList } from "./MemoryPanel";
import { resourceSource, sampleTime } from "./sources";
import { LoadPanel, ThermalPanel } from "./SystemPanels";

type WindowKey = "60" | "300" | "900";

const useWindow = create<{ window: WindowKey }>(() => ({ window: "300" }));

function CpuSection({ windowMs }: { windowMs: number }) {
  const cpuTotal = useResources((s) => s.latest?.cpuTotal ?? 0);
  const perCore = useResources((s) => s.latest?.perCore);
  const info = useResources((s) => s.systemInfo);
  const intervalMs = useSettings((s) => s.intervalMs);
  const series = useMemo(
    () => [{ key: "cpu", label: "CPU", color: SERIES.cpu, value: (d: ResourceSample) => d.cpuTotal }],
    [],
  );

  let busiest: { index: number; value: number } | null = null;
  perCore?.forEach((value, index) => {
    if (!busiest || value > busiest.value) busiest = { index, value };
  });
  const top = busiest as { index: number; value: number } | null;

  return (
    <section aria-labelledby="cpu-title" className="flex flex-col gap-3">
      <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-1">
        <div className="flex flex-col gap-0.5">
          <h2 id="cpu-title" className="text-[13px] font-medium text-fg">
            CPU
          </h2>
          <div className="flex items-baseline gap-2">
            <AnimatedNumber value={cpuTotal} format={formatPercent} className="text-[28px] font-semibold tracking-display text-fg" />
            <span className="text-fg-muted">across all cores</span>
          </div>
        </div>
        <div className="flex flex-col items-end gap-0.5 text-[12px] text-fg-muted">
          {info && (
            <span>
              {info.cpuBrand}, {info.physicalCores ? `${info.physicalCores} cores, ` : ""}
              {info.logicalCores} threads
            </span>
          )}
          {top && top.value > 0 && (
            <span>
              Busiest: core {top.index + 1} at <span className="num text-fg">{formatPercent(top.value, 0)}</span>
            </span>
          )}
        </div>
      </div>
      <LiveChart
        source={resourceSource}
        series={series}
        time={sampleTime}
        windowMs={windowMs}
        intervalMs={intervalMs}
        height={160}
        yMax={100}
        grid={[0.25, 0.5, 0.75, 1]}
        formatValue={formatPercent}
        formatTick={(v) => formatPercent(v, 0)}
        showTimeAxis
        ariaLabel="Total CPU usage over time"
      />
    </section>
  );
}

function LoadingLayout() {
  return (
    <div className="flex flex-col gap-8 px-6 py-5" role="status" aria-label="Loading CPU and memory">
      <div className="flex flex-col gap-3">
        <Skeleton className="h-3 w-12" />
        <Skeleton className="h-7 w-32" />
        <Skeleton className="h-40 w-full" />
      </div>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(156px,1fr))] gap-px">
        {Array.from({ length: 8 }, (_, i) => (
          <Skeleton key={i} className="h-16 rounded-none" />
        ))}
      </div>
      <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,1fr)] gap-8">
        <Skeleton className="h-48" />
        <Skeleton className="h-48" />
      </div>
    </div>
  );
}

export function ResourcesView() {
  const windowKey = useWindow((s) => s.window);
  const hasSample = useResources((s) => s.latest !== null);
  const historyState = useResources((s) => s.historyState);
  const historyError = useResources((s) => s.historyError);
  const loadHistory = useResources((s) => s.loadHistory);
  const windowMs = Number(windowKey) * 1000;

  return (
    <div className="h-full overflow-y-auto">
      <HeaderToolbar>
        <Segmented<WindowKey>
          label="Time window"
          value={windowKey}
          onChange={(w) => useWindow.setState({ window: w })}
          options={[
            { value: "60", label: "1 min" },
            { value: "300", label: "5 min" },
            { value: "900", label: "15 min" },
          ]}
        />
      </HeaderToolbar>
      {!hasSample && historyState === "loading" && <LoadingLayout />}
      {!hasSample && historyState === "error" && (
        <StatePanel className="h-full">
          <ErrorState error={historyError} subject="CPU and memory history" onRetry={() => void loadHistory()} />
        </StatePanel>
      )}
      {!hasSample && historyState === "ready" && (
        <StatePanel className="h-full">
          <p className="text-fg-muted">Waiting for the first sample from the system sampler.</p>
        </StatePanel>
      )}
      {hasSample && (
        <div className="mx-auto flex max-w-[1400px] flex-col gap-8 px-6 py-5">
          <div className="grid grid-cols-[minmax(0,1fr)_280px] gap-8">
            <CpuSection windowMs={windowMs} />
            <div className="flex flex-col gap-6 border-l border-line pl-8">
              <LoadPanel />
              <ThermalPanel />
            </div>
          </div>
          <section aria-labelledby="cores-title" className="flex flex-col gap-3">
            <h2 id="cores-title" className="text-[13px] font-medium text-fg">
              Per core
            </h2>
            <CoreField />
          </section>
          <div className="grid grid-cols-[minmax(0,1fr)_280px] gap-8 border-t border-line pt-6">
            <section aria-labelledby="memory-title" className="flex flex-col gap-3">
              <h2 id="memory-title" className="text-[13px] font-medium text-fg">
                Memory
              </h2>
              <MemoryChart windowMs={windowMs} />
            </section>
            <section aria-labelledby="top-memory-title" className="flex flex-col gap-2 border-l border-line pl-8">
              <h2 id="top-memory-title" className="text-[13px] font-medium text-fg">
                Using the most memory
              </h2>
              <TopMemoryList />
            </section>
          </div>
        </div>
      )}
    </div>
  );
}
