import type { CSSProperties } from "react";

import { cx } from "../lib/cx";

export function Skeleton({ className, style }: { className?: string; style?: CSSProperties }) {
  return <div aria-hidden className={cx("skeleton", className)} style={style} />;
}

/** Table-shaped placeholder: rows of cells with varied widths so it reads as data, not stripes. */
export function SkeletonRows({
  rows = 12,
  columns,
  rowHeight = 28,
  className,
}: {
  rows?: number;
  /** Relative column widths in fr units. */
  columns: number[];
  rowHeight?: number;
  className?: string;
}) {
  const template = columns.map((c) => `${c}fr`).join(" ");
  return (
    <div role="status" aria-label="Loading" className={cx("flex flex-col", className)}>
      {Array.from({ length: rows }, (_, r) => (
        <div
          key={r}
          className="grid items-center gap-4 border-b border-line px-3"
          style={{ gridTemplateColumns: template, height: rowHeight }}
        >
          {columns.map((_, c) => (
            <Skeleton
              key={c}
              className="h-2.5"
              style={{ width: `${45 + ((r * 7 + c * 13) % 50)}%`, opacity: 1 - r / (rows * 1.4) }}
            />
          ))}
        </div>
      ))}
    </div>
  );
}
