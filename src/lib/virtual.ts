// Minimal fixed-row-height virtualizer: renders only rows intersecting the scroll viewport.
import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

export interface VirtualRange {
  start: number;
  end: number;
}

export function computeRange(
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  count: number,
  overscan: number,
): VirtualRange {
  if (count === 0 || rowHeight <= 0) return { start: 0, end: 0 };
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const visible = Math.ceil(Math.max(0, viewportHeight) / rowHeight) + 1;
  const start = Math.max(0, first - overscan);
  const end = Math.min(count, first + visible + overscan);
  return { start: Math.min(start, end), end };
}

/** Scroll offset that brings `index` into view with minimal movement, or null if already visible. */
export function scrollOffsetFor(
  index: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  headerOffset = 0,
): number | null {
  const top = index * rowHeight;
  const bottom = top + rowHeight;
  const visibleTop = scrollTop;
  const visibleBottom = scrollTop + viewportHeight - headerOffset;
  if (top < visibleTop) return top;
  if (bottom > visibleBottom) return bottom - (viewportHeight - headerOffset);
  return null;
}

export function useVirtualRows(
  scrollRef: RefObject<HTMLElement | null>,
  count: number,
  rowHeight: number,
  { overscan = 10, headerOffset = 0 }: { overscan?: number; headerOffset?: number } = {},
) {
  const [metrics, setMetrics] = useState({ scrollTop: 0, height: 0 });
  const frame = useRef(0);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const read = () => {
      frame.current = 0;
      setMetrics((prev) =>
        prev.scrollTop === el.scrollTop && prev.height === el.clientHeight
          ? prev
          : { scrollTop: el.scrollTop, height: el.clientHeight },
      );
    };
    const onScroll = () => {
      if (!frame.current) frame.current = requestAnimationFrame(read);
    };
    read();
    el.addEventListener("scroll", onScroll, { passive: true });
    const ro = typeof ResizeObserver !== "undefined" ? new ResizeObserver(onScroll) : null;
    ro?.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro?.disconnect();
      if (frame.current) cancelAnimationFrame(frame.current);
    };
  }, [scrollRef]);

  const range = computeRange(
    Math.max(0, metrics.scrollTop - headerOffset),
    metrics.height,
    rowHeight,
    count,
    overscan,
  );

  const scrollToIndex = useCallback(
    (index: number) => {
      const el = scrollRef.current;
      if (!el) return;
      const offset = scrollOffsetFor(index, Math.max(0, el.scrollTop - headerOffset), el.clientHeight, rowHeight, headerOffset);
      if (offset !== null) el.scrollTop = offset + headerOffset;
    },
    [scrollRef, rowHeight, headerOffset],
  );

  return { ...range, totalHeight: count * rowHeight, scrollToIndex };
}
