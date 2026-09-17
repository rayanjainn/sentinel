import { Fan, Thermometer } from "@phosphor-icons/react";

import type { LoadKind } from "../../bindings/LoadKind";
import { AnimatedNumber } from "../../components/AnimatedNumber";
import { cx } from "../../lib/cx";
import { formatCount } from "../../lib/format";
import { useResources } from "../../stores/resources";

const LOAD_COPY: Record<LoadKind, { title: string; detail: string }> = {
  unixRunQueue: {
    title: "Load average",
    detail: "Processes running or waiting for a CPU, averaged over 1, 5 and 15 minutes.",
  },
  windowsProcessorQueue: {
    title: "Processor queue length",
    detail: "Threads ready to run but waiting for a processor, averaged over 1, 5 and 15 minutes.",
  },
};

const fmtLoad = (v: number) => v.toFixed(2);

export function LoadPanel() {
  const load = useResources((s) => s.latest?.load ?? null);
  const hasSample = useResources((s) => s.latest !== null);
  const cores = useResources((s) => s.latest?.perCore.length ?? 0);
  if (!hasSample) return null;
  if (!load) {
    return (
      <section className="flex flex-col gap-1">
        <h2 className="text-[13px] font-medium text-fg-muted">Load average</h2>
        <p className="text-[12px] text-fg-subtle">Not available on this system.</p>
      </section>
    );
  }
  const copy = LOAD_COPY[load.kind];
  const perCore = cores > 0 ? load.one / cores : null;
  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-col gap-0.5">
        <h2 className="text-[13px] font-medium text-fg">{copy.title}</h2>
        <p className="text-[12px] text-fg-muted">{copy.detail}</p>
      </div>
      <dl className="grid grid-cols-3 gap-4">
        {(
          [
            ["1 min", load.one],
            ["5 min", load.five],
            ["15 min", load.fifteen],
          ] as const
        ).map(([label, value]) => (
          <div key={label} className="flex flex-col gap-0.5">
            <dt className="text-[11px] text-fg-muted">{label}</dt>
            <dd>
              <AnimatedNumber value={value} format={fmtLoad} className="num text-[20px] text-fg" />
            </dd>
          </div>
        ))}
      </dl>
      {perCore !== null && (
        <p className={cx("text-[12px]", perCore > 1 ? "text-warn" : "text-fg-subtle")}>
          {perCore > 1
            ? `More work is queued than ${formatCount(cores)} logical cores can run at once.`
            : `${(perCore * 100).toFixed(0)}% of ${formatCount(cores)} logical cores’ capacity over the last minute.`}
        </p>
      )}
    </section>
  );
}

export function ThermalPanel() {
  const thermal = useResources((s) => s.latest?.thermal ?? null);
  const hasSample = useResources((s) => s.latest !== null);
  if (!hasSample) return null;
  if (!thermal || (thermal.temperatures.length === 0 && thermal.fans.length === 0)) {
    return (
      <section className="flex flex-col gap-1">
        <h2 className="text-[13px] font-medium text-fg-muted">Temperature and fans</h2>
        <p className="text-[12px] text-fg-subtle">
          Not available on this system. The operating system does not expose these sensors through a public interface.
        </p>
      </section>
    );
  }
  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-[13px] font-medium text-fg">Temperature and fans</h2>
      <ul className="flex flex-col gap-2">
        {thermal.temperatures.map((t) => {
          const limit = t.criticalCelsius ?? 105;
          const ratio = Math.min(1, Math.max(0, t.celsius / limit));
          const hot = t.criticalCelsius !== null && t.celsius >= t.criticalCelsius * 0.9;
          return (
            <li key={t.label} className="flex items-center gap-3 text-[12px]">
              <Thermometer size={13} className="shrink-0 text-fg-subtle" />
              <span className="w-32 truncate text-fg-muted" title={t.label}>
                {t.label}
              </span>
              <div className="relative h-1 flex-1 overflow-hidden rounded-full bg-[var(--viz-track)]">
                <div
                  className="absolute inset-0 origin-left rounded-full bg-[var(--viz-2)] transition-transform duration-300 ease-out motion-reduce:transition-none"
                  style={{ transform: `scaleX(${ratio})` }}
                />
              </div>
              <span className={cx("num w-14 text-right", hot ? "text-warn" : "text-fg")}>{t.celsius.toFixed(0)} °C</span>
            </li>
          );
        })}
        {thermal.fans.map((f) => (
          <li key={f.label} className="flex items-center gap-3 text-[12px]">
            <Fan size={13} className="shrink-0 text-fg-subtle" />
            <span className="w-32 truncate text-fg-muted" title={f.label}>
              {f.label}
            </span>
            <div className="relative h-1 flex-1 overflow-hidden rounded-full bg-[var(--viz-track)]">
              {f.maxRpm !== null && f.maxRpm > 0 && (
                <div
                  className="absolute inset-0 origin-left rounded-full bg-[var(--viz-1)] transition-transform duration-300 ease-out motion-reduce:transition-none"
                  style={{ transform: `scaleX(${Math.min(1, f.rpm / f.maxRpm)})` }}
                />
              )}
            </div>
            <span className="num w-20 text-right text-fg">{formatCount(f.rpm)} rpm</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
