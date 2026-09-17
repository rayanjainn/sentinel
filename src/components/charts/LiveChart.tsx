// Real-time rolling graph. Paths are recomputed imperatively when data arrives and the plot group
// translates continuously between samples, so the graph scrolls smoothly without React renders.
import { curveMonotoneX } from "@visx/curve";
import { scaleLinear } from "@visx/scale";
import { area as areaPath, line as linePath } from "@visx/shape";
import { useCallback, useEffect, useId, useRef, useState, type PointerEvent } from "react";

import { cx } from "../../lib/cx";
import { useElementSize, useOnScreen, usePrefersReducedMotion } from "../../lib/motion";
import { lowerBoundBy } from "../../lib/series";
import { onFrame } from "../../lib/ticker";

export interface ChartSeries<T> {
  key: string;
  label: string;
  color: string;
  value: (d: T) => number;
}

export interface DataSource<T> {
  get: () => T[];
  subscribe: (listener: () => void) => () => void;
}

export interface LiveChartProps<T> {
  source: DataSource<T>;
  series: ChartSeries<T>[];
  time: (d: T) => number;
  windowMs: number;
  intervalMs: number;
  height: number;
  /** Fixed upper bound (e.g. 100 for percentages); otherwise scales to the visible data. */
  yMax?: number;
  /** Lowest automatic upper bound so a quiet series does not amplify noise. */
  yFloor?: number;
  stacked?: boolean;
  /** Horizontal gridlines as fractions of the y domain. */
  grid?: number[];
  formatValue: (value: number) => string;
  /** Labels for gridlines; omitted when not provided. */
  formatTick?: (value: number) => string;
  showTimeAxis?: boolean;
  tooltip?: boolean;
  ariaLabel: string;
  className?: string;
  /** Area wash under unstacked lines. */
  fill?: boolean;
}

const PAD_TOP = 6;

export function niceMax(value: number): number {
  if (!(value > 0)) return 1;
  const exp = Math.pow(10, Math.floor(Math.log10(value)));
  const f = value / exp;
  const nice = f <= 1 ? 1 : f <= 2 ? 2 : f <= 2.5 ? 2.5 : f <= 5 ? 5 : 10;
  return nice * exp;
}

export function LiveChart<T>({
  source,
  series,
  time,
  windowMs,
  intervalMs,
  height,
  yMax,
  yFloor = 1,
  stacked = false,
  grid = [0.5, 1],
  formatValue,
  formatTick,
  showTimeAxis = false,
  tooltip = true,
  ariaLabel,
  className,
  fill = true,
}: LiveChartProps<T>) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const groupRef = useRef<SVGGElement>(null);
  const lineRefs = useRef(new Map<string, SVGPathElement | null>());
  const areaRefs = useRef(new Map<string, SVGPathElement | null>());
  const tickRefs = useRef(new Map<number, SVGTextElement | null>());
  const { width } = useElementSize(wrapRef);
  const onScreen = useOnScreen(wrapRef);
  const reduced = usePrefersReducedMotion();
  const clipId = useId();
  const [hover, setHover] = useState<{ x: number; item: T } | null>(null);
  const latestTs = useRef(0);
  const frozenNow = useRef<number | null>(null);
  const lastTranslate = useRef(Number.NaN);

  const plotHeight = height - PAD_TOP;
  const pxPerMs = width > 0 ? width / windowMs : 0;
  const delay = intervalMs;

  const translateFor = useCallback(
    (now: number) => {
      const renderNow = (frozenNow.current ?? now) - delay;
      return width - (renderNow - latestTs.current) * pxPerMs;
    },
    [width, pxPerMs, delay],
  );

  const applyTranslate = useCallback((tx: number) => {
    if (!groupRef.current || Math.abs(tx - lastTranslate.current) < 0.2) return;
    lastTranslate.current = tx;
    groupRef.current.setAttribute("transform", `translate(${tx.toFixed(2)} 0)`);
  }, []);

  const redraw = useCallback(() => {
    const data = source.get();
    if (width <= 0 || data.length === 0) {
      lineRefs.current.forEach((el) => el?.setAttribute("d", ""));
      areaRefs.current.forEach((el) => el?.setAttribute("d", ""));
      return;
    }
    const newest = time(data[data.length - 1]!);
    latestTs.current = newest;
    const start = Math.max(0, lowerBoundBy(data, newest - windowMs - delay * 2, time) - 1);
    const visible = data.slice(start);

    let peak = 0;
    for (const d of visible) {
      let v = 0;
      for (const s of series) v = stacked ? v + Math.max(0, s.value(d)) : Math.max(v, s.value(d));
      if (v > peak) peak = v;
    }
    const max = yMax ?? niceMax(Math.max(yFloor, peak * 1.1));
    if (formatTick) {
      tickRefs.current.forEach((el, g) => {
        if (el) el.textContent = formatTick(g * max);
      });
    }

    const y = scaleLinear<number>({ domain: [0, max], range: [plotHeight + PAD_TOP, PAD_TOP], clamp: true });
    const x = (d: T) => (time(d) - newest) * pxPerMs;

    if (stacked) {
      const cumulative = visible.map(() => 0);
      const indices = visible.map((_, i) => i);
      for (const s of series) {
        const lower = [...cumulative];
        visible.forEach((d, i) => {
          cumulative[i] = cumulative[i]! + Math.max(0, s.value(d));
        });
        const upper = [...cumulative];
        const gen = areaPath<number>({
          x: (i) => x(visible[i]!),
          y0: (i) => y(lower[i]!),
          y1: (i) => y(upper[i]!),
          curve: curveMonotoneX,
        });
        const top = linePath<number>({ x: (i) => x(visible[i]!), y: (i) => y(upper[i]!), curve: curveMonotoneX });
        areaRefs.current.get(s.key)?.setAttribute("d", gen(indices) ?? "");
        lineRefs.current.get(s.key)?.setAttribute("d", top(indices) ?? "");
      }
    } else {
      for (const s of series) {
        const l = linePath<T>({ x, y: (d) => y(s.value(d)), curve: curveMonotoneX });
        lineRefs.current.get(s.key)?.setAttribute("d", l(visible) ?? "");
        if (fill) {
          const a = areaPath<T>({ x, y0: y(0), y1: (d) => y(s.value(d)), curve: curveMonotoneX });
          areaRefs.current.get(s.key)?.setAttribute("d", a(visible) ?? "");
        }
      }
    }
    lastTranslate.current = Number.NaN;
    applyTranslate(reduced ? width : translateFor(Date.now()));
  }, [source, width, time, windowMs, delay, stacked, series, yMax, yFloor, formatTick, plotHeight, pxPerMs, fill, applyTranslate, reduced, translateFor]);

  useEffect(() => {
    redraw();
    return source.subscribe(redraw);
  }, [source, redraw]);

  useEffect(() => {
    if (reduced || !onScreen || width <= 0) return;
    return onFrame((now) => applyTranslate(translateFor(now)));
  }, [reduced, onScreen, width, applyTranslate, translateFor]);

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!tooltip || width <= 0) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const px = event.clientX - rect.left;
    if (frozenNow.current === null) frozenNow.current = Date.now();
    const renderNow = frozenNow.current - delay;
    const t = renderNow - (width - px) / pxPerMs;
    const data = source.get();
    if (data.length === 0) return;
    const i = lowerBoundBy(data, t, time);
    const candidates = [data[i - 1], data[i]].filter((d): d is T => d !== undefined);
    const nearest = candidates.sort((a, b) => Math.abs(time(a) - t) - Math.abs(time(b) - t))[0];
    if (!nearest) return;
    setHover({ x: width - (renderNow - time(nearest)) * pxPerMs, item: nearest });
  };

  const onPointerLeave = () => {
    frozenNow.current = null;
    setHover(null);
  };

  const hoverTotal = hover && stacked ? series.reduce((sum, s) => sum + s.value(hover.item), 0) : null;

  return (
    <div ref={wrapRef} className={cx("relative w-full", className)} onPointerMove={onPointerMove} onPointerLeave={onPointerLeave}>
      <svg width="100%" height={height} role="img" aria-label={ariaLabel} className="block overflow-visible">
        <defs>
          <clipPath id={clipId}>
            <rect x={0} y={0} width={Math.max(0, width)} height={height} />
          </clipPath>
        </defs>
        {grid.map((g) => {
          const yy = Math.round(PAD_TOP + plotHeight * (1 - g)) + 0.5;
          return (
            <g key={g}>
              <line x1={0} x2={width} y1={yy} y2={yy} stroke="var(--viz-grid)" strokeWidth={1} />
              {formatTick && (
                <text
                  ref={(el) => {
                    tickRefs.current.set(g, el);
                  }}
                  x={width - 2}
                  y={yy - 3}
                  textAnchor="end"
                  fill="var(--fg-subtle)"
                  fontSize={11}
                  style={{ fontVariantNumeric: "tabular-nums" }}
                />
              )}
            </g>
          );
        })}
        <line x1={0} x2={width} y1={height - 0.5} y2={height - 0.5} stroke="var(--line-strong)" strokeWidth={1} />
        <g clipPath={`url(#${clipId})`}>
          <g ref={groupRef}>
            {(stacked || fill ? series : []).map((s) => (
              <path
                key={`a-${s.key}`}
                ref={(el) => {
                  areaRefs.current.set(s.key, el);
                }}
                fill={s.color}
                fillOpacity={stacked ? 0.78 : 0.1}
              />
            ))}
            {series.map((s) => (
              <path
                key={`l-${s.key}`}
                ref={(el) => {
                  lineRefs.current.set(s.key, el);
                }}
                fill="none"
                stroke={stacked ? "var(--panel)" : s.color}
                strokeWidth={stacked ? 2 : 2}
                strokeLinejoin="round"
                strokeLinecap="round"
              />
            ))}
          </g>
        </g>
        {hover && <line x1={hover.x} x2={hover.x} y1={PAD_TOP} y2={height} stroke="var(--fg-subtle)" strokeWidth={1} />}
      </svg>
      {showTimeAxis && (
        <div className="mt-1 flex justify-between text-[11px] text-fg-subtle">
          <span>{windowMs >= 60_000 ? `${Math.round(windowMs / 60_000)} min ago` : `${Math.round(windowMs / 1000)} s ago`}</span>
          <span>Now</span>
        </div>
      )}
      {hover && (
        <div
          className="shadow-float pointer-events-none absolute top-1 z-10 flex min-w-40 flex-col gap-1 rounded-[6px] bg-raised px-2.5 py-2 text-[12px]"
          style={hover.x > width / 2 ? { right: width - hover.x + 10 } : { left: hover.x + 10 }}
        >
          <span className="text-[11px] text-fg-subtle">
            {new Date(time(hover.item)).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" })}
          </span>
          {[...series].reverse().map((s) => (
            <div key={s.key} className="flex items-center justify-between gap-4">
              <span className="flex items-center gap-1.5 text-fg-muted">
                <span className="h-0.5 w-2.5 rounded-full" style={{ background: s.color }} />
                {s.label}
              </span>
              <span className="num text-fg">{formatValue(s.value(hover.item))}</span>
            </div>
          ))}
          {hoverTotal !== null && (
            <div className="flex items-center justify-between gap-4 border-t border-line pt-1">
              <span className="text-fg-muted">Total</span>
              <span className="num text-fg">{formatValue(hoverTotal)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/** Adapts a zustand store slice into a chart data source. */
export function storeSource<S, T>(
  store: { getState: () => S; subscribe: (listener: (state: S, prev: S) => void) => () => void },
  select: (state: S) => T[],
): DataSource<T> {
  return {
    get: () => select(store.getState()),
    subscribe: (listener) =>
      store.subscribe((state, prev) => {
        if (select(state) !== select(prev)) listener();
      }),
  };
}
